//! 版本 JSON 解析与文件补全的离线测试。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use sha1::{Digest, Sha1};
use tmcl_common::{DataPaths, ErrorCode, TaskCtx};
use tmcl_game::natives::native_jars;
use tmcl_game::{
    AssetIndex, GameFiles, InstallOptions, RuleEnv, VersionJson, game_assets_dir, install_version,
    library_files, library_items, parse_version, prepare_legacy_assets, repair_version, resolve,
    verify_version,
};
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
        os_version: None,
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

fn resolved(id: &str) -> VersionJson {
    resolve(id, |id| parse_version(id, &fixture(&format!("{id}.json")))).unwrap()
}

#[test]
fn legacy_natives_are_picked_per_os() {
    let files = GameFiles::new(DataPaths::at("root"));
    // 期望值由独立的 Python 模型按官方规则从 fixture 算出：（文件数含 jar 与日志配置，natives jar 数）。
    for (id, expected) in [
        (
            "1.7.10",
            [("windows", 35, 4), ("osx", 34, 3), ("linux", 33, 2)],
        ),
        (
            "1.12.2",
            [("windows", 37, 3), ("osx", 37, 3), ("linux", 37, 3)],
        ),
        (
            "1.16.5",
            [("windows", 43, 8), ("osx", 44, 8), ("linux", 43, 8)],
        ),
    ] {
        let version = resolved(id);
        for (os, count, natives) in expected {
            let all = library_files(&files, &version, &env(os));
            assert_eq!(all.len(), count, "{id} {os}");
            assert!(all.iter().all(|f| f.url.is_some() && f.sha1.is_some()));
            let jars = native_jars(&files, &version, &env(os));
            assert_eq!(jars.len(), natives, "{id} {os}");
            // 1.16.5 的 LWJGL 3.2 natives 没有 `extract` 字段。
            assert!(
                jars.iter()
                    .all(|j| !j.flatten && (j.exclude.is_empty() || j.exclude == ["META-INF/"]))
            );
        }
    }
    let twitch = native_jars(&files, &resolved("1.7.10"), &env("windows"));
    assert!(twitch.iter().any(|j| {
        j.path
            .ends_with("twitch-platform-5.16-natives-windows-64.jar")
    }));
}

#[test]
fn new_style_natives_are_flattened() {
    let files = GameFiles::new(DataPaths::at("root"));
    let version = resolved("1.20.1");
    // 每个系统只取 x86_64 的那一份（7 个 LWJGL 模块）；arm64 / x86 的 jar 虽通过规则也不解压。
    for os in ["windows", "osx", "linux"] {
        let jars = native_jars(&files, &version, &env(os));
        assert_eq!(jars.len(), 7, "{os}");
        assert!(jars.iter().all(|j| j.flatten));
        assert!(jars.iter().all(|j| {
            let name = j.path.file_name().unwrap().to_string_lossy();
            !name.contains("arm64") && !name.contains("x86")
        }));
    }
    let arm = RuleEnv::new("osx", "arm64");
    assert_eq!(native_jars(&files, &version, &arm).len(), 7);
}

#[test]
fn fabric_libraries_and_parent_jar_are_included() {
    let files = GameFiles::new(DataPaths::at("root"));
    let version = resolved("fabric-loader-0.20.1-1.20.1");
    let all = library_files(&files, &version, &env("windows"));
    assert_eq!(all.len(), 64 + 8 + 2);
    assert_eq!(all[0].path, files.client_jar("1.20.1"));
    let loader = all
        .iter()
        .find(|f| f.path.ends_with("fabric-loader-0.20.1.jar"))
        .unwrap();
    assert_eq!(
        loader.url.as_deref(),
        Some(
            "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"
        )
    );
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
    let mut natives = add("/lib/a/nat/1/nat-1-natives-x.jar", b"natives jar");
    natives["path"] = json!("a/nat/1/nat-1-natives-x.jar");
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
            {"name": "a:nat:1",
             "natives": {"windows": "natives-x", "osx": "natives-x", "linux": "natives-x"},
             "extract": {"exclude": ["META-INF/"]},
             "downloads": {"classifiers": {"natives-x": natives}}},
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
    // 版本 JSON、索引、jar、库、natives、日志配置、两个去重后的资源；mac 专用库被规则排除。
    assert_eq!((report.downloaded, report.skipped), (8, 0));
    let files = GameFiles::new(paths.clone());
    assert!(files.client_jar("fake").is_file());
    assert!(files.library("a/lib/1/lib-1.jar").is_file());
    assert!(!files.library("a/mac/1/mac-1.jar").exists());
    assert!(files.library("a/nat/1/nat-1-natives-x.jar").is_file());
    assert!(files.log_config("client.xml").is_file());
    assert!(files.asset_index("fake-index").is_file());
    let hits = server.hits();

    let report = install_version(&engine, &paths, "fake", &options, &ctx)
        .await
        .unwrap();
    assert_eq!((report.downloaded, report.skipped), (0, 8));
    assert_eq!(server.hits(), hits + 1, "第二次只应请求版本清单");

    // 修复：删掉一个库、改坏一个资源后再次补全，只重新下载这两个文件。
    std::fs::remove_file(files.library("a/lib/1/lib-1.jar")).unwrap();
    let hash = sha1_hex(b"asset a");
    std::fs::write(files.asset_object(&hash), b"broken").unwrap();
    let report = install_version(&engine, &paths, "fake", &options, &ctx)
        .await
        .unwrap();
    assert_eq!((report.downloaded, report.skipped), (2, 6));
    assert_eq!(
        std::fs::read(files.library("a/lib/1/lib-1.jar")).unwrap(),
        b"library"
    );
}

