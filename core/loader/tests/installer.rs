//! Forge / NeoForge 安装器流程的离线测试（不运行真实的处理程序）。

mod common;

use std::io::{Cursor, Write};

use common::{GAME, add, fake_server, installer, sha1_hex};
use serde_json::{Value, json};
use tmcl_common::{DataPaths, ErrorCode, TaskCtx};
use tmcl_game::GameFiles;
use tmcl_loader::{JavaChoice, LoaderKind, neoforge};
use tmcl_net::testing::TestServer;
use zip::write::SimpleFileOptions;

const FORGE: &str = "47.2.0";
const MAVEN_VERSION: &str = "1.20.1-47.2.0";

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn metadata(versions: &[&str]) -> Vec<u8> {
    let items: String = versions
        .iter()
        .map(|v| format!("<version>{v}</version>"))
        .collect();
    format!("<metadata><versioning><versions>{items}</versions></versioning></metadata>")
        .into_bytes()
}

fn installer_path() -> String {
    format!("/forge/net/minecraftforge/forge/{MAVEN_VERSION}/forge-{MAVEN_VERSION}-installer.jar")
}

const PATCHED: &str = "net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-client.jar";

/// 新格式安装器：自带一个 maven 构件、一个需下载的处理程序 jar 和一个声明了输出的处理步骤。
fn new_installer(server: &TestServer, profile_minecraft: &str) -> Vec<u8> {
    let tool = zip(&[(
        "META-INF/MANIFEST.MF",
        b"Manifest-Version: 1.0\r\nMain-Class: Tool\r\n",
    )]);
    let mut tool_ref = add(
        server,
        "/forge/net/minecraftforge/binarypatcher/1.0/binarypatcher-1.0.jar",
        &tool,
    );
    tool_ref["path"] = json!("net/minecraftforge/binarypatcher/1.0/binarypatcher-1.0.jar");
    let mut extra = add(server, "/forge/a/extra/1/extra-1.jar", b"extra library");
    extra["path"] = json!("a/extra/1/extra-1.jar");
    let universal = b"forge universal";
    let version = serde_json::to_vec(&json!({
        "id": "1.20.1-forge-47.2.0",
        "inheritsFrom": GAME,
        "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
        "libraries": [
            {"name": "net.minecraftforge:forge:1.20.1-47.2.0:universal", "downloads": {"artifact": {
                "path": "net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-universal.jar",
                "url": "", "sha1": sha1_hex(universal), "size": universal.len()}}},
            {"name": "a:extra:1", "downloads": {"artifact": extra}},
        ],
    }))
    .unwrap();
    let profile = serde_json::to_vec(&json!({
        "spec": 1, "profile": "forge", "version": "1.20.1-forge-47.2.0",
        "minecraft": profile_minecraft, "json": "/version.json",
        "data": {
            "BINPATCH": {"client": "/data/client.lzma", "server": "/data/server.lzma"},
            "PATCHED": {"client": "[net.minecraftforge:forge:1.20.1-47.2.0:client]", "server": "x"},
            "PATCHED_SHA": {"client": format!("'{}'", sha1_hex(b"patched")), "server": "x"},
        },
        "processors": [
            {"sides": ["server"], "jar": "net.minecraftforge:installertools:1.0", "args": []},
            {"jar": "net.minecraftforge:binarypatcher:1.0", "classpath": [],
             "args": ["--clean", "{MINECRAFT_JAR}", "--output", "{PATCHED}", "--apply", "{BINPATCH}"],
             "outputs": {"{PATCHED}": "{PATCHED_SHA}"}},
        ],
        "libraries": [
            {"name": "net.minecraftforge:binarypatcher:1.0", "downloads": {"artifact": tool_ref}},
        ],
    }))
    .unwrap();
    zip(&[
        ("install_profile.json", &profile),
        ("version.json", &version),
        ("data/client.lzma", b"patch data"),
        (
            "maven/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-universal.jar",
            universal,
        ),
    ])
}

async fn forge_server(profile_minecraft: &str) -> TestServer {
    let server = fake_server().await;
    server.route(
        "/forge/net/minecraftforge/forge/maven-metadata.xml",
        metadata(&[MAVEN_VERSION, "1.19.4-45.0.0"]),
    );
    let jar = new_installer(&server, profile_minecraft);
    server.route(installer_path(), jar);
    server
}

fn no_java() -> JavaChoice {
    JavaChoice::Path("no-such-java".into())
}

#[tokio::test]
async fn forge_installs_with_satisfied_processor_outputs() {
    let server = forge_server(GAME).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let files = GameFiles::new(paths.clone());
    // 处理步骤的输出已存在且 SHA1 一致：跳过，不会去启动（不存在的）Java。
    let patched = files.library(PATCHED);
    std::fs::create_dir_all(patched.parent().unwrap()).unwrap();
    std::fs::write(&patched, b"patched").unwrap();

    let installed = installer(&server, &paths)
        .with_java(no_java())
        .install(LoaderKind::Forge, GAME, FORGE, &TaskCtx::noop("t"))
        .await
        .unwrap();
    assert_eq!(installed.version_id, "1.20.1-forge-47.2.0");

    let written: Value =
        serde_json::from_slice(&std::fs::read(files.version_json(&installed.version_id)).unwrap())
            .unwrap();
    assert_eq!(written["id"], "1.20.1-forge-47.2.0");
    assert_eq!(written["inheritsFrom"], GAME);
    // 安装器内的构件已解压，处理程序与额外的库已下载。
    for (path, data) in [
        (
            "net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-universal.jar",
            &b"forge universal"[..],
        ),
        ("a/extra/1/extra-1.jar", b"extra library"),
    ] {
        assert_eq!(std::fs::read(files.library(path)).unwrap(), data, "{path}");
    }
    assert!(
        files
            .library("net/minecraftforge/binarypatcher/1.0/binarypatcher-1.0.jar")
            .exists()
    );
    // 临时目录已清理。
    let leftovers = std::fs::read_dir(paths.cache_dir().join("loader"))
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0);
}

