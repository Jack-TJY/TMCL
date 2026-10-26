//! 资源浏览、依赖解析与安装的离线测试：Modrinth API 与 CDN 都由本地模拟服务器按内存中的
//! “目录”应答（版本、项目、哈希均为构造的最小 JSON），不联网。

#[allow(dead_code)] // 与 modrinth.rs 共用，部分辅助函数只在那边使用。
mod support;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use support::{MockServer, Request, Response};
use tmcl_common::{ErrorCode, TaskCtx};
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};
use tmcl_resources::browse::{self, BrowseQuery, ResourceSort};
use tmcl_resources::modrinth::ModrinthClient;
use tmcl_resources::{InstallRequest, InstanceTarget, ResourceInstaller, ResourceKind};

const SEARCH: &[u8] = include_bytes!("fixtures/modrinth/search.json");
const PROJECT_VERSIONS: &[u8] = include_bytes!("fixtures/modrinth/project_versions.json");

fn ctx() -> TaskCtx {
    TaskCtx::noop("test")
}

fn client(api: &MockServer) -> ModrinthClient {
    ModrinthClient::with_base(&HttpClient::new().unwrap(), &api.url("/v2"))
        .unwrap()
        .with_retry(RetryPolicy::with_delays([Duration::ZERO; 3]))
}

fn fabric() -> InstanceTarget {
    InstanceTarget {
        game_version: "1.20.1".into(),
        loader: Some("fabric".into()),
    }
}

/// 计算内容的 SHA1：借助下载引擎的同一实现。
async fn sha1(body: &[u8]) -> String {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), body).unwrap();
    tmcl_net::sha1_file(tmp.path()).await.unwrap()
}

/// 模拟的 Modrinth 数据。
#[derive(Default)]
struct Catalog {
    versions: HashMap<String, Value>,
    projects: HashMap<String, Value>,
    /// 项目 ID → 该项目的版本列表（新版本在前）。
    project_versions: HashMap<String, Vec<String>>,
    /// SHA1 → 版本 ID。
    hashes: HashMap<String, String>,
    /// CDN 文件名 → 内容。
    files: HashMap<String, Vec<u8>>,
}

impl Catalog {
    fn project(&mut self, id: &str, kind: &str, loaders: &[&str]) {
        self.projects.insert(
            id.into(),
            json!({ "id": id, "slug": id.to_lowercase(), "project_type": kind,
                "title": format!("Title {id}"), "client_side": "required",
                "server_side": "optional", "license": { "id": "MIT" }, "loaders": loaders }),
        );
    }

    /// 登记一个版本；`deps` 为 `(project_id, version_id, type)`。
    async fn version(
        &mut self,
        cdn: &MockServer,
        id: &str,
        project: &str,
        channel: &str,
        deps: &[(Option<&str>, Option<&str>, &str)],
    ) -> String {
        let name = format!("{}.jar", id.to_lowercase());
        let body = format!("content of {id}").into_bytes();
        let hash = sha1(&body).await;
        self.files.insert(name.clone(), body.clone());
        self.hashes.insert(hash.clone(), id.into());
        let deps: Vec<Value> = deps
            .iter()
            .map(|(p, v, t)| json!({ "project_id": p, "version_id": v, "dependency_type": t }))
            .collect();
        self.versions.insert(
            id.into(),
            json!({ "id": id, "project_id": project, "name": id, "version_number": id,
                "version_type": channel, "game_versions": ["1.20.1"], "loaders": ["fabric"],
                "files": [{ "hashes": { "sha1": hash }, "url": cdn.url(&format!("/{name}")),
                    "filename": name, "primary": true, "size": body.len() }],
                "dependencies": deps }),
        );
        self.project_versions
            .entry(project.into())
            .or_default()
            .push(id.into());
        hash
    }

    fn ids(req: &Request) -> Vec<String> {
        let ids = req
            .query()
            .into_iter()
            .find(|(k, _)| k == "ids")
            .map(|(_, v)| v)
            .unwrap_or_default();
        serde_json::from_str(&ids).unwrap_or_default()
    }

    fn respond(&self, req: &Request) -> Response {
        let path = req.path().trim_start_matches("/v2/");
        let pick = |table: &HashMap<String, Value>, ids: Vec<String>| {
            let list: Vec<&Value> = ids.iter().filter_map(|id| table.get(id)).collect();
            Response::json(json!(list).to_string().as_bytes())
        };
        match path.split('/').collect::<Vec<_>>().as_slice() {
            ["version", id] => match self.versions.get(*id) {
                Some(v) => Response::json(v.to_string().as_bytes()),
                None => Response::status(404),
            },
            ["versions"] => pick(&self.versions, Self::ids(req)),
            ["projects"] => pick(&self.projects, Self::ids(req)),
            ["project", id, "version"] => match self.project_versions.get(*id) {
                Some(ids) => pick(&self.versions, ids.clone()),
                None => Response::status(404),
            },
            ["version_files"] => {
                let body: Value = serde_json::from_slice(&req.body).unwrap();
                let found: serde_json::Map<String, Value> = body["hashes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|h| {
                        let h = h.as_str().unwrap();
                        let v = self.versions.get(self.hashes.get(h)?)?;
                        Some((h.to_owned(), v.clone()))
                    })
                    .collect();
                Response::json(Value::Object(found).to_string().as_bytes())
            }
            _ => Response::status(404),
        }
    }
}

