//! 测试用的最小 HTTP 服务器：由闭包决定响应（状态码、响应头、响应体），记录每个请求。
//!
//! `tmcl_net::testing::TestServer` 只支持按路径返回固定内容，这里还需要自定义响应头
//! （`X-Ratelimit-*`）和按请求次数变化的响应。

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 服务器收到的一个请求。
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    /// 路径加查询串，例如 `/v2/search?query=a`。
    pub target: String,
    /// 小写的响应头名与值。
    pub headers: Vec<(String, String)>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }

    /// 解码后的查询参数。
    pub fn query(&self) -> Vec<(String, String)> {
        let url = reqwest::Url::parse(&format!("http://test{}", self.target)).unwrap();
        url.query_pairs().into_owned().collect()
    }
}

/// 要返回的响应。
pub struct Response {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn json(body: &[u8]) -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: body.to_vec(),
        }
    }

    pub fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn header(mut self, name: &'static str, value: impl ToString) -> Self {
        self.headers.push((name, value.to_string()));
        self
    }
}

type Handler = dyn Fn(&Request, usize) -> Response + Send + Sync;

pub struct MockServer {
    base: String,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl MockServer {
    /// 启动服务器；`handler` 的第二个参数是该请求的序号（从 0 开始）。
    pub async fn start(
        handler: impl Fn(&Request, usize) -> Response + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let handler: Arc<Handler> = Arc::new(handler);
        let recorded = requests.clone();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let handler = handler.clone();
                let recorded = recorded.clone();
                tokio::spawn(async move { serve(socket, &*handler, &recorded).await });
            }
        });
        Self { base, requests }
    }

    /// 拼出完整地址。
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

async fn serve(mut socket: TcpStream, handler: &Handler, recorded: &Mutex<Vec<Request>>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.lines();
    let mut start = lines.next().unwrap_or_default().split_whitespace();
    let method = start.next().unwrap_or_default().to_owned();
    let target = start.next().unwrap_or_default().to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect();
    let length: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[head_end..].to_vec();
    while body.len() < length {
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    let request = Request {
        method,
        target,
        headers,
    };
    let index = {
        let mut recorded = recorded.lock().unwrap();
        recorded.push(request.clone());
        recorded.len() - 1
    };
    let response = handler(&request, index);
    let mut out = format!(
        "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        response.body.len()
    );
    for (name, value) in &response.headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    let _ = socket.write_all(out.as_bytes()).await;
    let _ = socket.write_all(&response.body).await;
    let _ = socket.shutdown().await;
}
