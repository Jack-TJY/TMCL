//! 把实例导出为 CurseForge 整合包的离线测试：按指纹查询由本地模拟服务器应答，
//! 导出的包再用 [`curseforge::read`] 读回，确认能被导入。

#[allow(dead_code)] // 与 modrinth.rs 共用，部分辅助函数只在那边使用。
mod support;

use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use support::{MockServer, Response};
use tmcl_common::{ErrorCode, TaskCtx};
use tmcl_instance::{LoaderKind, LoaderSpec};
use tmcl_net::{HttpClient, RetryPolicy};
use tmcl_resources::curseforge::{CurseForgeClient, fingerprint};
use tmcl_resources::modpack::curseforge;
use tmcl_resources::modpack::export::{self, ModpackExport};

const KNOWN: &[u8] = b"a mod that is on curseforge";

fn client(api: &MockServer) -> CurseForgeClient {
    CurseForgeClient::with_base(&HttpClient::new().unwrap(), &api.url("/v1"), "test-api-key")
        .unwrap()
        .with_retry(RetryPolicy::with_delays([Duration::ZERO; 3]))
}

/// 只认识 [`KNOWN`] 的模拟 API（项目 100、文件 200）。
async fn api() -> MockServer {
    let print = fingerprint::of_bytes(KNOWN);
    let file = json!({
        "id": 200, "modId": 100, "fileName": "known.jar", "releaseType": 1,
        "fileFingerprint": print, "isAvailable": true,
    });
    let body = json!({"data": {
        "exactMatches": [{"id": 100, "file": file, "latestFiles": []}],
        "unmatchedFingerprints": null,
    }})
    .to_string()
    .into_bytes();
    MockServer::start(move |req, _| {
        if req.method == "POST" && req.path() == "/v1/fingerprints/432" {
            Response::json(&body)
        } else {
            Response::status(404)
        }
    })
    .await
}

fn write(game: &Path, rel: &str, body: &[u8]) {
    let path = game.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn game_dir() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path();
    write(game, "mods/known.jar", KNOWN);
    write(game, "mods/local.jar", b"only on this computer");
    write(game, "config/jei.toml", b"x = 1");
    write(game, "saves/World/level.dat", b"save");
    tmp
}

fn options(include: &[&str]) -> ModpackExport {
    ModpackExport {
        name: "My Pack".into(),
        version: "1.0.0".into(),
        summary: None,
        game_version: "1.20.1".into(),
        loader: Some(LoaderSpec {
            kind: LoaderKind::Forge,
            version: "47.2.0".into(),
        }),
        include: include.iter().map(|s| (*s).to_owned()).collect(),
    }
}

fn entry_names(pack: &Path) -> Vec<String> {
    let archive = zip::ZipArchive::new(fs::File::open(pack).unwrap()).unwrap();
    let mut names: Vec<String> = archive
        .file_names()
        .map(|n| n.unwrap().into_owned())
        .filter(|n| !n.ends_with('/'))
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn known_files_go_to_manifest_and_rest_to_overrides() {
    let api = api().await;
    let game = game_dir();
    let out = tempfile::tempdir().unwrap();
    let dest = out.path().join("pack.zip");
    let ctx = TaskCtx::noop("export");
    let summary = export::export_curseforge(
        &client(&api),
        game.path(),
        &dest,
        &options(&["mods", "config"]),
        &ctx,
    )
    .await
    .unwrap();
    assert_eq!((summary.linked, summary.overrides), (1, 2));
    assert_eq!(
        entry_names(&dest),
        vec![
            "manifest.json",
            "overrides/config/jei.toml",
            "overrides/mods/local.jar",
        ]
    );

    let mut archive = zip::ZipArchive::new(fs::File::open(&dest).unwrap()).unwrap();
    let manifest: Value =
        serde_json::from_reader(archive.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(
        manifest["files"],
        json!([{"projectID": 100, "fileID": 200, "required": true}])
    );
    assert_eq!(
        manifest["minecraft"]["modLoaders"],
        json!([{"id": "forge-47.2.0", "primary": true}])
    );

    // 能被 TMCL 读回。
    let parsed = curseforge::read(&dest).unwrap().unwrap();
    assert_eq!(parsed.name, "My Pack");
    assert_eq!(parsed.files, vec![(100, 200)]);
    assert_eq!(parsed.loader.unwrap().kind, LoaderKind::Forge);

    let request = &api.requests()[0];
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    let mut sent: Vec<u64> = body["fingerprints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    sent.sort_unstable();
    let mut expected = vec![
        u64::from(fingerprint::of_bytes(KNOWN)),
        u64::from(fingerprint::of_bytes(b"only on this computer")),
    ];
    expected.sort_unstable();
    assert_eq!(sent, expected);
}

#[tokio::test]
async fn lookup_failure_leaves_no_file() {
    let api = MockServer::start(|_, _| Response::status(500)).await;
    let game = game_dir();
    let out = tempfile::tempdir().unwrap();
    let dest = out.path().join("pack.zip");
    let err = export::export_curseforge(
        &client(&api),
        game.path(),
        &dest,
        &options(&["mods"]),
        &TaskCtx::noop("export"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    assert!(fs::read_dir(out.path()).unwrap().next().is_none());
}
