//! 下载模块的离线测试，使用 `tmcl_net::testing::TestServer`。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sha1::{Digest, Sha1};
use tmcl_common::{ErrorCode, TaskCtx, VecSink};
use tmcl_net::testing::{Fault, TestServer};
use tmcl_net::{
    DownloadItem, DownloadOptions, Downloader, HttpClient, Outcome, RetryPolicy, download_all,
    is_valid,
};
use tokio::net::TcpListener;

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn item(server: &TestServer, route: &str, dir: &Path, body: &[u8]) -> DownloadItem {
    DownloadItem {
        url: server.url(route),
        path: dir.join(route.trim_start_matches('/')),
        size: Some(body.len() as u64),
        sha1: Some(sha1_hex(body)),
    }
}

/// 重试间隔很短的下载引擎，避免测试等待真实的 1/2/4 秒。
fn fast(concurrency: usize) -> Downloader {
    Downloader::new(
        HttpClient::new().unwrap(),
        DownloadOptions {
            concurrency,
            retry: RetryPolicy::with_delays([Duration::from_millis(10); 3]),
            ..DownloadOptions::default()
        },
    )
}

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 % 251) as u8).collect()
}

fn ranges(server: &TestServer) -> Vec<Option<String>> {
    server.requests().into_iter().map(|r| r.range).collect()
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn downloads_verifies_and_skips_on_second_run() {
    let a = b"hello".to_vec();
    let b = vec![7u8; 100_000];
    let server = TestServer::start(HashMap::from([
        ("/a.txt", a.clone()),
        ("/sub/b.bin", b.clone()),
    ]))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let client = HttpClient::new().unwrap();
    let items = vec![
        item(&server, "/a.txt", dir.path(), &a),
        item(&server, "/sub/b.bin", dir.path(), &b),
    ];

    let sink = Arc::new(VecSink::new());
    let ctx = TaskCtx::new("test", sink.clone());
    let report = download_all(&client, items.clone(), 4, "files", &ctx)
        .await
        .unwrap();
    assert_eq!(
        (report.downloaded, report.skipped, report.bytes),
        (2, 0, 100_005)
    );
    assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), a);
    assert_eq!(std::fs::read(dir.path().join("sub/b.bin")).unwrap(), b);
    let last = sink.events().pop().unwrap();
    assert_eq!(last.stage, "files");
    assert_eq!((last.done, last.total), (2, 2));
    assert_eq!((last.bytes_done, last.bytes_total), (100_005, 100_005));
    assert_eq!(server.hits(), 2);

    let report = download_all(&client, items, 4, "files", &ctx)
        .await
        .unwrap();
    assert_eq!((report.downloaded, report.skipped), (0, 2));
    assert_eq!(server.hits(), 2, "第二次运行不应发出请求");
}

#[tokio::test]
async fn corrupted_existing_file_is_redownloaded() {
    let body = b"correct content".to_vec();
    let server = TestServer::start(HashMap::from([("/f", body.clone())])).await;
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, "/f", dir.path(), &body);
    std::fs::write(&target.path, b"wrong content!!").unwrap();
    assert!(
        !is_valid(&target.path, target.size, target.sha1.as_deref())
            .await
            .unwrap()
    );

    fast(1)
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), body);
}

#[tokio::test]
async fn persistent_hash_mismatch_fails_and_leaves_no_file() {
    let server = TestServer::start(HashMap::from([("/f", b"tampered".to_vec())])).await;
    let dir = tempfile::tempdir().unwrap();
    let mut target = item(&server, "/f", dir.path(), b"original");
    target.size = None;

    let err = fast(1)
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHashMismatch);
    assert_eq!(err.params["expected"], sha1_hex(b"original"));
    assert_eq!(server.hits(), 4, "校验失败应重下 3 次");
    assert!(file_names(dir.path()).is_empty());
}

#[tokio::test]
async fn size_mismatch_is_reported() {
    let server = TestServer::start(HashMap::from([("/f", b"12345".to_vec())])).await;
    let dir = tempfile::tempdir().unwrap();
    let target = DownloadItem {
        url: server.url("/f"),
        path: dir.path().join("f"),
        size: Some(4),
        sha1: None,
    };
    let err = fast(1)
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetSizeMismatch);
    assert!(file_names(dir.path()).is_empty());
}

