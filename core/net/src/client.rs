//! 共享的 HTTP 客户端：统一 `User-Agent`、超时、取消与错误码映射。

use std::time::Duration;

use serde::de::DeserializeOwned;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};

/// 所有请求使用的 `User-Agent`：`TMCL/<版本>`。
pub const USER_AGENT: &str = concat!("TMCL/", env!("CARGO_PKG_VERSION"));

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// 启动器共用的 HTTP 客户端，克隆开销很小（内部是引用计数）。
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
}

impl HttpClient {
    /// 创建带默认 `User-Agent` 与超时设置的客户端。
    pub fn new() -> AppResult<Self> {
        let inner = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(|e| AppError::new(ErrorCode::NetRequest).with_detail(e.to_string()))?;
        Ok(Self { inner })
    }

    /// 发送 GET 请求，返回状态码为 2xx 的响应；可被 `ctx` 取消。
    pub async fn get(&self, url: &str, ctx: &TaskCtx) -> AppResult<reqwest::Response> {
        let send = self.inner.get(url).send();
        let response = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
            r = send => r.map_err(|e| map_reqwest_error(url, &e))?,
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
    pub async fn get_bytes(&self, url: &str, ctx: &TaskCtx) -> AppResult<Vec<u8>> {
        let response = self.get(url, ctx).await?;
        let body = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
            r = response.bytes() => r.map_err(|e| map_reqwest_error(url, &e))?,
        };
        Ok(body.to_vec())
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
