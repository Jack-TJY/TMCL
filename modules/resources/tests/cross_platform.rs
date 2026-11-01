//! 跨平台补全必需前置的离线测试：Modrinth、CurseForge 的 API 由本地模拟服务器应答，
//! 文件由 `TestServer` 提供。

#[allow(dead_code)] // 与其他测试共用，部分辅助函数只在那边使用。
mod support;

use std::time::Duration;

use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use support::{MockServer, Request, Response};
use tmcl_common::TaskCtx;
use tmcl_net::testing::TestServer;
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};
use tmcl_resources::curseforge::{CurseForgeClient, fingerprint};
use tmcl_resources::modrinth::ModrinthClient;
use tmcl_resources::{
    InstallPlan, InstallRequest, InstanceTarget, ResourceInstaller, ResourceKind, ResourcePlatform,
};

const A_MR: &[u8] = b"mod a from modrinth";
const A_CF: &[u8] = b"mod a from curseforge";
const LIB_B: &[u8] = b"lib b from curseforge";
const LIB: &[u8] = b"lib from modrinth";

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 另一平台上是否有适用于实例的版本。
#[derive(Clone, Copy)]
struct Other {
    has_version: bool,
}

fn mr_project(id: &str, slug: &str, title: &str) -> Value {
    json!({ "id": id, "slug": slug, "project_type": "mod", "title": title,
        "client_side": "required", "server_side": "optional", "license": { "id": "MIT" },
        "loaders": ["fabric"] })
}

fn mr_version(cdn: &TestServer, id: &str, project: &str, data: &[u8], deps: Value) -> Value {
    let name = format!("{}.jar", id.to_lowercase());
    json!({ "id": id, "project_id": project, "name": id, "version_number": id,
        "date_published": "2024-01-01T00:00:00Z", "version_type": "release",
        "game_versions": ["1.20.1"], "loaders": ["fabric"],
        "files": [{ "hashes": { "sha1": sha1_hex(data) }, "url": cdn.url(&format!("/{name}")),
            "filename": name, "primary": true, "size": data.len() }],
        "dependencies": deps })
}

fn cf_file(cdn: &TestServer, id: u32, mod_id: u32, name: &str, data: &[u8], deps: Value) -> Value {
    json!({
        "id": id, "modId": mod_id, "displayName": format!("{name} display"), "fileName": name,
        "releaseType": 1, "fileLength": data.len(), "downloadUrl": cdn.url(&format!("/{name}")),
        "hashes": [{"value": sha1_hex(data), "algo": 1}],
        "gameVersions": ["1.20.1", "Fabric"], "dependencies": deps,
        "fileFingerprint": fingerprint::of_bytes(data),
    })
}