#[tokio::test]
async fn processor_failure_leaves_no_version_json() {
    let server = forge_server(GAME).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let err = installer(&server, &paths)
        .with_java(no_java())
        .install(LoaderKind::Forge, GAME, FORGE, &TaskCtx::noop("t"))
        .await
        .unwrap_err();
    // 输出不存在，需要运行处理程序，而 Java 无法启动。
    assert_eq!(err.code, ErrorCode::LaunchSpawnFailed);
    let files = GameFiles::new(paths.clone());
    assert!(!files.version_json("1.20.1-forge-47.2.0").exists());
    assert_eq!(
        std::fs::read_dir(paths.cache_dir().join("loader"))
            .map(|d| d.count())
            .unwrap_or(0),
        0
    );
}

#[tokio::test]
async fn installer_for_another_game_version_is_invalid() {
    let server = forge_server("1.19.4").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let err = installer(&server, &paths)
        .with_java(no_java())
        .install(LoaderKind::Forge, GAME, FORGE, &TaskCtx::noop("t"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::LoaderInstallerInvalid);
    assert_eq!(err.params["kind"], "Forge");
}

#[tokio::test]
async fn unknown_forge_version_and_old_game_are_rejected() {
    let server = forge_server(GAME).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let inst = installer(&server, &paths);
    let ctx = TaskCtx::noop("t");
    let err = inst
        .install(LoaderKind::Forge, GAME, "47.9.9", &ctx)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::LoaderVersionNotFound);
    let err = inst
        .install(LoaderKind::Forge, "1.6.4", "9.11.1.1345", &ctx)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::LoaderUnsupportedVersion);
}

#[tokio::test]
async fn legacy_installer_copies_universal_jar() {
    let server = fake_server().await;
    server.route(
        "/forge/net/minecraftforge/forge/maven-metadata.xml",
        metadata(&["1.20.1-1.0.0-1.20.1"]),
    );
    let mut legacy_lib = add(&server, "/forge/b/legacy/1/legacy-1.jar", b"legacy lib");
    legacy_lib["path"] = json!("b/legacy/1/legacy-1.jar");
    let profile = serde_json::to_vec(&json!({
        "install": {
            "profileName": "Forge", "target": "1.20.1-Forge1.0.0-1.20.1",
            "path": "net.minecraftforge:forge:1.20.1-1.0.0-1.20.1",
            "filePath": "forge-1.20.1-1.0.0-1.20.1-universal.jar",
            "minecraft": GAME,
        },
        "versionInfo": {
            "id": "1.20.1-Forge1.0.0-1.20.1",
            "inheritsFrom": GAME, "jar": GAME,
            "mainClass": "net.minecraft.launchwrapper.Launch",
            "minecraftArguments": "--username ${auth_player_name} --tweakClass cpw.mods.fml.common.launcher.FMLTweaker",
            "libraries": [
                {"name": "net.minecraftforge:forge:1.20.1-1.0.0-1.20.1", "url": server.url("/forge/")},
                {"name": "b:legacy:1", "url": server.url("/forge/"), "serverreq": true,
                 "checksums": ["ignored"]},
            ],
        },
    }))
    .unwrap();
    let jar = zip(&[
        ("install_profile.json", &profile),
        (
            "forge-1.20.1-1.0.0-1.20.1-universal.jar",
            b"legacy universal",
        ),
    ]);
    server.route(
        "/forge/net/minecraftforge/forge/1.20.1-1.0.0-1.20.1/forge-1.20.1-1.0.0-1.20.1-installer.jar",
        jar,
    );
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let installed = installer(&server, &paths)
        .install(LoaderKind::Forge, GAME, "1.0.0", &TaskCtx::noop("t"))
        .await
        .unwrap();
    assert_eq!(installed.version_id, "1.20.1-forge-1.0.0");
    let files = GameFiles::new(paths);
    assert_eq!(
        std::fs::read(
            files.library(
                "net/minecraftforge/forge/1.20.1-1.0.0-1.20.1/forge-1.20.1-1.0.0-1.20.1.jar"
            )
        )
        .unwrap(),
        b"legacy universal"
    );
    assert!(files.library("b/legacy/1/legacy-1.jar").exists());
    let version = files.load_version(&installed.version_id).unwrap();
    assert_eq!(version.main_class, "net.minecraft.launchwrapper.Launch");
}

#[tokio::test]
async fn neoforge_rejects_versions_of_other_games() {
    let server = fake_server().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let err = installer(&server, &paths)
        .install(LoaderKind::Neoforge, GAME, "21.1.209", &TaskCtx::noop("t"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::LoaderVersionNotFound);
    // 安装器不存在（404）同样是找不到版本。
    let err = installer(&server, &paths)
        .install(LoaderKind::Neoforge, GAME, "47.1.106", &TaskCtx::noop("t"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::LoaderVersionNotFound);
}

#[test]
fn neoforge_installer_urls() {
    let maven = "https://maven.neoforged.net/releases";
    assert_eq!(
        neoforge::installer_url(maven, "1.20.1", "47.1.106").as_deref(),
        Some(
            "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar"
        )
    );
    assert_eq!(
        neoforge::installer_url(maven, "1.21.1", "21.1.209").as_deref(),
        Some(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.209/neoforge-21.1.209-installer.jar"
        )
    );
    assert_eq!(neoforge::installer_url(maven, "1.21", "21.1.209"), None);
}
