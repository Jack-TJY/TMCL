//! 共享的 HTTP 客户端：统一 `User-Agent`、超时、取消与错误码映射。

use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::header::{HeaderName, HeaderValue};

use serde::de::DeserializeOwned;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};

/// 所有请求使用的 `User-Agent`：`TMCL/<版本>`。
pub const USER_AGENT: &str = concat!("TMCL/", env!("CARGO_PKG_VERSION"));

/// 各阶段的超时。超时一律返回 `NET_TIMEOUT`，由下载引擎的重试与换源处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// 建立连接（DNS、TCP、代理隧道）。
    pub connect: Duration,
    /// 从发出请求到收到响应头，包含 TLS 握手与重定向。
    pub response: Duration,
    /// 读取响应体时，两次收到数据之间的最长间隔。
    pub read_idle: Duration,
    /// 低速判定窗口：每过一个窗口检查一次平均速度。
    pub speed_window: Duration,
    /// 窗口内的最低平均速度（字节/秒），低于它视为超时；0 表示不检查。
    pub min_speed: u64,
    /// `get_bytes` / `get_json` 整个请求（含响应体）的总时限，这两个方法只用于元数据等小文件。
    pub total: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(15),
            response: Duration::from_secs(30),
            read_idle: Duration::from_secs(30),
            speed_window: Duration::from_secs(30),
            min_speed: 1024,
            total: Duration::from_secs(120),
        }
    }
}

/// 启动器共用的 HTTP 客户端，克隆开销很小（内部是引用计数）。
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
    timeouts: Timeouts,
    host_headers: Arc<Vec<HostHeader>>,
}

/// 只发给某个域名（含其子域名）的请求头，见 [`HttpClient::with_host_header`]。
#[derive(Debug)]
struct HostHeader {
    host: String,
    name: HeaderName,
    value: HeaderValue,
}

impl HostHeader {
    fn matches(&self, url: &str) -> bool {
        let Some(host) = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
        else {
            return false;
        };
        host == self.host
            || host
                .strip_suffix(&self.host)
                .is_some_and(|rest| rest.ends_with('.'))
    }
}

impl HttpClient {
    /// 创建带默认 `User-Agent` 与超时设置的客户端。
    pub fn new() -> AppResult<Self> {
        Self::with_timeouts(Timeouts::default())
    }

    /// 使用指定超时创建客户端。
    pub fn with_timeouts(timeouts: Timeouts) -> AppResult<Self> {
        let inner = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(timeouts.connect)
            .read_timeout(timeouts.read_idle)
            .build()
            .map_err(|e| AppError::new(ErrorCode::NetRequest).with_detail(e.to_string()))?;
        Ok(Self {
            inner,
            timeouts,
            host_headers: Arc::new(Vec::new()),
        })
    }

