//! 版本清单的 SQLite 缓存：未过期直接用、过期重新获取、获取失败退回旧缓存。

use std::time::Duration;

use tmcl_common::{Db, ErrorCode, TaskCtx};
use tmcl_game::{ManifestPolicy, ManifestStore, VersionType, load_manifest, load_manifest_checked};
use tmcl_net::testing::{Fault, TestServer};
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};

fn manifest(release: &str) -> Vec<u8> {
    format!(
        r#"{{"latest": {{"release": "{release}", "snapshot": "{release}"}},
            "versions": [{{"id": "{release}", "type": "release", "url": "https://example.invalid/{release}.json",
                          "time": "", "releaseTime": "2023-06-12T13:25:51+00:00",
                          "sha1": "38d412af3b247e485c7f7e50ac25febc4e79ab1d"}}]}}"#
    )
    .into_bytes()
}

fn downloader() -> Downloader {
    Downloader::new(
        HttpClient::new().unwrap(),
        DownloadOptions {
            retry: RetryPolicy::none(),
            ..DownloadOptions::default()
        },
    )
}

fn expired() -> ManifestPolicy {
    ManifestPolicy {
        max_age: Duration::ZERO,
        refresh: false,
    }
}

#[tokio::test]
async fn fresh_cache_skips_network() {
    let server = TestServer::start([("/m.json", manifest("1.20.1"))]).await;
    let db = Db::open_in_memory().unwrap();
    let url = server.url("/m.json");
    let ctx = TaskCtx::noop("test");
    let dl = downloader();

    let first = load_manifest(&dl, &db, &url, ManifestPolicy::default(), &ctx)
        .await
        .unwrap();
    assert_eq!(first.latest.release, "1.20.1");
    assert_eq!(first.of_kind(VersionType::Release).count(), 1);
    assert_eq!(server.hits(), 1);

    server.route("/m.json", manifest("1.21"));
    let second = load_manifest(&dl, &db, &url, ManifestPolicy::default(), &ctx)
        .await
        .unwrap();
    assert_eq!(second, first);
    assert_eq!(server.hits(), 1);

    // 强制刷新会重新获取并更新缓存。
    let refresh = ManifestPolicy {
        refresh: true,
        ..ManifestPolicy::default()
    };
    let third = load_manifest(&dl, &db, &url, refresh, &ctx).await.unwrap();
    assert_eq!(third.latest.release, "1.21");
    assert_eq!(server.hits(), 2);
    let (body, _) = db.load_manifest(&url).unwrap().unwrap();
    assert!(body.contains("1.21"));
}

#[tokio::test]
async fn expired_cache_is_refetched() {
    let server = TestServer::start([("/m.json", manifest("1.20.1"))]).await;
    let db = Db::open_in_memory().unwrap();
    let url = server.url("/m.json");
    let ctx = TaskCtx::noop("test");
    let dl = downloader();

    load_manifest(&dl, &db, &url, expired(), &ctx)
        .await
        .unwrap();
    server.route("/m.json", manifest("1.21"));
    let next = load_manifest(&dl, &db, &url, expired(), &ctx)
        .await
        .unwrap();
    assert_eq!(next.latest.release, "1.21");
    assert_eq!(server.hits(), 2);
}

#[tokio::test]
async fn falls_back_to_stale_cache_when_offline() {
    let server = TestServer::start([("/m.json", manifest("1.20.1"))]).await;
    let db = Db::open_in_memory().unwrap();
    let url = server.url("/m.json");
    let ctx = TaskCtx::noop("test");
    let dl = downloader();

    load_manifest(&dl, &db, &url, expired(), &ctx)
        .await
        .unwrap();
    server.fail("/m.json", [Fault::Status(503)]);
    let stale = load_manifest_checked(&dl, &db, &url, expired(), &ctx)
        .await
        .unwrap();
    assert_eq!(stale.manifest.latest.release, "1.20.1");
    assert_eq!(
        stale.stale.map(|e| e.code),
        Some(tmcl_common::ErrorCode::NetHttpStatus)
    );
    assert_eq!(server.hits(), 2);

    // 服务器返回了无法解析的内容时同样退回缓存，并且不会覆盖缓存。
    server.route("/m.json", b"not json".to_vec());
    let stale = load_manifest(&dl, &db, &url, expired(), &ctx)
        .await
        .unwrap();
    assert_eq!(stale.latest.release, "1.20.1");
    assert!(
        db.load_manifest(&url)
            .unwrap()
            .unwrap()
            .0
            .contains("1.20.1")
    );
}

#[tokio::test]
async fn no_cache_and_offline_is_error() {
    let server = TestServer::start([("/m.json", manifest("1.20.1"))]).await;
    let db = Db::open_in_memory().unwrap();
    let url = server.url("/m.json");
    server.fail("/m.json", [Fault::Status(500)]);
    let err = load_manifest(
        &downloader(),
        &db,
        &url,
        ManifestPolicy::default(),
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    assert_eq!(db.load_manifest(&url).unwrap(), None);
}

#[tokio::test]
async fn corrupt_cache_is_ignored() {
    let server = TestServer::start([("/m.json", manifest("1.20.1"))]).await;
    let db = Db::open_in_memory().unwrap();
    let url = server.url("/m.json");
    db.save_manifest(&url, "{broken").unwrap();
    let m = load_manifest(
        &downloader(),
        &db,
        &url,
        ManifestPolicy::default(),
        &TaskCtx::noop("t"),
    )
    .await
    .unwrap();
    assert_eq!(m.latest.release, "1.20.1");
    assert_eq!(server.hits(), 1);
}
