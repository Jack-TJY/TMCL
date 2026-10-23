//! 超时的离线测试：挂起、卡住或极慢的请求要在时限内失败，并交给重试续传。

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use sha1::{Digest, Sha1};
use tmcl_common::{AppResult, ErrorCode, TaskCtx};
use tmcl_net::testing::{Fault, TestServer};
use tmcl_net::{
    DownloadItem, DownloadOptions, Downloader, HttpClient, Outcome, RetryPolicy, Timeouts,
};

/// 测试中所有超时都很短；任何用例超过这个时间都说明请求挂住了。
const GUARD: Duration = Duration::from_secs(10);

fn short() -> Timeouts {
    Timeouts {
        connect: Duration::from_millis(500),
        response: Duration::from_millis(300),
        read_idle: Duration::from_millis(300),
        speed_window: Duration::from_millis(300),
        min_speed: 1000,
        total: Duration::from_millis(500),
    }
}

fn engine(timeouts: Timeouts, retry: RetryPolicy) -> Downloader {
    Downloader::new(
        HttpClient::with_timeouts(timeouts).unwrap(),
        DownloadOptions {
            retry,
            ..DownloadOptions::default()
        },
    )
}

fn retry() -> RetryPolicy {
    RetryPolicy::with_delays([Duration::from_millis(10); 3])
}

fn item(server: &TestServer, dir: &Path, body: &[u8]) -> DownloadItem {
    DownloadItem {
        url: server.url("/f"),
        path: dir.join("f"),
        size: Some(body.len() as u64),
        sha1: Some(
            Sha1::digest(body)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        ),
    }
}

async fn download(engine: &Downloader, item: &DownloadItem) -> AppResult<Outcome> {
    tokio::time::timeout(GUARD, engine.download_one(item, &TaskCtx::noop("test")))
        .await
        .expect("请求挂住了")
}

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 7 % 253) as u8).collect()
}

#[tokio::test]
async fn missing_response_headers_time_out_and_are_retried() {
    let data = body(2_000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    server.fail("/f", [Fault::Hang]);
    let dir = tempfile::tempdir().unwrap();

    let outcome = download(&engine(short(), retry()), &item(&server, dir.path(), &data)).await;

    assert_eq!(outcome.unwrap(), Outcome::Downloaded(2_000));
    assert_eq!(server.hits(), 2);
}

#[tokio::test]
async fn missing_response_headers_report_timeout() {
    let data = body(2_000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    server.fail("/f", [Fault::Hang]);
    let dir = tempfile::tempdir().unwrap();

    let engine = engine(short(), RetryPolicy::none());
    let err = download(&engine, &item(&server, dir.path(), &data))
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::NetTimeout);
    assert_eq!(err.params.get("url"), Some(&server.url("/f")));
}

#[tokio::test]
async fn stalled_body_times_out_and_resumes() {
    let data = body(50_000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    server.fail("/f", [Fault::Stall(20_000)]);
    let dir = tempfile::tempdir().unwrap();

    let outcome = download(&engine(short(), retry()), &item(&server, dir.path(), &data)).await;

    assert_eq!(outcome.unwrap(), Outcome::Downloaded(50_000));
    let ranges: Vec<_> = server.requests().into_iter().map(|r| r.range).collect();
    assert_eq!(ranges, [None, Some("bytes=20000-".to_owned())]);
}

#[tokio::test]
async fn trickling_body_below_min_speed_times_out() {
    let data = body(2_000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    // 每 20 ms 一个字节，约 50 字节/秒，远低于 1000 字节/秒。
    server.fail("/f", [Fault::Trickle(Duration::from_millis(20))]);
    let dir = tempfile::tempdir().unwrap();

    let engine = engine(short(), RetryPolicy::none());
    let err = download(&engine, &item(&server, dir.path(), &data))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetTimeout);

    // 故障用完后重新下载，从已收到的字节续传。
    let outcome = download(&engine, &item(&server, dir.path(), &data)).await;
    assert_eq!(outcome.unwrap(), Outcome::Downloaded(2_000));
    let second = server.requests()[1].range.clone().unwrap();
    assert_ne!(second, "bytes=0-");
}

#[tokio::test]
async fn min_speed_zero_disables_speed_check() {
    let data = body(30);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    server.fail("/f", [Fault::Trickle(Duration::from_millis(20))]);
    let dir = tempfile::tempdir().unwrap();

    let timeouts = Timeouts {
        min_speed: 0,
        ..short()
    };
    let engine = engine(timeouts, RetryPolicy::none());
    let outcome = download(&engine, &item(&server, dir.path(), &data)).await;

    assert_eq!(outcome.unwrap(), Outcome::Downloaded(30));
    assert_eq!(server.hits(), 1);
}

#[tokio::test]
async fn get_bytes_has_total_limit() {
    let server = TestServer::start(HashMap::from([("/meta.json", body(200))])).await;
    // 200 字节 × 20 ms ≈ 4 秒，超过 500 ms 的总时限；关闭低速检查，只看总时限。
    server.fail("/meta.json", [Fault::Trickle(Duration::from_millis(20))]);
    let client = HttpClient::with_timeouts(Timeouts {
        min_speed: 0,
        ..short()
    })
    .unwrap();

    let started = std::time::Instant::now();
    let err = client
        .get_bytes(&server.url("/meta.json"), &TaskCtx::noop("test"))
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::NetTimeout);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn defaults_bound_every_stage() {
    let t = Timeouts::default();
    for limit in [t.connect, t.response, t.read_idle, t.speed_window, t.total] {
        assert!(!limit.is_zero() && limit <= Duration::from_secs(120));
    }
    assert!(t.min_speed > 0);
    assert_eq!(HttpClient::new().unwrap().timeouts(), t);
}