type Files = Arc<Mutex<HashMap<String, Vec<u8>>>>;

/// 模拟 CDN：按文件名返回 [`Catalog::files`] 中的内容（由 [`api`] 填入）。
async fn cdn() -> (MockServer, Files) {
    let files = Files::default();
    let served = files.clone();
    let server = MockServer::start(move |req, _| {
        match served
            .lock()
            .unwrap()
            .get(req.path().trim_start_matches('/'))
        {
            Some(body) => Response::json(body),
            None => Response::status(404),
        }
    })
    .await;
    (server, files)
}

/// 启动模拟 API，并把目录中的文件交给 CDN。
async fn api(catalog: Catalog, files: &Files) -> MockServer {
    files.lock().unwrap().extend(catalog.files.clone());
    let catalog = Arc::new(catalog);
    MockServer::start(move |req, _| catalog.respond(req)).await
}

fn installer(api: &MockServer, game: &Path) -> ResourceInstaller {
    let downloader = Downloader::new(HttpClient::new().unwrap(), DownloadOptions::default());
    ResourceInstaller::new(client(api), downloader, game.to_path_buf(), fabric())
}

fn request(version_id: &str, kind: ResourceKind, world: Option<&str>) -> InstallRequest {
    InstallRequest {
        version_id: version_id.into(),
        kind,
        world: world.map(Into::into),
    }
}

#[tokio::test]
async fn search_filters_by_instance() {
    let api = MockServer::start(|_, _| Response::json(SEARCH)).await;
    let target = InstanceTarget {
        game_version: "1.20.1".into(),
        loader: Some("quilt".into()),
    };
    let query = BrowseQuery {
        query: " sodium ".into(),
        sort: ResourceSort::Downloads,
        ..BrowseQuery::default()
    };
    let page = browse::search(&client(&api), &target, ResourceKind::Mod, &query, &ctx())
        .await
        .unwrap();
    assert_eq!(page.hits[0].slug, "sodium");
    assert!(page.total_hits > 0);
    let q = api.requests()[0].query();
    assert!(q.contains(&("query".into(), "sodium".into())));
    assert!(q.contains(&(
        "facets".into(),
        r#"[["project_type:mod"],["versions:1.20.1"],["categories:quilt","categories:fabric"]]"#
            .into()
    )));
    assert!(q.contains(&("limit".into(), "20".into())));
}

