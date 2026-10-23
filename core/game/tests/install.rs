//! 版本 JSON 解析与文件补全的离线测试。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use sha1::{Digest, Sha1};
use tmcl_common::{DataPaths, ErrorCode, TaskCtx};
use tmcl_game::install::library_items;
use tmcl_game::{GameFiles, InstallOptions, RuleEnv, install_version, parse_version};
use tmcl_net::testing::TestServer;
use tmcl_net::{DownloadOptions, Downloader, HttpClient};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/versions")
        .join(name);
    std::fs::read(path).unwrap()
}

fn env(os: &'static str) -> RuleEnv {
    RuleEnv {
        os_name: os,
        arch: "x86_64",
        features: BTreeMap::new(),
    }
}

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn parses_1_20_1_fixture() {
    let version = parse_version("1.20.1", &fixture("1.20.1.json")).unwrap();
    assert_eq!(version.id, "1.20.1");
    assert_eq!(version.kind, "release");
    assert_eq!(version.main_class, "net.minecraft.client.main.Main");
    assert_eq!(version.java_version.as_ref().unwrap().major_version, 17);
    assert_eq!(version.asset_index.as_ref().unwrap().id, "5");
    let args = version.arguments.as_ref().unwrap();
    assert_eq!((args.game.len(), args.jvm.len()), (28, 11));
    assert_eq!(
        version
            .logging
            .as_ref()
            .unwrap()
            .client
            .as_ref()
            .unwrap()
            .file
            .id,
        "client-1.12.xml"
    );

    let files = GameFiles::new(DataPaths::at("root"));
    // 客户端 jar + 适用的库 + 日志配置。
    for (os, libs) in [("windows", 64), ("osx", 58), ("linux", 52)] {
        assert_eq!(
            library_items(&files, &version, &env(os)).len(),
            libs + 2,
            "{os}"
        );
    }
}

#[test]
fn invalid_version_json_is_reported() {
    let err = parse_version("1.20.1", br#"{"id": "1.20.1"}"#).unwrap_err();
    assert_eq!(err.code, ErrorCode::GameVersionJsonInvalid);
    assert_eq!(err.params["version"], "1.20.1");
}

/// 构造一个指向本地服务器的最小版本，覆盖 jar、库、日志配置、资源索引与资源文件。
async fn fake_release() -> (TestServer, InstallOptions) {
    let server = TestServer::start(Vec::<(String, Vec<u8>)>::new()).await;
    let base = server.url("");
    let add = |path: &str, data: &[u8]| {
        server.route(path, data.to_vec());
        json!({"sha1": sha1_hex(data), "size": data.len(), "url": format!("{base}{path}")})
    };

    let (a, b) = (b"asset a".to_vec(), b"asset b".to_vec());
    let (ha, hb) = (sha1_hex(&a), sha1_hex(&b));
    add(&format!("/res/{}/{ha}", &ha[..2]), &a);
    add(&format!("/res/{}/{hb}", &hb[..2]), &b);
    let index = serde_json::to_vec(&json!({"objects": {
        "a.ogg": {"hash": ha, "size": a.len()},
        "b.png": {"hash": hb, "size": b.len()},
        "b-copy.png": {"hash": hb, "size": b.len()},
    }}))
    .unwrap();
    let mut index_ref = add("/index.json", &index);
    index_ref["id"] = json!("fake-index");
    let client = add("/client.jar", b"client jar");
    let mut lib = add("/lib/a/lib/1/lib-1.jar", b"library");
    lib["path"] = json!("a/lib/1/lib-1.jar");
    let mut mac = add("/lib/a/mac/1/mac-1.jar", b"mac only");
    mac["path"] = json!("a/mac/1/mac-1.jar");
    let mut log = add("/log.xml", b"<Configuration/>");
    log["id"] = json!("client.xml");

    let version = serde_json::to_vec(&json!({
        "id": "fake", "type": "release", "mainClass": "Main",
        "arguments": {"game": [], "jvm": []},
        "assetIndex": index_ref,
        "assets": "fake-index",
        "downloads": {"client": client},
        "libraries": [
            {"name": "a:lib:1", "downloads": {"artifact": lib}},
            {"name": "a:mac:1", "downloads": {"artifact": mac},
             "rules": [{"action": "allow", "os": {"name": "nowhere"}}]},
        ],
        "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}", "type": "log4j2-xml", "file": log}},
    }))
    .unwrap();
    let entry = add("/fake.json", &version);
    let manifest = serde_json::to_vec(&json!({
        "latest": {"release": "fake", "snapshot": "fake"},
        "versions": [{"id": "fake", "type": "release", "url": entry["url"],
            "time": "", "releaseTime": "", "sha1": entry["sha1"]}],
    }))
    .unwrap();
    add("/manifest.json", &manifest);

    let options = InstallOptions {
        manifest_url: server.url("/manifest.json"),
        resources_url: server.url("/res"),
    };
    (server, options)
}

fn downloader(concurrency: usize) -> Downloader {
    let options = DownloadOptions {
        concurrency,
        ..DownloadOptions::default()
    };
    Downloader::new(HttpClient::new().unwrap(), options)
}

#[tokio::test]
async fn installs_then_skips_everything_on_second_run() {
    let (server, options) = fake_release().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let engine = downloader(4);
    let ctx = TaskCtx::noop("game_install");

    let report = install_version(&engine, &paths, "fake", &options, &ctx)
        .await
        .unwrap();
    // 版本 JSON、索引、jar、库、日志配置、两个去重后的资源；mac 专用库被规则排除。
    assert_eq!((report.downloaded, report.skipped), (7, 0));
    let files = GameFiles::new(paths.clone());
    assert!(files.client_jar("fake").is_file());
    assert!(files.library("a/lib/1/lib-1.jar").is_file());
    assert!(!files.library("a/mac/1/mac-1.jar").exists());
    assert!(files.log_config("client.xml").is_file());
    assert!(files.asset_index("fake-index").is_file());
    let hits = server.hits();

    let report = install_version(&engine, &paths, "fake", &options, &ctx)
        .await
        .unwrap();
    assert_eq!((report.downloaded, report.skipped), (0, 7));
    assert_eq!(server.hits(), hits + 1, "第二次只应请求版本清单");
}

#[tokio::test]
async fn unknown_version_is_reported() {
    let (_server, options) = fake_release().await;
    let dir = tempfile::tempdir().unwrap();
    let err = install_version(
        &downloader(4),
        &DataPaths::at(dir.path()),
        "1.0.0",
        &options,
        &TaskCtx::noop("game_install"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::GameVersionNotFound);
}

#[tokio::test]
async fn install_respects_downloader_connection_limit() {
    let (server, options) = fake_release().await;
    server.set_delay(Duration::from_millis(50));
    let dir = tempfile::tempdir().unwrap();

    install_version(
        &downloader(1),
        &DataPaths::at(dir.path()),
        "fake",
        &options,
        &TaskCtx::noop("game_install"),
    )
    .await
    .unwrap();
    assert_eq!(server.max_active(), 1);
}
