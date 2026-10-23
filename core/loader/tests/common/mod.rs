//! 加载器离线测试共用的本地服务器端点与下载引擎。

#![allow(dead_code)]

use tmcl_loader::Endpoints;
use tmcl_net::testing::TestServer;
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};

/// 本测试使用的游戏版本号。
pub const GAME: &str = "1.20.1";

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
