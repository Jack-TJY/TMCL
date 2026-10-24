//! 登录接口共用的 HTTP 发送：可取消、有总时限，错误映射为 `NET_*`。
//!
//! 请求体与响应体可能含令牌或密码，一律不写入日志或错误详情。

use std::time::Duration;

use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::USER_AGENT;

/// 单个请求的总时限。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// 与 `HttpClient` 使用相同 `User-Agent` 的 reqwest 客户端。
pub(crate) fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::new(ErrorCode::NetRequest).with_detail(e.to_string()))
}

/// 发送请求，返回状态码与响应体。`url` 只用于错误参数，不应带查询串中的令牌。
pub(crate) async fn send(
    request: reqwest::RequestBuilder,
    url: &str,
    ctx: &TaskCtx,
) -> AppResult<(u16, Vec<u8>)> {
    let request = async {
        let response = request.send().await?;
        let status = response.status().as_u16();
        Ok::<_, reqwest::Error>((status, response.bytes().await?.to_vec()))
    };
    let result = tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
        r = tokio::time::timeout(REQUEST_TIMEOUT, request) => r,
    };
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => {
            let code = if err.is_timeout() {
                ErrorCode::NetTimeout
            } else if err.is_connect() {
                ErrorCode::NetConnect
            } else {
                ErrorCode::NetRequest
            };
            // reqwest 的错误信息不含请求体，可以作为详情。
            Err(AppError::new(code)
                .with_param("url", url)
                .with_detail(err.without_url().to_string()))
        }
        Err(_) => Err(AppError::new(ErrorCode::NetTimeout).with_param("url", url)),
    }
}

/// 非预期的 HTTP 状态码。
pub(crate) fn http_status(url: &str, status: u16) -> AppError {
    AppError::new(ErrorCode::NetHttpStatus)
        .with_param("url", url)
        .with_param("status", status)
}

/// 解析 JSON 响应体；失败时详情只含解析错误，不含响应内容。
pub(crate) fn json<T: serde::de::DeserializeOwned>(url: &str, bytes: &[u8]) -> AppResult<T> {
    serde_json::from_slice(bytes).map_err(|e| {
        AppError::new(ErrorCode::CommonJson)
            .with_param("url", url)
            .with_detail(e.to_string())
    })
}