#[tokio::test]
async fn versions_are_filtered_and_missing_project_is_empty() {
    let api = MockServer::start(|req, _| match req.path() {
        "/v2/project/sodium/version" => Response::json(PROJECT_VERSIONS),
        _ => Response::status(404),
    })
    .await;
    let client = client(&api);
    let list = browse::versions(&client, &fabric(), ResourceKind::Mod, "sodium", &ctx())
        .await
        .unwrap();
    assert_eq!(list[0].id, "OihdIimA");
    assert!(list.iter().all(|v| !v.file_name.is_empty()));
    let q = api.requests()[0].query();
    assert!(q.contains(&("loaders".into(), r#"["fabric"]"#.into())));
    let none = browse::versions(&client, &fabric(), ResourceKind::Mod, "gone", &ctx())
        .await
        .unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn plan_resolves_dependencies_and_reports_conflicts_before_installing() {
    let (cdn, files) = cdn().await;
    let mut c = Catalog::default();
    for id in ["A", "B", "C", "D", "E", "M"] {
        c.project(id, "mod", &["fabric"]);
    }
    let required = "required";
    c.version(
        &cdn,
        "va1",
        "A",
        "release",
        &[
            (Some("B"), None, required),
            (Some("X"), None, "optional"),
            (Some("E"), None, "incompatible"),
            (Some("M"), None, required),
        ],
    )
    .await;
    // B 的最新版是测试版，应选正式版 vb1；vb1 指定了 C 的版本。
    c.version(&cdn, "vb2", "B", "beta", &[]).await;
    c.version(
        &cdn,
        "vb1",
        "B",
        "release",
        &[(None, Some("vc1"), required)],
    )
    .await;
    c.version(&cdn, "vc1", "C", "release", &[(Some("D"), None, required)])
        .await;
    c.version(&cdn, "vd1", "D", "release", &[]).await;
    c.version(&cdn, "ve1", "E", "release", &[]).await;
    c.project_versions.insert("M".into(), Vec::new());
    // D、E 已安装（E 处于禁用状态）。
    let tmp = tempfile::tempdir().unwrap();
    let mods = tmp.path().join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("d.jar"), &c.files["vd1.jar"]).unwrap();
    std::fs::write(mods.join("e.jar.disabled"), &c.files["ve1.jar"]).unwrap();
    let api = api(c, &files).await;

    let installer = installer(&api, tmp.path());
    let req = request("va1", ResourceKind::Mod, None);
    let plan = installer.plan(&req, &ctx()).await.unwrap();

    let files: Vec<(&str, Option<&str>)> = plan
        .files
        .iter()
        .map(|f| (f.version_id.as_str(), f.required_by.as_deref()))
        .collect();
    assert_eq!(
        files,
        [("va1", None), ("vb1", Some("A")), ("vc1", Some("B"))]
    );
    assert_eq!(plan.files[1].title, "Title B");
    assert_eq!(plan.files[1].kind, ResourceKind::Mod);
    assert_eq!(plan.conflicts.len(), 1);
    let conflict = &plan.conflicts[0];
    assert_eq!(
        (
            conflict.project_id.as_str(),
            conflict.with_project_id.as_str()
        ),
        ("A", "E")
    );
    assert!(conflict.with_installed);
    assert_eq!(conflict.with_title, "Title E");
    assert_eq!(plan.unavailable.len(), 1);
    assert_eq!(plan.unavailable[0].project_id.as_deref(), Some("M"));
    assert_eq!(plan.unavailable[0].required_by, "A");

    // B 的版本按实例过滤。
    let b = api
        .requests()
        .into_iter()
        .find(|r| r.path() == "/v2/project/B/version")
        .unwrap();
    assert!(
        b.query()
            .contains(&("game_versions".into(), r#"["1.20.1"]"#.into()))
    );

    let err = installer.install(&req, &ctx()).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesIncompatible);
    assert_eq!(err.params["with"], "Title E");
    assert!(!mods.join("va1.jar").exists());
}

#[tokio::test]
async fn conflicts_within_the_plan_are_reported() {
    let (cdn, files) = cdn().await;
    let mut c = Catalog::default();
    c.project("A", "mod", &["fabric"]);
    c.project("B", "mod", &["fabric"]);
    c.version(
        &cdn,
        "va1",
        "A",
        "release",
        &[(Some("B"), None, "required")],
    )
    .await;
    c.version(
        &cdn,
        "vb1",
        "B",
        "release",
        &[(None, Some("va1"), "incompatible")],
    )
    .await;
    let api = api(c, &files).await;
    let tmp = tempfile::tempdir().unwrap();
    let plan = installer(&api, tmp.path())
        .plan(&request("va1", ResourceKind::Mod, None), &ctx())
        .await
        .unwrap();
    assert_eq!(plan.conflicts.len(), 1);
    assert_eq!(plan.conflicts[0].project_id, "B");
    assert_eq!(plan.conflicts[0].with_project_id, "A");
    assert!(!plan.conflicts[0].with_installed);
}

#[tokio::test]
async fn install_downloads_dependencies_and_replaces_old_version() {
    let (cdn, files) = cdn().await;
    let mut c = Catalog::default();
    c.project("B", "mod", &["fabric"]);
    c.project("C", "mod", &["fabric"]);
    c.version(&cdn, "vb0", "B", "release", &[]).await;
    c.version(
        &cdn,
        "vb1",
        "B",
        "release",
        &[(Some("C"), None, "required")],
    )
    .await;
    c.version(&cdn, "vc1", "C", "release", &[]).await;
    // vb0 只作为已安装的旧版本出现。
    c.project_versions.insert("B".into(), vec!["vb1".into()]);
    let old = c.files["vb0.jar"].clone();
    let expected_c = c.files["vc1.jar"].clone();
    let api = api(c, &files).await;

    let tmp = tempfile::tempdir().unwrap();
    let mods = tmp.path().join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("b-old.jar"), &old).unwrap();

    let plan = installer(&api, tmp.path())
        .install(&request("vb1", ResourceKind::Mod, None), &ctx())
        .await
        .unwrap();
    assert_eq!(plan.files[0].replaces.as_deref(), Some("b-old.jar"));
    assert_eq!(plan.files[1].replaces, None);
    assert!(mods.join("vb1.jar").is_file());
    assert_eq!(std::fs::read(mods.join("vc1.jar")).unwrap(), expected_c);
    assert!(!mods.join("b-old.jar").exists());
}

#[tokio::test]
async fn datapacks_go_to_the_world_and_errors_are_reported() {
    let (cdn, files) = cdn().await;
    let mut c = Catalog::default();
    c.project("DP", "mod", &["datapack"]);
    c.version(&cdn, "vdp", "DP", "release", &[]).await;
    let api = api(c, &files).await;

    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("saves/W")).unwrap();
    let installer = installer(&api, tmp.path());
    let plan = installer
        .install(&request("vdp", ResourceKind::Datapack, Some("W")), &ctx())
        .await
        .unwrap();
    assert_eq!(plan.files[0].kind, ResourceKind::Datapack);
    assert!(tmp.path().join("saves/W/datapacks/vdp.jar").is_file());

    let err = installer
        .plan(
            &request("vdp", ResourceKind::Datapack, Some("Missing")),
            &ctx(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesWorldNotFound);
    let err = installer
        .plan(&request("nope", ResourceKind::Mod, None), &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesVersionNotFound);
    assert_eq!(err.params["version"], "nope");
}