    /// 返回一个对 `host` 及其子域名的请求都附加 `name: value` 请求头的克隆，其他请求不受影响。
    ///
    /// 用于需要凭据的 CDN（如 CurseForge 的 `x-api-key`）。值标记为敏感，不会出现在
    /// `Debug` 输出中；名称或值不合法时返回 `NET_REQUEST`（错误中不含值）。
    pub fn with_host_header(&self, host: &str, name: &str, value: &str) -> AppResult<Self> {
        let invalid = |what: &str| {
            AppError::new(ErrorCode::NetRequest)
                .with_param("host", host)
                .with_detail(format!("invalid header {what}"))
        };
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| invalid("name"))?;
        let mut value = HeaderValue::from_str(value).map_err(|_| invalid("value"))?;
        value.set_sensitive(true);
        let mut headers: Vec<HostHeader> = self
            .host_headers
            .iter()
            .map(|h| HostHeader {
                host: h.host.clone(),
                name: h.name.clone(),
                value: h.value.clone(),
            })
            .collect();
        headers.push(HostHeader {
            host: host.trim_matches('.').to_ascii_lowercase(),
            name,
            value,
        });
        Ok(Self {
            host_headers: Arc::new(headers),
            ..self.clone()
        })
    }

    /// 当前超时设置。
    pub fn timeouts(&self) -> Timeouts {
        self.timeouts
    }

    /// 包装响应体，按本客户端的空闲与低速超时逐块读取。
    pub fn body(&self, response: reqwest::Response, url: &str) -> Body {
        Body {
            response,
            url: url.to_owned(),
            timeouts: self.timeouts,
            window_start: Instant::now(),
            window_bytes: 0,
        }
    }

    /// 发送 GET 请求，返回状态码为 2xx 的响应；可被 `ctx` 取消。
    pub async fn get(&self, url: &str, ctx: &TaskCtx) -> AppResult<reqwest::Response> {
        self.send(self.inner.get(url), url, ctx).await
    }

    /// 发送带 `Range: bytes=<start>-[<end>]` 的 GET 请求，`end` 为闭区间终点。
    ///
    /// 服务器可能忽略 `Range` 而返回 200 和完整内容，调用方需检查状态码。
    pub async fn get_range(
        &self,
        url: &str,
        start: u64,
        end: Option<u64>,
        ctx: &TaskCtx,
    ) -> AppResult<reqwest::Response> {
        let range = match end {
            Some(end) => format!("bytes={start}-{end}"),
            None => format!("bytes={start}-"),
        };
        let request = self.inner.get(url).header(reqwest::header::RANGE, range);
        self.send(request, url, ctx).await
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        url: &str,
        ctx: &TaskCtx,
    ) -> AppResult<reqwest::Response> {
        let mut request = request;
        for header in self.host_headers.iter().filter(|h| h.matches(url)) {
            request = request.header(header.name.clone(), header.value.clone());
        }
        // 连接、TLS 握手与等待响应头都算在内，避免任何一步无限挂起。
        let send = tokio::time::timeout(self.timeouts.response, request.send());
        let sent = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
            r = send => r,
        };
        let response = match sent {
            Ok(r) => r.map_err(|e| map_reqwest_error(url, &e))?,
            Err(_) => return Err(timeout_error(url, "等待响应头", self.timeouts.response)),
        };
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::new(ErrorCode::NetHttpStatus)
                .with_param("url", url)
                .with_param("status", status.as_u16()));
        }
        Ok(response)
    }

    /// 下载整个响应体到内存，适合元数据等小文件。
    ///
    /// 整个请求受 [`Timeouts::total`] 限制。
    pub async fn get_bytes(&self, url: &str, ctx: &TaskCtx) -> AppResult<Vec<u8>> {
        let fetch = async {
            let response = self.get(url, ctx).await?;
            self.body(response, url).bytes(ctx).await
        };
        tokio::time::timeout(self.timeouts.total, fetch)
            .await
            .map_err(|_| timeout_error(url, "整个请求", self.timeouts.total))?
    }

    /// 下载并解析 JSON。
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str, ctx: &TaskCtx) -> AppResult<T> {
        let bytes = self.get_bytes(url, ctx).await?;
        serde_json::from_slice(&bytes).map_err(|e| {
            AppError::new(ErrorCode::CommonJson)
                .with_param("url", url)
                .with_detail(e.to_string())
        })
    }
}

/// 带空闲超时与低速检测的响应体读取器，见 [`HttpClient::body`]。
pub struct Body {
    response: reqwest::Response,
    url: String,
    timeouts: Timeouts,
    window_start: Instant,
    window_bytes: u64,
}

impl Body {
    /// 响应本身（状态码、响应头）。
    pub fn response(&self) -> &reqwest::Response {
        &self.response
    }

