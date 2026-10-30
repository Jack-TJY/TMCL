//! 从 CurseForge 安装单个资源的离线测试：API 由本地模拟服务器应答，文件由 `TestServer` 提供。

#[allow(dead_code)] // 与其他测试共用，部分辅助函数只在那边使用。
mod support;

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use support::{MockServer, Response};
use tmcl_common::{ErrorCode, TaskCtx};
use tmcl_net::testing::TestServer;
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};
use tmcl_resources::curseforge::{CurseForgeClient, fingerprint};
use tmcl_resources::modrinth::ModrinthClient;
use tmcl_resources::{
    InstallRequest, InstanceTarget, ResourceInstaller, ResourceKind, ResourcePlatform,
};

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

const A_NEW: &[u8] = b"mod a 2.0";
const A_OLD: &[u8] = b"mod a 1.0";
const LIB: &[u8] = b"library";
const OTHER: &[u8] = b"other mod";

fn file(
    id: u32,
    mod_id: u32,
    name: &str,
    release: u32,
    data: &[u8],
    url: &str,
    deps: Value,
) -> Value {
    json!({
        "id": id, "modId": mod_id, "displayName": format!("{name} display"), "fileName": name,
        "releaseType": release, "fileLength": data.len(), "downloadUrl": url,
        "hashes": [{"value": sha1_hex(data), "algo": 1}],
        "gameVersions": ["1.20.1", "Fabric"], "dependencies": deps,
        "fileFingerprint": fingerprint::of_bytes(data),
    })
}

struct Env {
    _dir: tempfile::TempDir,
    game: std::path::PathBuf,
    installer: ResourceInstaller,
    _servers: (MockServer, MockServer, TestServer),
}

/// `incompatible` 为真时 Mod A 声明与已安装的 Other（项目 5）不兼容。
async fn env(incompatible: bool) -> Env {
    let cdn = TestServer::start(vec![
        ("/a-2.0.jar".to_owned(), A_NEW.to_vec()),
        ("/lib-beta.jar".to_owned(), b"beta".to_vec()),
        ("/lib.jar".to_owned(), LIB.to_vec()),
    ])
    .await;
    let mut deps = vec![
        json!({"modId": 2, "relationType": 3}),
        json!({"modId": 7, "relationType": 2}),
    ];
    if incompatible {
        deps.push(json!({"modId": 5, "relationType": 5}));
    }
    let root = file(
        100,
        1,
        "a-2.0.jar",
        1,
        A_NEW,
        &cdn.url("/a-2.0.jar"),
        json!(deps),
    );
    let files = json!({"data": [root]}).to_string().into_bytes();
    let lib_files = json!({
        "data": [
            file(201, 2, "lib-beta.jar", 2, b"beta", &cdn.url("/lib-beta.jar"), json!([])),
            file(200, 2, "lib.jar", 1, LIB, &cdn.url("/lib.jar"), json!([])),
        ],
        "pagination": {"index": 0, "pageSize": 50, "resultCount": 2, "totalCount": 2},
    })
    .to_string()
    .into_bytes();
    let mods = json!({"data": [
        {"id": 1, "name": "Mod A", "slug": "mod-a"},
        {"id": 2, "name": "Lib", "slug": "lib"},
        {"id": 5, "name": "Other", "slug": "other"},
    ]})
    .to_string()
    .into_bytes();
    let matches = json!({"data": {"exactMatches": [
        {"id": 1, "file": file(90, 1, "a-1.0.jar", 1, A_OLD, "https://x/a", json!([]))},
        {"id": 5, "file": file(50, 5, "other.jar", 1, OTHER, "https://x/o", json!([]))},
    ], "unmatchedFingerprints": []}})
    .to_string()
    .into_bytes();
    let api = MockServer::start(move |req, _| match (req.method.as_str(), req.path()) {
        ("POST", "/v1/mods/files") => Response::json(&files),
        ("POST", "/v1/mods") => Response::json(&mods),
        ("GET", "/v1/mods/2/files") => Response::json(&lib_files),
        ("POST", "/v1/fingerprints/432") => Response::json(&matches),
        _ => Response::status(404),
    })
    .await;
    let modrinth_api = MockServer::start(|_, _| Response::json(b"{}")).await;

    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::write(game.join("mods/a-1.0.jar"), A_OLD).unwrap();
    std::fs::write(game.join("mods/other.jar"), OTHER).unwrap();

    let http = HttpClient::new().unwrap();
    let retry = RetryPolicy::with_delays([Duration::ZERO; 2]);
    let curseforge = CurseForgeClient::with_base(&http, &api.url("/v1"), "test-api-key")
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
        _servers: (api, modrinth_api, cdn),
    }
}

fn request() -> InstallRequest {
    InstallRequest {
        platform: ResourcePlatform::Curseforge,
        version_id: "100".into(),
        kind: ResourceKind::Mod,
        world: None,
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn installs_file_with_required_dependency_and_replaces_old_version() {
    let env = env(false).await;
    let ctx = TaskCtx::noop("test");
    let plan = env.installer.plan(&request(), &ctx).await.unwrap();
    assert!(
        plan.conflicts.is_empty() && plan.unavailable.is_empty(),
        "{plan:?}"
    );
    let files: Vec<_> = plan
        .files
        .iter()
        .map(|f| {
            (
                f.title.as_str(),
                f.version_id.as_str(),
                f.required_by.as_deref(),
                f.replaces.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        files,
        [
            ("Mod A", "100", None, Some("a-1.0.jar")),
            // 选正式版而不是更新的测试版。
            ("Lib", "200", Some("1"), None),
        ]
    );

    env.installer.install(&request(), &ctx).await.unwrap();
    assert_eq!(
        names(&env.game.join("mods")),
        ["a-2.0.jar", "lib.jar", "other.jar"]
    );
}

#[tokio::test]
async fn incompatible_installed_project_blocks_install() {
    let env = env(true).await;
    let ctx = TaskCtx::noop("test");
    let plan = env.installer.plan(&request(), &ctx).await.unwrap();
    assert_eq!(plan.conflicts.len(), 1);
    assert_eq!(plan.conflicts[0].with_title, "Other");
    assert!(plan.conflicts[0].with_installed);
    let err = env.installer.install(&request(), &ctx).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesIncompatible);
    assert_eq!(names(&env.game.join("mods")), ["a-1.0.jar", "other.jar"]);
}

#[tokio::test]
async fn curseforge_without_client_is_disabled() {
    let env = env(false).await;
    let installer = env.installer.clone().with_curseforge(None);
    let err = installer
        .plan(&request(), &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesCurseforgeDisabled);
}
