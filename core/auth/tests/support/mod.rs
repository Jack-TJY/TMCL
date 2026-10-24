//! 测试共用的本地 HTTP 服务器：按 `"<METHOD> <path>"` 返回预设响应，并记录收到的请求。
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 一条预设响应：状态码、额外响应头、响应体。
#[derive(Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub fn reply(status: u16, body: impl Into<Vec<u8>>) -> Reply {
    Reply {
        status,
        headers: vec![],
        body: body.into(),
    }
}

#[derive(Default)]
struct State {
    /// `"<METHOD> <path>"` → 依次使用的响应，最后一个重复使用。
    routes: HashMap<String, Vec<Reply>>,
    /// 收到的 POST：路径与 JSON 请求体。
    posts: Vec<(String, Value)>,
    /// 收到的全部请求。
    requests: Vec<Request>,
}

/// 收到的一个请求；头名称已转为小写。
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub struct Site {
    pub base: String,
    state: Arc<Mutex<State>>,
}

impl Site {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let state = shared.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let head_end = loop {
                        let n = socket.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            break i + 4;
                        }
                    };
                    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                    let mut lines = head.lines();
                    let mut first = lines.next().unwrap_or_default().split(' ');
                    let method = first.next().unwrap_or_default().to_owned();
                    let path = first.next().unwrap_or_default().to_owned();
                    let headers: Vec<(String, String)> = lines
                        .filter_map(|l| l.split_once(':'))
                        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
                        .collect();
                    let length: usize = headers
                        .iter()
                        .find(|(k, _)| k == "content-length")
                        .and_then(|(_, v)| v.parse().ok())
                        .unwrap_or(0);
                    while buf.len() < head_end + length {
                        let n = socket.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    let reply = {
                        let mut s = state.lock().unwrap();
                        let body = buf[head_end..].to_vec();
                        if method == "POST" {
                            let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
                            s.posts.push((path.clone(), json));
                        }
                        s.requests.push(Request {
                            method: method.clone(),
                            path: path.clone(),
                            headers: headers.clone(),
                            body,
                        });
                        match s.routes.get_mut(&format!("{method} {path}")) {
                            Some(list) if list.len() > 1 => list.remove(0),
                            Some(list) => list[0].clone(),
                            None => reply(404, ""),
                        }
                    };
                    let mut out = format!(
                        "HTTP/1.1 {} X\r\ncontent-length: {}\r\ncontent-type: application/json\r\nconnection: close\r\n",
                        reply.status,
                        reply.body.len()
                    );
                    for (k, v) in &reply.headers {
                        out.push_str(&format!("{k}: {v}\r\n"));
                    }
                    out.push_str("\r\n");
                    let _ = socket.write_all(out.as_bytes()).await;
                    let _ = socket.write_all(&reply.body).await;
                });
            }
        });
        Self { base, state }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub fn on(&self, method: &str, path: &str, replies: Vec<Reply>) {
        self.state
            .lock()
            .unwrap()
            .routes
            .insert(format!("{method} {path}"), replies);
    }

    pub fn posts(&self) -> Vec<(String, Value)> {
        self.state.lock().unwrap().posts.clone()
    }

    pub fn requests(&self) -> Vec<Request> {
        self.state.lock().unwrap().requests.clone()
    }
}