#[tokio::test]
async fn not_found_is_mapped_and_not_retried() {
    let server = TestServer::start(HashMap::<String, Vec<u8>>::new()).await;
    let dir = tempfile::tempdir().unwrap();
    let items = vec![item(&server, "/missing", dir.path(), b"x")];
    let err = fast(2)
        .download_all(items, "files", &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    assert_eq!(err.params["status"], "404");
    assert_eq!(server.hits(), 1);
}

#[tokio::test]
async fn server_errors_are_retried() {
    let data = body(1000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    server.fail("/f", [Fault::Status(503), Fault::Status(500)]);
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, "/f", dir.path(), &data);
    fast(1)
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(server.hits(), 3);
    assert_eq!(std::fs::read(&target.path).unwrap(), data);
}

#[tokio::test]
async fn gives_up_after_three_retries() {
    let server = TestServer::start(HashMap::from([("/f", b"x".to_vec())])).await;
    server.fail("/f", [Fault::Status(503); 10]);
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, "/f", dir.path(), b"x");
    let err = fast(1)
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.params["status"], "503");
    assert_eq!(server.hits(), 4, "1 次请求 + 3 次重试");
}

#[tokio::test]
async fn resumes_after_mid_stream_disconnect() {
    let data = body(200_000);
    let server = TestServer::start(HashMap::from([("/big", data.clone())])).await;
    server.fail("/big", [Fault::Truncate(50_000)]);
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, "/big", dir.path(), &data);

    let outcome = fast(1)
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(outcome, Outcome::Downloaded(200_000));
    assert_eq!(std::fs::read(&target.path).unwrap(), data);
    assert_eq!(
        ranges(&server),
        vec![None, Some("bytes=50000-".to_owned())],
        "第二次请求应从断开处续传"
    );
    assert_eq!(file_names(dir.path()), vec!["big"]);
}

#[tokio::test]
async fn resumes_part_left_by_previous_run() {
    let data = body(10_000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, "/f", dir.path(), &data);
    std::fs::write(dir.path().join("f.part"), &data[..3000]).unwrap();

    let sink = Arc::new(VecSink::new());
    let ctx = TaskCtx::new("test", sink.clone());
    fast(1)
        .download_all(vec![target.clone()], "files", &ctx)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), data);
    assert_eq!(ranges(&server), vec![Some("bytes=3000-".to_owned())]);
    let last = sink.events().pop().unwrap();
    assert_eq!((last.bytes_done, last.bytes_total), (10_000, 10_000));
}

#[tokio::test]
async fn stale_part_with_wrong_content_is_discarded() {
    let data = body(10_000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, "/f", dir.path(), &data);
    std::fs::write(dir.path().join("f.part"), vec![0u8; 3000]).unwrap();

    let sink = Arc::new(VecSink::new());
    let ctx = TaskCtx::new("test", sink.clone());
    fast(1)
        .download_all(vec![target.clone()], "files", &ctx)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), data);
    assert_eq!(ranges(&server), vec![Some("bytes=3000-".to_owned()), None]);
    let last = sink.events().pop().unwrap();
    assert_eq!(last.bytes_done, 10_000, "从头重下时不应重复计数");
}

#[tokio::test]
async fn server_without_range_support_restarts_from_scratch() {
    let data = body(10_000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    server.ignore_range("/f");
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, "/f", dir.path(), &data);
    std::fs::write(dir.path().join("f.part"), &data[..3000]).unwrap();

    fast(1)
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), data);
    assert_eq!(server.hits(), 1);
}

#[tokio::test]
async fn oversized_part_is_discarded() {
    let data = body(1000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, "/f", dir.path(), &data);
    std::fs::write(dir.path().join("f.part"), vec![1u8; 5000]).unwrap();

    fast(1)
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), data);
    assert_eq!(ranges(&server), vec![None]);
}