/// 在本地写一个继承 `fake` 的加载器版本 JSON。
fn write_loader(paths: &DataPaths, libraries: serde_json::Value) {
    let files = GameFiles::new(paths.clone());
    let path = files.version_json("loader");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let json = json!({"id": "loader", "inheritsFrom": "fake", "mainClass": "Loader",
        "libraries": libraries});
    std::fs::write(path, serde_json::to_vec(&json).unwrap()).unwrap();
}

#[tokio::test]
async fn installs_loader_version_with_parent() {
    let (server, options) = fake_release().await;
    server.route("/maven/l/loader/1/loader-1.jar", b"loader".to_vec());
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    write_loader(
        &paths,
        json!([{"name": "l:loader:1", "url": server.url("/maven/")}]),
    );

    let report = install_version(
        &downloader(4),
        &paths,
        "loader",
        &options,
        &TaskCtx::noop("game_install"),
    )
    .await
    .unwrap();
    // 父版本的 8 个文件 + 加载器库；加载器自己的 JSON 不经过清单。
    assert_eq!((report.downloaded, report.skipped), (9, 0));
    let files = GameFiles::new(paths.clone());
    assert!(files.library("l/loader/1/loader-1.jar").is_file());
    assert!(files.client_jar("fake").is_file());
    assert!(!files.client_jar("loader").exists());
}

#[tokio::test]
async fn missing_local_only_library_is_reported() {
    let (_server, options) = fake_release().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let generated = "f/client/1/client-1.jar";
    write_loader(
        &paths,
        json!([{"name": "f:client:1", "downloads": {"artifact":
            {"path": generated, "sha1": "", "size": 0, "url": ""}}}]),
    );
    let run = || {
        let paths = paths.clone();
        let options = options.clone();
        async move {
            install_version(
                &downloader(4),
                &paths,
                "loader",
                &options,
                &TaskCtx::noop("game_install"),
            )
            .await
        }
    };

    let err = run().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::GameFileMissing);
    assert_eq!(err.params["version"], "loader");

    let path = GameFiles::new(paths.clone()).library(generated);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"generated by installer").unwrap();
    let report = run().await.unwrap();
    // 第一次只下载了版本 JSON 与资源索引。
    assert_eq!(report.downloaded, 6, "缺失时不应先下载库与资源");
}

