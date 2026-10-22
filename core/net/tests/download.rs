//! 下载模块的离线测试：用本地 TCP 端口起一个最小 HTTP 服务器。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use sha1::{Digest, Sha1};
use tmcl_common::{ErrorCode, TaskCtx, VecSink};
use tmcl_net::{DownloadItem, HttpClient, download_all, download_one, is_valid};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 按路径返回固定内容的服务器，记录收到的请求数。
struct TestServer {
    base: String,
    hits: Arc<AtomicUsize>,
}

impl TestServer {
    async fn start(routes: HashMap<&'static str, Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let routes = Arc::new(routes);
        let counter = hits.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let routes = routes.clone();
                let counter = counter.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        let n = socket.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    counter.fetch_add(1, Ordering::SeqCst);
                    let request = String::from_utf8_lossy(&buf);
                    let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    let response = match routes.get(path.as_str()) {
                        Some(body) => {
                            let mut r = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            )
                            .into_bytes();
                            r.extend_from_slice(body);
                            r
                        }
                        None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_vec(),
                    };
                    let _ = socket.write_all(&response).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        Self { base, hits }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

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

    let client = HttpClient::new().unwrap();
    download_one(&client, &target, &TaskCtx::noop("test"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target.path).unwrap(), body);
}

#[tokio::test]
async fn hash_mismatch_leaves_no_file() {
    let server = TestServer::start(HashMap::from([("/f", b"tampered".to_vec())])).await;
    let dir = tempfile::tempdir().unwrap();
    let mut target = item(&server, "/f", dir.path(), b"original");
    target.size = None;

    let client = HttpClient::new().unwrap();
    let err = download_one(&client, &target, &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHashMismatch);
    assert_eq!(err.params["expected"], sha1_hex(b"original"));
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
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
    let client = HttpClient::new().unwrap();
    let err = download_one(&client, &target, &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetSizeMismatch);
    assert!(!target.path.exists());
}

#[tokio::test]
async fn http_error_status_is_mapped() {
    let server = TestServer::start(HashMap::new()).await;
    let dir = tempfile::tempdir().unwrap();
    let client = HttpClient::new().unwrap();
    let items = vec![item(&server, "/missing", dir.path(), b"x")];
    let err = download_all(&client, items, 2, "files", &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    assert_eq!(err.params["status"], "404");
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
    let client = HttpClient::new().unwrap();
    let items = vec![item(&server, "/f", dir.path(), b"data")];
    let err = download_all(&client, items, 2, "files", &ctx)
        .await
        .unwrap_err();
    assert!(err.is_cancelled());
    assert!(!dir.path().join("f").exists());
    assert!(!dir.path().join("f.part").exists());
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