fn page(data: Vec<Value>) -> Vec<u8> {
    let n = data.len();
    json!({ "data": data,
        "pagination": {"index": 0, "pageSize": 50, "resultCount": n, "totalCount": n} })
    .to_string()
    .into_bytes()
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

struct Env {
    _dir: tempfile::TempDir,
    game: std::path::PathBuf,
    installer: ResourceInstaller,
    curseforge_api: MockServer,
    _servers: (MockServer, TestServer),
}

/// Modrinth：Mod A（`A1`）必需 Lib B，Lib B 在 Modrinth 上没有任何版本；
/// 项目 `lib`（Lib）只在 `other.has_version` 时有版本。
/// CurseForge：Mod A（文件 100）必需 Lib（项目 2），Lib 在 CurseForge 上没有文件；
/// Lib B 在 CurseForge 上叫 “LIB  b”（项目 3，短名不同），只在 `other.has_version` 时有文件。
async fn env(other: Other) -> Env {
    let cdn = TestServer::start(vec![
        ("/a1.jar".to_owned(), A_MR.to_vec()),
        ("/a-cf.jar".to_owned(), A_CF.to_vec()),
        ("/lib-b.jar".to_owned(), LIB_B.to_vec()),
        ("/lib1.jar".to_owned(), LIB.to_vec()),
    ])
    .await;

    let a1 = mr_version(
        &cdn,
        "A1",
        "AAA",
        A_MR,
        json!([{ "project_id": "BBB", "dependency_type": "required" }]),
    );
    let lib_versions = if other.has_version {
        vec![mr_version(&cdn, "LIB1", "LIBMR", LIB, json!([]))]
    } else {
        Vec::new()
    };
    let lib_versions = json!(lib_versions).to_string().into_bytes();
    let projects = [
        ("AAA", mr_project("AAA", "mod-a", "Mod A")),
        ("BBB", mr_project("BBB", "lib-b", "Lib B")),
        ("LIBMR", mr_project("LIBMR", "lib", "Lib")),
    ];
    let modrinth_api = MockServer::start(move |req, _| match (req.method.as_str(), req.path()) {
        ("GET", "/v2/version/A1") => Response::json(a1.to_string().as_bytes()),
        ("GET", "/v2/projects") => {
            let wanted = ids(req);
            let list: Vec<&Value> = projects
                .iter()
                .filter(|(id, _)| wanted.iter().any(|w| w == id))
                .map(|(_, p)| p)
                .collect();
            Response::json(json!(list).to_string().as_bytes())
        }
        ("GET", "/v2/project/lib") => Response::json(projects[2].1.to_string().as_bytes()),
        ("GET", "/v2/project/BBB/version") => Response::json(b"[]"),
        ("GET", "/v2/project/LIBMR/version") => Response::json(&lib_versions),
        ("POST", "/v2/version_files") => Response::json(b"{}"),
        _ => Response::status(404),
    })
    .await;

    let root = cf_file(
        &cdn,
        100,
        1,
        "a-cf.jar",
        A_CF,
        json!([{"modId": 2, "relationType": 3}]),
    );
    let files = json!({"data": [root]}).to_string().into_bytes();
    let mods = json!({"data": [
        {"id": 1, "name": "Mod A", "slug": "mod-a"},
        {"id": 2, "name": "Lib", "slug": "lib"},
        {"id": 3, "name": "LIB  b", "slug": "lib-b-cf"},
    ]})
    .to_string()
    .into_bytes();
    let search = page(vec![
        json!({"id": 9, "name": "Lib B Addon", "slug": "lib-b-addon"}),
        json!({"id": 3, "name": "LIB  b", "slug": "lib-b-cf"}),
    ]);
    let lib_b_files = if other.has_version {
        page(vec![cf_file(&cdn, 300, 3, "lib-b.jar", LIB_B, json!([]))])
    } else {
        page(Vec::new())
    };
    let empty = page(Vec::new());
    let curseforge_api = MockServer::start(move |req, _| match (req.method.as_str(), req.path()) {
        ("POST", "/v1/mods/files") => Response::json(&files),
        ("POST", "/v1/mods") => Response::json(&mods),
        ("GET", "/v1/mods/search") => Response::json(&search),
        ("GET", "/v1/mods/2/files") => Response::json(&empty),
        ("GET", "/v1/mods/3/files") => Response::json(&lib_b_files),
        _ => Response::status(404),
    })
    .await;

    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    std::fs::create_dir_all(game.join("mods")).unwrap();
    let http = HttpClient::new().unwrap();
    let retry = RetryPolicy::with_delays([Duration::ZERO; 2]);
    let curseforge = CurseForgeClient::with_base(&http, &curseforge_api.url("/v1"), "test-api-key")
        .unwrap()
        .with_retry(retry.clone());
    let modrinth = ModrinthClient::with_base(&http, &modrinth_api.url("/v2"))
        .unwrap()
        .with_retry(retry);
    let downloader = Downloader::new(
        http,
        DownloadOptions {
            retry: RetryPolicy::none(),
            ..DownloadOptions::default()
        },
    );
    let target = InstanceTarget {
        game_version: "1.20.1".into(),
        loader: Some("fabric".into()),
    };
    let installer = ResourceInstaller::new(modrinth, downloader, game.clone(), target)
        .with_curseforge(Some(curseforge));
    Env {
        _dir: dir,
        game,
        installer,
        curseforge_api,
        _servers: (modrinth_api, cdn),
    }
}

fn request(platform: ResourcePlatform, version_id: &str) -> InstallRequest {
    InstallRequest {
        platform,
        version_id: version_id.into(),
        kind: ResourceKind::Mod,
        world: None,
    }
}

fn files(plan: &InstallPlan) -> Vec<(&str, ResourcePlatform, Option<&str>)> {
    plan.files
        .iter()
        .map(|f| (f.title.as_str(), f.platform, f.required_by.as_deref()))
        .collect()
}

const M: ResourcePlatform = ResourcePlatform::Modrinth;
const C: ResourcePlatform = ResourcePlatform::Curseforge;

#[tokio::test]
async fn modrinth_dependency_missing_is_installed_from_curseforge() {
    let env = env(Other { has_version: true }).await;
    let ctx = TaskCtx::noop("test");
    let req = request(M, "A1");
    let plan = env.installer.plan(&req, &ctx).await.unwrap();
    assert!(
        plan.unavailable.is_empty() && plan.conflicts.is_empty(),
        "{plan:?}"
    );
    // 名称只比较字母与数字：“LIB  b” 与 “Lib B” 相同，“Lib B Addon” 不同。
    assert_eq!(
        files(&plan),
        [("Mod A", M, None), ("LIB  b", C, Some("AAA"))]
    );
    assert_eq!(plan.files[1].version_id, "300");
    let searched = env
        .curseforge_api
        .requests()
        .into_iter()
        .find(|r| r.path() == "/v1/mods/search")
        .unwrap();
    assert!(
        searched
            .query()
            .contains(&("searchFilter".into(), "Lib B".into()))
    );

    env.installer.install(&req, &ctx).await.unwrap();
    let mut names: Vec<String> = std::fs::read_dir(env.game.join("mods"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["a1.jar", "lib-b.jar"]);
}

#[tokio::test]
async fn curseforge_dependency_missing_is_installed_from_modrinth() {
    let env = env(Other { has_version: true }).await;
    let plan = env
        .installer
        .plan(&request(C, "100"), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert!(plan.unavailable.is_empty(), "{plan:?}");
    assert_eq!(files(&plan), [("Mod A", C, None), ("Lib", M, Some("1"))]);
    assert_eq!(plan.files[1].version_id, "LIB1");
}

#[tokio::test]
async fn without_curseforge_the_dependency_stays_unavailable() {
    let env = env(Other { has_version: true }).await;
    let installer = env.installer.clone().with_curseforge(None);
    let plan = installer
        .plan(&request(M, "A1"), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(files(&plan), [("Mod A", M, None)]);
    assert_eq!(plan.unavailable.len(), 1);
    assert_eq!(plan.unavailable[0].title.as_deref(), Some("Lib B"));
    assert!(env.curseforge_api.requests().is_empty());
}

#[tokio::test]
async fn no_matching_version_on_the_other_platform_stays_unavailable() {
    let env = env(Other { has_version: false }).await;
    let ctx = TaskCtx::noop("test");

    let plan = env.installer.plan(&request(M, "A1"), &ctx).await.unwrap();
    assert_eq!(files(&plan), [("Mod A", M, None)]);
    assert_eq!(plan.unavailable[0].project_id.as_deref(), Some("BBB"));
    assert_eq!(plan.unavailable[0].required_by, "AAA");

    let plan = env.installer.plan(&request(C, "100"), &ctx).await.unwrap();
    assert_eq!(files(&plan), [("Mod A", C, None)]);
    assert_eq!(plan.unavailable.len(), 1);
    assert_eq!(plan.unavailable[0].title.as_deref(), Some("Lib"));
    assert_eq!(plan.unavailable[0].required_by, "1");
}

#[tokio::test]
async fn failing_other_platform_does_not_break_the_plan() {
    let env = env(Other { has_version: true }).await;
    let down = MockServer::start(|_, _| Response::status(500)).await;
    let curseforge = CurseForgeClient::with_base(
        &HttpClient::new().unwrap(),
        &down.url("/v1"),
        "test-api-key",
    )
    .unwrap()
    .with_retry(RetryPolicy::none());
    let plan = env
        .installer
        .clone()
        .with_curseforge(Some(curseforge))
        .plan(&request(M, "A1"), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(files(&plan), [("Mod A", M, None)]);
    assert_eq!(plan.unavailable[0].title.as_deref(), Some("Lib B"));
}