#[tokio::test]
async fn cancel_mid_download_keeps_only_part_then_resumes() {
    let data = body(100_000);
    let server = TestServer::start(HashMap::from([("/f", data.clone())])).await;
    server.fail("/f", [Fault::Stall(40_000)]);
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, "/f", dir.path(), &data);
    let engine = fast(1);

    let ctx = TaskCtx::noop("test");
    let cancel = ctx.cancel.clone();
    let part = dir.path().join("f.part");
    let watcher = {
        let part = part.clone();
        tokio::spawn(async move {
            // 等到已经写入一部分再取消。
            while std::fs::metadata(&part).map_or(0, |m| m.len()) < 40_000 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            cancel.cancel();
        })
    };
    let err = tokio::time::timeout(
        Duration::from_secs(10),
        engine.download_all(vec![target.clone()], "files", &ctx),
    )
    .await
    .expect("取消应及时生效")
    .unwrap_err();
    watcher.await.unwrap();
    assert!(err.is_cancelled());
    assert_eq!(file_names(dir.path()), vec!["f.part"]);

    engine
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), data);
    assert_eq!(ranges(&server), vec![None, Some("bytes=40000-".to_owned())]);
    assert_eq!(file_names(dir.path()), vec!["f"]);
}

#[tokio::test]
async fn concurrency_limit_is_respected() {
    let routes: Vec<(String, Vec<u8>)> =
        (0..24).map(|i| (format!("/f{i}"), body(100 + i))).collect();
    let server = TestServer::start(routes.clone()).await;
    server.set_delay(Duration::from_millis(30));
    let dir = tempfile::tempdir().unwrap();
    let items = routes
        .iter()
        .map(|(route, data)| item(&server, route, dir.path(), data))
        .collect();

    let report = fast(3)
        .download_all(items, "files", &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(report.downloaded, 24);
    let max = server.max_active();
    assert!((2..=3).contains(&max), "最大并发 {max} 应不超过 3");
}

#[tokio::test]
async fn concurrency_limit_is_shared_by_clones() {
    let routes: Vec<(String, Vec<u8>)> =
        (0..12).map(|i| (format!("/f{i}"), body(100 + i))).collect();
    let server = TestServer::start(routes.clone()).await;
    server.set_delay(Duration::from_millis(30));
    let dir = tempfile::tempdir().unwrap();
    let mut items: Vec<DownloadItem> = routes
        .iter()
        .map(|(route, data)| item(&server, route, dir.path(), data))
        .collect();
    let second = items.split_off(6);

    let engine = fast(2);
    let other = engine.clone();
    let ctx = TaskCtx::noop("test");
    let (a, b) = tokio::join!(
        engine.download_all(items, "a", &ctx),
        other.download_all(second, "b", &ctx)
    );
    a.unwrap();
    b.unwrap();
    assert!(server.max_active() <= 2, "两批共享同一个上限");
}

#[tokio::test]
async fn connection_refused_is_mapped() {
    // 先占用再释放一个端口，确保没有服务在监听。
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let client = HttpClient::new().unwrap();
    let err = client
        .get_bytes(&format!("http://{addr}/x"), &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetConnect);
}

#[tokio::test]
async fn cancelled_task_returns_cancelled() {
    let server = TestServer::start(HashMap::from([("/f", b"data".to_vec())])).await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = TaskCtx::noop("test");
    ctx.cancel.cancel();
    let items = vec![item(&server, "/f", dir.path(), b"data")];
    let err = fast(2)
        .download_all(items, "files", &ctx)
        .await
        .unwrap_err();
    assert!(err.is_cancelled());
    assert!(file_names(dir.path()).is_empty());
    assert_eq!(server.hits(), 0);
}

#[tokio::test]
async fn get_json_parses_body() {
    let server =
        TestServer::start(HashMap::from([("/v.json", br#"{"id":"1.20.1"}"#.to_vec())])).await;
    let client = HttpClient::new().unwrap();
    let value: serde_json::Value = client
        .get_json(&server.url("/v.json"), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(value["id"], "1.20.1");
}

#[tokio::test]
async fn range_request_returns_partial_content() {
    let server = TestServer::start(HashMap::from([("/f", body(100))])).await;
    let client = HttpClient::new().unwrap();
    let response = client
        .get_range(&server.url("/f"), 10, Some(19), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(response.status(), 206);
    assert_eq!(response.bytes().await.unwrap().to_vec(), body(100)[10..20]);
    assert_eq!(ranges(&server), vec![Some("bytes=10-19".to_owned())]);
}
