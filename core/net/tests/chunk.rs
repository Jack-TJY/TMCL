//! 分片下载的离线测试。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sha1::{Digest, Sha1};
use tmcl_common::{ErrorCode, TaskCtx, VecSink};
use tmcl_net::testing::{Fault, TestServer};
use tmcl_net::{DownloadItem, DownloadOptions, Downloader, HttpClient, Outcome, RetryPolicy};

const CHUNK: u64 = 400;

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 % 251) as u8).collect()
}

/// 超过 1000 字节即分片、每片 400 字节、重试间隔很短的下载引擎。
fn engine(concurrency: usize) -> Downloader {
    Downloader::new(
        HttpClient::new().unwrap(),
        DownloadOptions {
            concurrency,
            retry: RetryPolicy::with_delays([Duration::from_millis(10); 3]),
            chunk_threshold: 1000,
            chunk_size: CHUNK,
        },
    )
}

fn item(server: &TestServer, dir: &Path, data: &[u8]) -> DownloadItem {
    DownloadItem {
        url: server.url("/big"),
        path: dir.join("big.jar"),
        size: Some(data.len() as u64),
        sha1: Some(sha1_hex(data)),
    }
}

fn ranges(server: &TestServer) -> Vec<String> {
    let mut ranges: Vec<String> = server
        .requests()
        .into_iter()
        .map(|r| r.range.unwrap_or_default())
        .collect();
    ranges.sort();
    ranges
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
async fn large_file_is_downloaded_in_chunks() {
    let data = body(2500);
    let server = TestServer::start(HashMap::from([("/big", data.clone())])).await;
    let dir = tempfile::tempdir().unwrap();
    let sink = Arc::new(VecSink::new());
    let ctx = TaskCtx::new("test", sink.clone());

    let report = engine(4)
        .download_all(vec![item(&server, dir.path(), &data)], "client", &ctx)
        .await
        .unwrap();
    assert_eq!((report.downloaded, report.bytes), (1, 2500));
    assert_eq!(std::fs::read(dir.path().join("big.jar")).unwrap(), data);
    assert_eq!(file_names(dir.path()), vec!["big.jar"]);
    let mut expected: Vec<String> = (0..7)
        .map(|i| format!("bytes={}-{}", i * CHUNK, ((i + 1) * CHUNK).min(2500) - 1))
        .collect();
    expected.push("bytes=0-0".to_owned());
    expected.sort();
    assert_eq!(ranges(&server), expected);
    let last = sink.events().pop().unwrap();
    assert_eq!((last.bytes_done, last.bytes_total), (2500, 2500));
}

#[tokio::test]
async fn small_file_is_not_chunked() {
    let data = body(1000);
    let server = TestServer::start(HashMap::from([("/big", data.clone())])).await;
    let dir = tempfile::tempdir().unwrap();
    engine(4)
        .download_one(&item(&server, dir.path(), &data), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(ranges(&server), vec![String::new()]);
}

#[tokio::test]
async fn server_without_range_falls_back_to_single_stream() {
    let data = body(2500);
    let server = TestServer::start(HashMap::from([("/big", data.clone())])).await;
    server.ignore_range("/big");
    let dir = tempfile::tempdir().unwrap();
    engine(4)
        .download_one(&item(&server, dir.path(), &data), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(dir.path().join("big.jar")).unwrap(), data);
    // 一次探测（被忽略）+ 一次完整下载。
    assert_eq!(server.hits(), 2);
    assert_eq!(file_names(dir.path()), vec!["big.jar"]);
}

#[tokio::test]
async fn interrupted_chunk_is_resumed() {
    let data = body(2500);
    let server = TestServer::start(HashMap::from([("/big", data.clone())])).await;
    // 第 1 个请求是探测，第 2 个请求（某个分片）发送 100 字节后断开。
    server.fail("/big", [Fault::Truncate(usize::MAX), Fault::Truncate(100)]);
    let dir = tempfile::tempdir().unwrap();
    let outcome = engine(1)
        .download_one(&item(&server, dir.path(), &data), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(outcome, Outcome::Downloaded(2500));
    assert_eq!(std::fs::read(dir.path().join("big.jar")).unwrap(), data);
    let resumed = ranges(&server).into_iter().any(|r| {
        r.strip_prefix("bytes=")
            .and_then(|r| r.split('-').next())
            .and_then(|s| s.parse::<u64>().ok())
            .is_some_and(|start| start % CHUNK == 100)
    });
    assert!(
        resumed,
        "断开的分片应从第 100 字节续传：{:?}",
        ranges(&server)
    );
    assert_eq!(file_names(dir.path()), vec!["big.jar"]);
}

#[tokio::test]
async fn cancel_leaves_only_part_files_and_run_resumes() {
    let data = body(2500);
    let server = TestServer::start(HashMap::from([("/big", data.clone())])).await;
    server.fail("/big", [Fault::Truncate(usize::MAX), Fault::Stall(150)]);
    let dir = tempfile::tempdir().unwrap();
    let target = item(&server, dir.path(), &data);
    let engine = engine(2);

    let ctx = TaskCtx::noop("test");
    let cancel = ctx.cancel.clone();
    let run = engine.download_one(&target, &ctx);
    let canceller = async {
        // 等卡住的分片写入一部分后再取消。
        while server.hits() < 3 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel();
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(run, canceller)
    })
    .await
    .expect("取消应及时生效");
    assert!(result.unwrap_err().is_cancelled());
    let names = file_names(dir.path());
    assert!(!names.is_empty());
    assert!(names.iter().all(|n| n.ends_with(".part")), "{names:?}");

    engine
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), data);
    assert_eq!(file_names(dir.path()), vec!["big.jar"]);
}

