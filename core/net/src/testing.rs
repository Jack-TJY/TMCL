//! 测试辅助：本地最小 HTTP 服务器（仅在 `test-util` 特性下编译）。
//!
//! 按路径返回固定内容，未知路径返回 404，支持 `Range` 请求，
//! 并可按路径注入故障（错误状态码、中途断开、卡住、不回响应头、极慢发送、
//! 忽略 `Range`、响应延迟）。
//! 记录每个请求与同时处理的最大连接数，供本 crate 与下游 crate
//! （如 `tmcl-game`）的离线测试使用。

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 服务器收到的一个请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// 请求路径。
    pub path: String,
    /// `Range` 头的值，例如 `bytes=100-`。
    pub range: Option<String>,
    /// 全部请求头，名称为小写。
    pub headers: Vec<(String, String)>,
}

impl Request {
    /// 按小写名称取请求头。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// 某个路径上注入的故障，每次请求消耗一次。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// 返回该状态码，不带响应体。
    Status(u16),
    /// 声明完整长度，但只发送前 n 个字节就断开。
    Truncate(usize),
    /// 发送前 n 个字节后一直不再发送（模拟卡住，供取消测试）。
    Stall(usize),
    /// 读完请求后一直不回响应头。
    Hang,
    /// 正常回响应头，然后每隔给定时间只发送 1 个字节。
    Trickle(Duration),
}

#[derive(Default)]
struct State {
    routes: HashMap<String, Vec<u8>>,
    faults: HashMap<String, Vec<Fault>>,
    no_range: Vec<String>,
    delay: Duration,
    requests: Vec<Request>,
}

/// 按路径返回固定内容的服务器，记录收到的请求。
pub struct TestServer {
    base: String,
    state: Arc<Mutex<State>>,
    max_active: Arc<AtomicUsize>,
}

impl TestServer {
    /// 在 127.0.0.1 的随机端口启动，`routes` 为路径 → 响应体。
    pub async fn start<K: Into<String>>(routes: impl IntoIterator<Item = (K, Vec<u8>)>) -> Self {
        let state = State {
            routes: routes.into_iter().map(|(k, v)| (k.into(), v)).collect(),
            ..State::default()
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("测试服务器");
        let base = format!("http://{}", listener.local_addr().expect("测试服务器"));
        let server = Self {
            base,
            state: Arc::new(Mutex::new(state)),
            max_active: Arc::new(AtomicUsize::new(0)),
        };
        let state = server.state.clone();
        let max_active = server.max_active.clone();
        let active = Arc::new(AtomicUsize::new(0));
        tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    return;
                };
                let state = state.clone();
                let active = active.clone();
                let max_active = max_active.clone();
                tokio::spawn(async move {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(now, Ordering::SeqCst);
                    serve(socket, &state).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        server
    }

    /// 添加或替换一个路由；用于内容里需要包含服务器地址的场景。
    pub fn route(&self, path: impl Into<String>, body: Vec<u8>) {
        lock(&self.state).routes.insert(path.into(), body);
    }

    /// 为 `path` 依次注入故障：第 n 个请求使用第 n 个故障，用完后恢复正常。
    pub fn fail(&self, path: impl Into<String>, faults: impl IntoIterator<Item = Fault>) {
        let mut faults: Vec<Fault> = faults.into_iter().collect();
        faults.reverse();
        lock(&self.state).faults.insert(path.into(), faults);
    }

    /// 让 `path` 忽略 `Range` 头，总是返回完整内容。
    pub fn ignore_range(&self, path: impl Into<String>) {
        lock(&self.state).no_range.push(path.into());
    }

    /// 每个响应在发送前等待 `delay`，用于观察并发数。
    pub fn set_delay(&self, delay: Duration) {
        lock(&self.state).delay = delay;
    }

    /// 拼出完整地址。
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// 目前收到的请求数。
    pub fn hits(&self) -> usize {
        lock(&self.state).requests.len()
    }

    /// 目前收到的所有请求，按到达顺序。
    pub fn requests(&self) -> Vec<Request> {
        lock(&self.state).requests.clone()
    }

    /// 同时处理的最大连接数。
    pub fn max_active(&self) -> usize {
        self.max_active.load(Ordering::SeqCst)
    }
}

async fn serve(mut socket: TcpStream, state: &Mutex<State>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = socket.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let request = String::from_utf8_lossy(&buf);
    let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let headers: Vec<(String, String)> = request
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect();
    let range = headers
        .iter()
        .find(|(name, _)| name == "range")
        .map(|(_, value)| value.clone());

    let (body, fault, honor_range, delay) = {
        let mut state = lock(state);
        state.requests.push(Request {
            path: path.clone(),
            range: range.clone(),
            headers,
        });
        let fault = state.faults.get_mut(&path).and_then(Vec::pop);
        let honor_range = !state.no_range.contains(&path);
        (
            state.routes.get(&path).cloned(),
            fault,
            honor_range,
            state.delay,
        )
    };
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }

    if fault == Some(Fault::Hang) {
        tokio::time::sleep(Duration::from_secs(60)).await;
        return;
    }

    let (head, payload) = match (body, fault) {
        (_, Some(Fault::Status(status))) => (status_head(status), Vec::new()),
        (None, _) => (status_head(404), Vec::new()),
        (Some(body), _) => match range.as_deref().filter(|_| honor_range) {
            Some(range) => match parse_range(range, body.len()) {
                Some((start, end)) => (
                    format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nConnection: close\r\n\r\n",
                        end + 1 - start,
                        body.len()
                    ),
                    body[start..=end].to_vec(),
                ),
                None => (
                    format!(
                        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        body.len()
                    ),
                    Vec::new(),
                ),
            },
            None => (
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                    body.len()
                ),
                body,
            ),
        },
    };

    let _ = socket.write_all(head.as_bytes()).await;
    match fault {
        Some(Fault::Truncate(n)) => {
            let _ = socket.write_all(&payload[..n.min(payload.len())]).await;
        }
        Some(Fault::Stall(n)) => {
            let _ = socket.write_all(&payload[..n.min(payload.len())]).await;
            let _ = socket.flush().await;
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        Some(Fault::Trickle(interval)) => {
            for byte in payload.chunks(1) {
                if socket.write_all(byte).await.is_err() || socket.flush().await.is_err() {
                    return;
                }
                tokio::time::sleep(interval).await;
            }
        }
        _ => {
            let _ = socket.write_all(&payload).await;
        }
    }
    let _ = socket.shutdown().await;
}

fn status_head(status: u16) -> String {
    format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

/// 解析 `bytes=a-b` / `bytes=a-`，返回闭区间；越界时返回 `None`。
fn parse_range(range: &str, len: usize) -> Option<(usize, usize)> {
    let (start, end) = range.strip_prefix("bytes=")?.split_once('-')?;
    let start: usize = start.parse().ok()?;
    let last = len.checked_sub(1)?;
    let end = match end {
        "" => last,
        end => end.parse::<usize>().ok()?.min(last),
    };
    (start <= end).then_some((start, end))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
