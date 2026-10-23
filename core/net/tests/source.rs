//! 下载源回退的离线测试：一个本地服务器充当官方源，另一个充当镜像。

use std::time::Duration;

use sha1::{Digest, Sha1};
use tmcl_common::{ErrorCode, TaskCtx};
use tmcl_net::testing::{Fault, TestServer};
use tmcl_net::{
    DownloadItem, DownloadOptions, Downloader, HttpClient, Outcome, PrefixMirror, RetryPolicy,
    Sources,
};

const DATA: &[u8] = b"official file content";

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 官方源与镜像各一个服务器，`policy` 为下载源策略。
async fn setup(mirror_body: &[u8], policy: &str) -> (TestServer, TestServer, Downloader) {
    let official = TestServer::start([("/f.jar", DATA.to_vec())]).await;
    let mirror = TestServer::start([("/m/f.jar", mirror_body.to_vec())]).await;
    let mut sources = Sources::new(policy);
    sources.add_mirror(PrefixMirror::new(
        "test",
        [(official.url(""), mirror.url("/m"))],
    ));
    let engine = Downloader::new(
        HttpClient::new().unwrap(),
        DownloadOptions {
            retry: RetryPolicy::with_delays([Duration::from_millis(10); 2]),
            ..DownloadOptions::default()
        },
    )
    .with_sources(sources);
    (official, mirror, engine)
}

fn item(official: &TestServer, dir: &std::path::Path) -> DownloadItem {
    DownloadItem {
        url: official.url("/f.jar"),
        path: dir.join("f.jar"),
        size: Some(DATA.len() as u64),
        sha1: Some(sha1_hex(DATA)),
    }
}

#[tokio::test]
async fn mirror_is_used_first() {
    let (official, mirror, engine) = setup(DATA, "test").await;
    let dir = tempfile::tempdir().unwrap();
    let target = item(&official, dir.path());
    let outcome = engine
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(outcome, Outcome::Downloaded(DATA.len() as u64));
    assert_eq!(mirror.hits(), 1);
    assert_eq!(official.hits(), 0);
}

#[tokio::test]
async fn mirror_404_falls_back_to_official() {
    let (official, mirror, engine) = setup(DATA, "test").await;
    mirror.fail("/m/f.jar", [Fault::Status(404)]);
    let dir = tempfile::tempdir().unwrap();
    let target = item(&official, dir.path());
    engine
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), DATA);
    // 404 不可恢复，镜像只请求一次就换源。
    assert_eq!(mirror.hits(), 1);
    assert_eq!(official.hits(), 1);
}

#[tokio::test]
async fn mirror_server_error_retries_then_falls_back() {
    let (official, mirror, engine) = setup(DATA, "test").await;
    mirror.fail("/m/f.jar", [Fault::Status(503); 3]);
    let dir = tempfile::tempdir().unwrap();
    let target = item(&official, dir.path());
    engine
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), DATA);
    assert_eq!(mirror.hits(), 3);
    assert_eq!(official.hits(), 1);
}

#[tokio::test]
async fn mirror_hash_mismatch_falls_back_to_official() {
    let tampered = b"tampered file content";
    assert_eq!(tampered.len(), DATA.len());
    let (official, mirror, engine) = setup(tampered, "test").await;
    let dir = tempfile::tempdir().unwrap();
    let target = item(&official, dir.path());
    engine
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), DATA);
    assert!(mirror.hits() >= 1);
    assert_eq!(official.hits(), 1);
}

#[tokio::test]
async fn all_sources_failing_reports_official_error() {
    let (official, mirror, engine) = setup(DATA, "test").await;
    mirror.fail("/m/f.jar", [Fault::Status(404)]);
    official.fail("/f.jar", [Fault::Status(410)]);
    let dir = tempfile::tempdir().unwrap();
    let target = item(&official, dir.path());
    let err = engine
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    assert_eq!(err.params.get("status").map(String::as_str), Some("410"));
    assert_eq!(err.params.get("url"), Some(&target.url));
    assert!(!target.path.exists());
}

#[tokio::test]
async fn official_policy_skips_mirror() {
    let (official, mirror, engine) = setup(DATA, "official").await;
    let dir = tempfile::tempdir().unwrap();
    engine
        .download_one(&item(&official, dir.path()), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(mirror.hits(), 0);
    assert_eq!(official.hits(), 1);
}

#[tokio::test]
async fn get_bytes_falls_back_to_official() {
    let (official, mirror, engine) = setup(b"[1]", "test").await;
    official.route("/f.json", b"[2]".to_vec());
    mirror.route("/m/f.json", b"[1]".to_vec());
    let ctx = TaskCtx::noop("test");
    let from_mirror: Vec<u32> = engine
        .get_json(&official.url("/f.json"), &ctx)
        .await
        .unwrap();
    assert_eq!(from_mirror, [1]);
    mirror.fail("/m/f.json", [Fault::Status(404)]);
    let from_official: Vec<u32> = engine
        .get_json(&official.url("/f.json"), &ctx)
        .await
        .unwrap();
    assert_eq!(from_official, [2]);
}