#[tokio::test]
async fn chunks_respect_concurrency_limit() {
    let data = body(4000);
    let server = TestServer::start(HashMap::from([("/big", data.clone())])).await;
    server.set_delay(Duration::from_millis(30));
    let dir = tempfile::tempdir().unwrap();
    engine(2)
        .download_one(&item(&server, dir.path(), &data), &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(server.hits(), 11);
    assert_eq!(server.max_active(), 2);
}

#[tokio::test]
async fn hash_mismatch_removes_all_temporary_files() {
    let data = body(2500);
    let server = TestServer::start(HashMap::from([("/big", data.clone())])).await;
    let dir = tempfile::tempdir().unwrap();
    let target = DownloadItem {
        sha1: Some(sha1_hex(b"something else")),
        ..item(&server, dir.path(), &data)
    };
    let err = engine(4)
        .download_one(&target, &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHashMismatch);
    assert!(file_names(dir.path()).is_empty());
}

/// 联网：用默认设置（8 MiB 以上分片）下载 1.20.1 的 client.jar 并校验。
/// 运行：`cargo test -p tmcl-net --test chunk -- --ignored`
#[tokio::test]
#[ignore = "需要联网"]
async fn online_client_jar_is_downloaded_in_chunks() {
    let client = HttpClient::new().unwrap();
    let ctx = TaskCtx::noop("test");
    let manifest: serde_json::Value = client
        .get_json(
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
            &ctx,
        )
        .await
        .unwrap();
    let url = manifest["versions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "1.20.1")
        .and_then(|v| v["url"].as_str())
        .unwrap()
        .to_owned();
    let version: serde_json::Value = client.get_json(&url, &ctx).await.unwrap();
    let jar = &version["downloads"]["client"];
    let dir = tempfile::tempdir().unwrap();
    let target = DownloadItem {
        url: jar["url"].as_str().unwrap().to_owned(),
        path: dir.path().join("client.jar"),
        size: jar["size"].as_u64(),
        sha1: jar["sha1"].as_str().map(str::to_owned),
    };
    assert!(target.size.unwrap() > 8 * 1024 * 1024);
    let engine = Downloader::new(client, DownloadOptions::default());
    let outcome = engine.download_one(&target, &ctx).await.unwrap();
    assert_eq!(outcome, Outcome::Downloaded(target.size.unwrap()));
    assert_eq!(file_names(dir.path()), vec!["client.jar"]);
}
