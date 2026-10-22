//! 测试辅助：本地最小 HTTP 服务器（仅在 `test-util` 特性下编译）。
//!
//! 按路径返回固定内容，未知路径返回 404，并记录请求数，
//! 供本 crate 与下游 crate（如 `tmcl-game`）的离线测试使用。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 按路径返回固定内容的服务器，记录收到的请求数。
pub struct TestServer {
    base: String,
    hits: Arc<AtomicUsize>,
}

impl TestServer {
    /// 在 127.0.0.1 的随机端口启动，`routes` 为路径 → 响应体。
    pub async fn start<K: Into<String>>(routes: impl IntoIterator<Item = (K, Vec<u8>)>) -> Self {
        let routes: HashMap<String, Vec<u8>> =
            routes.into_iter().map(|(k, v)| (k.into(), v)).collect();
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("测试服务器");
        let base = format!("http://{}", listener.local_addr().expect("测试服务器"));
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

    /// 拼出完整地址。
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// 目前收到的请求数。
    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}
