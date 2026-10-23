//! 加载器离线测试共用的本地服务器与最小原版。

#![allow(dead_code)]

use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use tmcl_common::DataPaths;
use tmcl_game::InstallOptions;
use tmcl_loader::{Endpoints, LoaderInstaller};
use tmcl_net::testing::TestServer;
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};

/// 本测试使用的游戏版本号。
pub const GAME: &str = "1.20.1";

pub fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 在服务器上登记 `path`，返回 `{sha1, size, url}`。
pub fn add(server: &TestServer, path: &str, data: &[u8]) -> Value {
    server.route(path, data.to_vec());
    json!({"sha1": sha1_hex(data), "size": data.len(), "url": server.url(path)})
}

/// 启动服务器并登记一个最小原版 [`GAME`]（客户端 jar、一个库、空资源索引）。
pub async fn fake_server() -> TestServer {
    let server = TestServer::start(Vec::<(String, Vec<u8>)>::new()).await;
    let mut index = add(&server, "/index.json", br#"{"objects":{}}"#);
    index["id"] = json!("fake-index");
    let client = add(&server, "/client.jar", b"client jar");
    let mut lib = add(&server, "/lib/a/lib/1/lib-1.jar", b"library");
    lib["path"] = json!("a/lib/1/lib-1.jar");
    let version = serde_json::to_vec(&json!({
        "id": GAME, "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "arguments": {"game": [], "jvm": []},
        "assetIndex": index, "assets": "fake-index",
        "downloads": {"client": client},
        "javaVersion": {"component": "java-runtime-gamma", "majorVersion": 17},
        "libraries": [{"name": "a:lib:1", "downloads": {"artifact": lib}}],
    }))
    .unwrap();
    let entry = add(&server, "/game.json", &version);
    let manifest = serde_json::to_vec(&json!({
        "latest": {"release": GAME, "snapshot": GAME},
        "versions": [{"id": GAME, "type": "release", "url": entry["url"],
            "time": "", "releaseTime": "", "sha1": entry["sha1"]}],
    }))
    .unwrap();
    add(&server, "/manifest.json", &manifest);
    server
}

/// 指向本地服务器的端点：`/fabric`、`/quilt`、`/forge`、`/forge-files`、`/neoforge`、`/bmclapi`。
pub fn endpoints(server: &TestServer) -> Endpoints {
    Endpoints {
        fabric_meta: server.url("/fabric"),
        quilt_meta: server.url("/quilt"),
        forge_maven: server.url("/forge"),
        forge_files: server.url("/forge-files"),
        neoforge_maven: server.url("/neoforge"),
        bmclapi: server.url("/bmclapi"),
    }
}

/// 不重试的下载引擎，让失败路径的测试快速结束。
pub fn downloader() -> Downloader {
    let options = DownloadOptions {
        retry: RetryPolicy::none(),
        ..DownloadOptions::default()
    };
    Downloader::new(HttpClient::new().unwrap(), options)
}

pub fn installer(server: &TestServer, paths: &DataPaths) -> LoaderInstaller {
    LoaderInstaller::new(downloader(), paths.clone())
        .with_endpoints(endpoints(server))
        .with_game_options(InstallOptions {
            manifest_url: server.url("/manifest.json"),
            resources_url: server.url("/res"),
        })
}