#[tokio::test]
async fn uses_local_json_when_manifest_is_unreachable() {
    let (_server, mut options) = fake_release().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let ctx = TaskCtx::noop("game_install");
    install_version(&downloader(4), &paths, "fake", &options, &ctx)
        .await
        .unwrap();

    options.manifest_url = "http://127.0.0.1:9/manifest.json".to_owned();
    let engine = Downloader::new(
        HttpClient::new().unwrap(),
        DownloadOptions {
            retry: tmcl_net::RetryPolicy::none(),
            ..DownloadOptions::default()
        },
    );
    let report = install_version(&engine, &paths, "fake", &options, &ctx)
        .await
        .unwrap();
    assert_eq!(report.downloaded, 0);

    // 本地也没有时，返回获取清单的错误。
    let err = install_version(&engine, &paths, "other", &options, &ctx)
        .await
        .unwrap_err();
    assert_ne!(err.code, ErrorCode::GameVersionNotFound);
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

#[tokio::test]
async fn verify_finds_broken_files_and_repair_fixes_them() {
    let (server, options) = fake_release().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let ctx = TaskCtx::noop("game_verify");
    install_version(&downloader(4), &paths, "fake", &options, &ctx)
        .await
        .unwrap();

    let report = verify_version(&paths, "fake", &ctx).await.unwrap();
    // jar、库、natives、日志配置、资源索引、两个资源。
    assert_eq!(report.checked, 7);
    assert!(report.is_ok());

    let files = GameFiles::new(paths.clone());
    let lib = files.library("a/lib/1/lib-1.jar");
    std::fs::remove_file(&lib).unwrap();
    std::fs::write(files.client_jar("fake"), b"tampered").unwrap();
    let hits = server.hits();
    let report = verify_version(&paths, "fake", &ctx).await.unwrap();
    let mut expected = vec![lib.clone(), files.client_jar("fake")];
    expected.sort();
    assert_eq!(report.broken, expected);
    assert_eq!(server.hits(), hits, "校验不应联网");

    let repaired = repair_version(&downloader(4), &paths, "fake", &options, &ctx)
        .await
        .unwrap();
    assert_eq!(repaired.downloaded, 2);
    assert!(verify_version(&paths, "fake", &ctx).await.unwrap().is_ok());

    // 资源索引损坏时只报告索引本身。
    std::fs::write(files.asset_index("fake-index"), b"{}").unwrap();
    let report = verify_version(&paths, "fake", &ctx).await.unwrap();
    assert_eq!(report.checked, 5);
    assert_eq!(report.broken, [files.asset_index("fake-index")]);
}

#[tokio::test]
async fn verify_requires_local_version_json() {
    let dir = tempfile::tempdir().unwrap();
    let err = verify_version(&DataPaths::at(dir.path()), "1.20.1", &TaskCtx::noop("v"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::CommonIo);
}

#[tokio::test]
async fn verify_can_be_cancelled() {
    let (_server, options) = fake_release().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let ctx = TaskCtx::noop("game_verify");
    install_version(&downloader(4), &paths, "fake", &options, &ctx)
        .await
        .unwrap();
    ctx.cancel.cancel();
    let err = verify_version(&paths, "fake", &ctx).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::CommonCancelled);
}

fn legacy_index(
    flag: &str,
    objects: &[(&str, &[u8])],
) -> (tempfile::TempDir, GameFiles, AssetIndex) {
    let dir = tempfile::tempdir().unwrap();
    let files = GameFiles::new(DataPaths::at(dir.path()));
    let mut map = serde_json::Map::new();
    for (name, data) in objects {
        let hash = sha1_hex(data);
        let path = files.asset_object(&hash);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, data).unwrap();
        map.insert(
            (*name).to_owned(),
            json!({"hash": hash, "size": data.len()}),
        );
    }
    let mut index = json!({"objects": map});
    index[flag] = json!(true);
    let index: AssetIndex = serde_json::from_value(index).unwrap();
    (dir, files, index)
}

#[test]
fn virtual_assets_are_copied_by_name() {
    let (dir, files, index) = legacy_index(
        "virtual",
        &[
            ("sound/step.ogg", b"step"),
            ("../evil", b"evil"),
            ("lang/en_US.lang", b"en"),
        ],
    );
    let game_dir = dir.path().join("game");
    let root = game_assets_dir(&files, "legacy", &index, &game_dir);
    assert_eq!(root, files.virtual_assets_dir("legacy"));

    assert_eq!(
        prepare_legacy_assets(&files, "legacy", &index, &game_dir).unwrap(),
        2
    );
    assert_eq!(std::fs::read(root.join("sound/step.ogg")).unwrap(), b"step");
    assert!(!files.assets_dir().join("virtual/evil").exists());
    // 第二次全部跳过。
    assert_eq!(
        prepare_legacy_assets(&files, "legacy", &index, &game_dir).unwrap(),
        0
    );
}

#[test]
fn pre_1_6_assets_go_to_game_resources() {
    let (dir, files, index) = legacy_index("map_to_resources", &[("music/calm1.ogg", b"calm")]);
    let game_dir = dir.path().join("game");
    assert_eq!(
        game_assets_dir(&files, "pre-1.6", &index, &game_dir),
        game_dir.join("resources")
    );
    assert_eq!(
        prepare_legacy_assets(&files, "pre-1.6", &index, &game_dir).unwrap(),
        1
    );
    assert_eq!(
        std::fs::read(game_dir.join("resources/music/calm1.ogg")).unwrap(),
        b"calm"
    );

    // 新版本索引不复制。
    let modern: AssetIndex = serde_json::from_value(json!({"objects": {}})).unwrap();
    assert_eq!(
        game_assets_dir(&files, "5", &modern, &game_dir),
        files.assets_dir()
    );
    assert_eq!(
        prepare_legacy_assets(&files, "5", &modern, &game_dir).unwrap(),
        0
    );

    // 资源文件缺失时报错。
    std::fs::remove_dir_all(files.assets_dir().join("objects")).unwrap();
    std::fs::remove_dir_all(game_dir.join("resources")).unwrap();
    let err = prepare_legacy_assets(&files, "pre-1.6", &index, &game_dir).unwrap_err();
    assert_eq!(err.code, ErrorCode::CommonIo);
}
