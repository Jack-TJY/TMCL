//! 把实例导出为 `.mrpack` 的离线测试：Modrinth 的按哈希查询由本地模拟服务器应答，
//! 导出的包再用 [`mrpack`] 读回，确认能被导入。

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
use tmcl_resources::modpack::export::{self, ModpackExport};
use tmcl_resources::modpack::mrpack;
use tmcl_resources::modrinth::ModrinthClient;

const KNOWN: &[u8] = b"a mod that is on modrinth";
const KNOWN_URL: &str = "https://cdn.modrinth.com/data/AAAA/versions/v1/known.jar";

fn client(api: &MockServer) -> ModrinthClient {
    ModrinthClient::with_base(&HttpClient::new().unwrap(), &api.url("/v2"))
        .unwrap()
        .with_retry(RetryPolicy::with_delays([Duration::ZERO; 3]))
}

async fn sha1(body: &[u8]) -> String {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), body).unwrap();
    tmcl_net::sha1_file(tmp.path()).await.unwrap()
}

/// 只认识 [`KNOWN`] 的模拟 API。
async fn api() -> MockServer {
    let hash = sha1(KNOWN).await;
    let version = json!({
        "id": "v1", "project_id": "AAAA", "name": "Known 1.0", "version_number": "1.0",
        "version_type": "release",
        "files": [
            {"hashes": {"sha1": "0".repeat(40), "sha512": "0".repeat(128)},
             "url": "https://cdn.modrinth.com/other.jar", "filename": "other.jar", "size": 1},
            {"hashes": {"sha1": hash, "sha512": "ab".repeat(64)},
             "url": KNOWN_URL, "filename": "known.jar", "primary": true, "size": KNOWN.len()},
        ],
    });
    let body = json!({ hash: version }).to_string().into_bytes();
    MockServer::start(move |req, _| {
        if req.method == "POST" && req.path() == "/v2/version_files" {
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
    write(game, "mods/known.jar.disabled", KNOWN);
    write(game, "config/sodium.json", b"{}");
    write(game, "options.txt", b"lang:zh_cn");
    write(game, "saves/World/level.dat", b"save");
    write(game, "logs/latest.log", b"log");
    tmp
}

fn options(include: &[&str]) -> ModpackExport {
    ModpackExport {
        name: "My Pack".into(),
        version: "1.0.0".into(),
        summary: Some("Exported".into()),
        game_version: "1.20.1".into(),
        loader: Some(LoaderSpec {
            kind: LoaderKind::Fabric,
            version: "0.16.5".into(),
        }),
        include: include.iter().map(|s| (*s).to_owned()).collect(),
    }
}

fn read_index(pack: &Path) -> Value {
    let mut archive = zip::ZipArchive::new(fs::File::open(pack).unwrap()).unwrap();
    serde_json::from_reader(archive.by_name(mrpack::INDEX_FILE).unwrap()).unwrap()
}

fn entry_names(pack: &Path) -> Vec<String> {
    let archive = zip::ZipArchive::new(fs::File::open(pack).unwrap()).unwrap();
    let mut names: Vec<String> = archive
        .file_names()
        .map(|n| n.unwrap().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn links_modrinth_files_and_puts_the_rest_in_overrides() {
    let api = api().await;
    let game = game_dir();
    let out = tempfile::tempdir().unwrap();
    let dest = out.path().join("pack.mrpack");
    let ctx = TaskCtx::noop("test");

    let summary = export::export(
        &client(&api),
        game.path(),
        &dest,
        &options(&["mods", "config", "options.txt", "missing"]),
        &ctx,
    )
    .await
    .unwrap();
    assert_eq!((summary.linked, summary.overrides), (1, 4));
    assert!(!out.path().join("pack.mrpack.part").exists());

    assert_eq!(
        entry_names(&dest),
        [
            "modrinth.index.json",
            "overrides/config/sodium.json",
            "overrides/mods/known.jar.disabled",
            "overrides/mods/local.jar",
            "overrides/options.txt",
        ]
    );
    let index = read_index(&dest);
    assert_eq!(index["formatVersion"], 1);
    assert_eq!(index["summary"], "Exported");
    assert_eq!(
        index["dependencies"],
        json!({"minecraft": "1.20.1", "fabric-loader": "0.16.5"})
    );
    let file = &index["files"][0];
    assert_eq!(file["hashes"]["sha512"], "ab".repeat(64));
    assert_eq!(file["fileSize"], KNOWN.len());

    // 导出的包能被 TMCL 自己读回并解压。
    let pack = mrpack::read(&dest).unwrap();
    assert_eq!(pack.info.name, "My Pack");
    assert_eq!(pack.info.version, "1.0.0");
    assert_eq!(pack.info.loader.unwrap().kind, LoaderKind::Fabric);
    assert_eq!(pack.files.len(), 1);
    assert_eq!(pack.files[0].path, Path::new("mods").join("known.jar"));
    assert_eq!(pack.files[0].url, KNOWN_URL);
    assert_eq!(pack.files[0].sha1, sha1(KNOWN).await);
    let target = tempfile::tempdir().unwrap();
    assert_eq!(
        mrpack::extract_overrides(&dest, target.path(), &ctx).unwrap(),
        4
    );
    assert_eq!(
        fs::read(target.path().join("mods/local.jar")).unwrap(),
        b"only on this computer"
    );
}

#[tokio::test]
async fn vanilla_pack_without_lookups_and_target_inside_game_dir() {
    // 没有需要查询的文件时不访问网络；导出到游戏目录内时不把自己打进包里。
    let api = MockServer::start(|_, _| Response::status(500)).await;
    let game = game_dir();
    let dest = game.path().join("pack.mrpack");
    write(game.path(), "pack.mrpack", b"exported last time");
    let mut pack = options(&["config", "pack.mrpack"]);
    pack.loader = None;
    pack.summary = Some("  ".into());

    let summary = export::export(
        &client(&api),
        game.path(),
        &dest,
        &pack,
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap();
    assert_eq!((summary.linked, summary.overrides), (0, 1));
    assert!(api.requests().is_empty());
    let index = read_index(&dest);
    assert_eq!(index["dependencies"], json!({"minecraft": "1.20.1"}));
    assert!(index.get("summary").is_none());
    assert_eq!(index["files"], json!([]));
}

#[tokio::test]
async fn lookup_failure_and_cancel_leave_no_file() {
    let down = MockServer::start(|_, _| Response::status(500)).await;
    let game = game_dir();
    let out = tempfile::tempdir().unwrap();
    let dest = out.path().join("pack.mrpack");

    let err = export::export(
        &client(&down),
        game.path(),
        &dest,
        &options(&["mods"]),
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);

    let api = api().await;
    let ctx = TaskCtx::noop("t");
    ctx.cancel.cancel();
    let err = export::export(&client(&api), game.path(), &dest, &options(&["mods"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.is_cancelled());
    assert_eq!(fs::read_dir(out.path()).unwrap().count(), 0);
}
