//! Fabric / Quilt 安装与版本列表的离线测试。

mod common;

use common::{GAME, add, fake_server, installer};
use serde_json::json;
use tmcl_common::{DataPaths, ErrorCode, TaskCtx};
use tmcl_game::{GameFiles, RuleEnv, library_files, resolve};
use tmcl_loader::{Endpoints, LoaderKind, fabric, quilt, version_id};
use tmcl_net::Sources;

fn profile(id: &str, server: &tmcl_net::testing::TestServer) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "id": id,
        "inheritsFrom": GAME,
        "type": "release",
        "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "arguments": {"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]},
        "libraries": [
            {"name": "net.fabricmc:fabric-loader:0.16.5", "url": server.url("/maven/")},
            // 与原版同名的库：继承合并时子版本优先。
            {"name": "a:lib:2", "url": server.url("/maven/")},
        ],
    }))
    .unwrap()
}

#[tokio::test]
async fn fabric_installs_inherited_version_json() {
    let server = fake_server().await;
    let loader = add(
        &server,
        "/maven/net/fabricmc/fabric-loader/0.16.5/fabric-loader-0.16.5.jar",
        b"loader",
    );
    add(&server, "/maven/a/lib/2/lib-2.jar", b"library 2");
    server.route(
        format!("/fabric/v2/versions/loader/{GAME}/0.16.5/profile/json"),
        profile("fabric-loader-0.16.5-1.20.1", &server),
    );
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());

    let installed = fabric::install(
        &installer(&server, &paths),
        GAME,
        "0.16.5",
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap();
    assert_eq!(installed.version_id, "fabric-loader-0.16.5-1.20.1");
    // 原版：版本 JSON、资源索引、jar、库；加载器：两个库。
    assert_eq!(installed.report.downloaded, 6);

    let files = GameFiles::new(paths.clone());
    let version = resolve(&installed.version_id, |id| files.load_version(id)).unwrap();
    assert_eq!(version.jar_id(), GAME);
    assert_eq!(
        version.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
    );
    let all = library_files(&files, &version, &RuleEnv::current());
    let names: Vec<_> = all
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        ["1.20.1.jar", "fabric-loader-0.16.5.jar", "lib-2.jar"]
    );
    assert_eq!(
        std::fs::read(files.library("net/fabricmc/fabric-loader/0.16.5/fabric-loader-0.16.5.jar"))
            .unwrap()
            .len() as u64,
        loader["size"].as_u64().unwrap()
    );

    // 第二次安装不重复下载。
    let again = fabric::install(
        &installer(&server, &paths),
        GAME,
        "0.16.5",
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap();
    assert_eq!(again.report.downloaded, 0);
}

#[tokio::test]
async fn quilt_rewrites_id_and_uses_v3_meta() {
    let server = fake_server().await;
    add(
        &server,
        "/maven/net/fabricmc/fabric-loader/0.16.5/fabric-loader-0.16.5.jar",
        b"loader",
    );
    add(&server, "/maven/a/lib/2/lib-2.jar", b"library 2");
    // 元数据给出的 id 与本地命名不同，安装时改写。
    server.route(
        format!("/quilt/v3/versions/loader/{GAME}/0.26.0-beta.1/profile/json"),
        profile("whatever", &server),
    );
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let installed = quilt::install(
        &installer(&server, &paths),
        GAME,
        "0.26.0-beta.1",
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap();
    assert_eq!(installed.version_id, "quilt-loader-0.26.0-beta.1-1.20.1");
    let files = GameFiles::new(paths);
    assert_eq!(
        files.load_version(&installed.version_id).unwrap().id,
        installed.version_id
    );
}

#[tokio::test]
async fn missing_profile_is_version_not_found_and_writes_nothing() {
    let server = fake_server().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let err = fabric::install(
        &installer(&server, &paths),
        GAME,
        "9.9.9",
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::LoaderVersionNotFound);
    assert_eq!(err.params["kind"], "Fabric");
    assert_eq!(err.params["version"], "9.9.9");
    let id = version_id(LoaderKind::Fabric, GAME, "9.9.9");
    assert!(!GameFiles::new(paths).version_json(&id).exists());
}

#[tokio::test]
async fn failed_library_download_leaves_no_version_json() {
    let server = fake_server().await;
    // 只登记了 profile，库下载会 404。
    server.route(
        format!("/fabric/v2/versions/loader/{GAME}/0.16.5/profile/json"),
        profile("fabric-loader-0.16.5-1.20.1", &server),
    );
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let err = fabric::install(
        &installer(&server, &paths),
        GAME,
        "0.16.5",
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    let files = GameFiles::new(paths);
    assert!(!files.version_json("fabric-loader-0.16.5-1.20.1").exists());
    // 原版已经补全，可以单独使用。
    assert!(files.version_json(GAME).exists());
}

#[tokio::test]
async fn profile_inheriting_another_game_version_is_rejected() {
    let server = fake_server().await;
    server.route(
        format!("/fabric/v2/versions/loader/{GAME}/0.16.5/profile/json"),
        serde_json::to_vec(&json!({"id": "x", "inheritsFrom": "1.19.4", "libraries": []})).unwrap(),
    );
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let err = fabric::install(
        &installer(&server, &paths),
        GAME,
        "0.16.5",
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::LoaderInstallerInvalid);
}

#[tokio::test]
async fn cancelled_install_writes_nothing() {
    let server = fake_server().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let ctx = TaskCtx::noop("t");
    ctx.cancel.cancel();
    let err = fabric::install(&installer(&server, &paths), GAME, "0.16.5", &ctx)
        .await
        .unwrap_err();
    assert!(err.is_cancelled());
    assert!(
        !GameFiles::new(paths)
            .version_json("fabric-loader-0.16.5-1.20.1")
            .exists()
    );
}

#[test]
fn bmclapi_serves_fabric_forge_neoforge_but_not_quilt() {
    let endpoints = Endpoints::default();
    let sources = Sources::default();
    let first = |url: String| sources.candidates(&url)[0].source.clone();
    assert_eq!(
        first(fabric::profile_url(&endpoints.fabric_meta, GAME, "0.16.5")),
        "bmclapi-loaders"
    );
    assert_eq!(
        first(format!(
            "{}/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar",
            endpoints.forge_maven
        )),
        "bmclapi-loaders"
    );
    assert_eq!(
        first(format!(
            "{}/net/neoforged/neoforge/maven-metadata.xml",
            endpoints.neoforge_maven
        )),
        "bmclapi-loaders"
    );
    assert_eq!(
        first(quilt::profile_url(&endpoints.quilt_meta, GAME, "0.26.4")),
        "official"
    );
}