    /// 读取下一块数据，读完返回 `None`；可被 `ctx` 取消。
    ///
    /// 超过 [`Timeouts::read_idle`] 没有收到数据，或一个窗口内的平均速度低于
    /// [`Timeouts::min_speed`] 时返回 `NET_TIMEOUT`。
    pub async fn next(
        &mut self,
        ctx: &TaskCtx,
    ) -> AppResult<Option<impl Deref<Target = [u8]> + Send + use<>>> {
        let read = tokio::time::timeout(self.timeouts.read_idle, self.response.chunk());
        let read = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
            r = read => r,
        };
        let chunk = match read {
            Ok(r) => r.map_err(|e| map_reqwest_error(&self.url, &e))?,
            Err(_) => {
                return Err(timeout_error(
                    &self.url,
                    "等待数据",
                    self.timeouts.read_idle,
                ));
            }
        };
        if let Some(chunk) = &chunk {
            self.check_speed(chunk.len() as u64)?;
        }
        Ok(chunk)
    }

    /// 读完整个响应体。
    pub async fn bytes(mut self, ctx: &TaskCtx) -> AppResult<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(chunk) = self.next(ctx).await? {
            out.extend_from_slice(&chunk);
        }
        Ok(out)
    }

    /// 累计窗口内的字节数；窗口结束时平均速度过低则失败，否则开始新窗口。
    fn check_speed(&mut self, n: u64) -> AppResult<()> {
        self.window_bytes += n;
        let elapsed = self.window_start.elapsed();
        if self.timeouts.min_speed == 0 || elapsed < self.timeouts.speed_window {
            return Ok(());
        }
        let floor = self.timeouts.min_speed as f64 * elapsed.as_secs_f64();
        if (self.window_bytes as f64) < floor {
            return Err(AppError::new(ErrorCode::NetTimeout)
                .with_param("url", &self.url)
                .with_detail(format!(
                    "{:.1} 秒内只收到 {} 字节，低于 {} 字节/秒",
                    elapsed.as_secs_f64(),
                    self.window_bytes,
                    self.timeouts.min_speed
                )));
        }
        self.window_start = Instant::now();
        self.window_bytes = 0;
        Ok(())
    }
}

fn timeout_error(url: &str, stage: &str, limit: Duration) -> AppError {
    AppError::new(ErrorCode::NetTimeout)
        .with_param("url", url)
        .with_detail(format!("{stage}超过 {} 秒", limit.as_secs_f64()))
}

/// 把 `reqwest` 错误映射为稳定错误码。
pub(crate) fn map_reqwest_error(url: &str, err: &reqwest::Error) -> AppError {
    let code = if err.is_timeout() {
        ErrorCode::NetTimeout
    } else if err.is_connect() {
        ErrorCode::NetConnect
    } else if let Some(status) = err.status() {
        return AppError::new(ErrorCode::NetHttpStatus)
            .with_param("url", url)
            .with_param("status", status.as_u16());
    } else {
        ErrorCode::NetRequest
    };
    AppError::new(code)
        .with_param("url", url)
        .with_detail(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(host: &str) -> HostHeader {
        HostHeader {
            host: host.to_owned(),
            name: HeaderName::from_static("x-api-key"),
            value: HeaderValue::from_static("test-api-key"),
        }
    }

    #[test]
    fn host_header_matches_host_and_subdomains_only() {
        let h = header("forgecdn.net");
        assert!(h.matches("https://forgecdn.net/a"));
        assert!(h.matches("https://edge.forgecdn.net/files/1/2/a.jar"));
        assert!(h.matches("https://MEDIAFILEZ.forgecdn.net/a"));
        assert!(!h.matches("https://notforgecdn.net/a"));
        assert!(!h.matches("https://forgecdn.net.evil.example/a"));
        assert!(!h.matches("https://cdn.modrinth.com/forgecdn.net"));
        assert!(!h.matches("not a url"));
    }

    #[test]
    fn host_header_value_is_hidden_from_debug() {
        let client = HttpClient::new()
            .unwrap()
            .with_host_header("forgecdn.net", "x-api-key", "test-api-key")
            .unwrap();
        assert!(!format!("{client:?}").contains("test-api-key"));
        let err = HttpClient::new()
            .unwrap()
            .with_host_header("forgecdn.net", "x-api-key", "bad\nvalue")
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NetRequest);
        assert!(!format!("{err:?}").contains("bad"));
    }
}
