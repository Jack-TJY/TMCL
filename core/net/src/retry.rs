//! 重试策略：对可恢复的网络错误按指数退避重试（默认 1 s、2 s、4 s，共 3 次）。

use std::future::Future;
use std::time::Duration;

use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};

/// 重试策略。`delays[i]` 是第 i 次重试前的等待时间，长度即最大重试次数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// 每次重试前的等待时间。
    pub delays: Vec<Duration>,
}

impl Default for RetryPolicy {
    /// 1 s、2 s、4 s 指数退避，最多重试 3 次。
    fn default() -> Self {
        Self::with_delays([1, 2, 4].map(Duration::from_secs))
    }
}

impl RetryPolicy {
    /// 不重试。
    pub fn none() -> Self {
        Self { delays: Vec::new() }
    }

    /// 使用给定的等待时间序列。
    pub fn with_delays(delays: impl IntoIterator<Item = Duration>) -> Self {
        Self {
            delays: delays.into_iter().collect(),
        }
    }

    /// 执行 `op`，遇到 [`is_retryable`] 的错误时等待后重试；等待期间可被取消。
    ///
    /// `op` 的参数是当前尝试序号（从 0 开始）。返回最后一次的错误。
    pub async fn run<T, F, Fut>(&self, ctx: &TaskCtx, mut op: F) -> AppResult<T>
    where
        F: FnMut(usize) -> Fut,
        Fut: Future<Output = AppResult<T>>,
    {
        let mut attempt = 0;
        loop {
            ctx.check_cancelled()?;
            let err = match op(attempt).await {
                Ok(value) => return Ok(value),
                Err(err) => err,
            };
            let Some(delay) = self.delays.get(attempt).filter(|_| is_retryable(&err)) else {
                return Err(err);
            };
            tracing::debug!(code = %err.code, attempt, ?delay, "下载失败，稍后重试");
            tokio::select! {
                biased;
                _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
                _ = tokio::time::sleep(*delay) => {}
            }
            attempt += 1;
        }
    }
}

/// 错误是否值得重试：超时、连接失败、传输中断、大小或 SHA1 不符，
/// 以及 408、429 和 5xx 状态码。其余 4xx、本地 IO 错误和取消不重试。
pub fn is_retryable(err: &AppError) -> bool {
    match err.code {
        ErrorCode::NetTimeout
        | ErrorCode::NetConnect
        | ErrorCode::NetRequest
        | ErrorCode::NetSizeMismatch
        | ErrorCode::NetHashMismatch => true,
        ErrorCode::NetHttpStatus => err
            .params
            .get("status")
            .and_then(|s| s.parse::<u16>().ok())
            .is_some_and(|s| matches!(s, 408 | 429 | 500..=599)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn status(code: u16) -> AppError {
        AppError::new(ErrorCode::NetHttpStatus).with_param("status", code)
    }

    #[test]
    fn classifies_errors() {
        assert!(is_retryable(&AppError::new(ErrorCode::NetTimeout)));
        assert!(is_retryable(&AppError::new(ErrorCode::NetHashMismatch)));
        assert!(is_retryable(&status(503)));
        assert!(is_retryable(&status(429)));
        assert!(!is_retryable(&status(404)));
        assert!(!is_retryable(&status(403)));
        assert!(!is_retryable(&AppError::new(ErrorCode::CommonIo)));
        assert!(!is_retryable(&AppError::cancelled()));
    }

    #[test]
    fn default_is_exponential_backoff() {
        assert_eq!(
            RetryPolicy::default().delays,
            [1, 2, 4].map(Duration::from_secs)
        );
    }

    #[tokio::test]
    async fn retries_until_success() {
        let calls = AtomicUsize::new(0);
        let policy = RetryPolicy::with_delays([Duration::ZERO; 3]);
        let value = policy
            .run(&TaskCtx::noop("test"), |attempt| {
                calls.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt < 2 {
                        Err(AppError::new(ErrorCode::NetTimeout))
                    } else {
                        Ok(attempt)
                    }
                }
            })
            .await
            .unwrap();
        assert_eq!(value, 2);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn gives_up_after_last_delay() {
        let calls = AtomicUsize::new(0);
        let policy = RetryPolicy::with_delays([Duration::ZERO; 3]);
        let err = policy
            .run(&TaskCtx::noop("test"), |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(status(500)) }
            })
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NetHttpStatus);
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn does_not_retry_permanent_errors() {
        let calls = AtomicUsize::new(0);
        let err = RetryPolicy::default()
            .run(&TaskCtx::noop("test"), |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(status(404)) }
            })
            .await
            .unwrap_err();
        assert_eq!(err.params["status"], "404");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancel_interrupts_backoff() {
        let ctx = TaskCtx::noop("test");
        let cancel = ctx.cancel.clone();
        let policy = RetryPolicy::with_delays([Duration::from_secs(3600)]);
        let run = policy.run(&ctx, |_| {
            let cancel = cancel.clone();
            async move {
                cancel.cancel();
                Err::<(), _>(AppError::new(ErrorCode::NetConnect))
            }
        });
        let err = tokio::time::timeout(Duration::from_secs(5), run)
            .await
            .expect("取消应立即打断退避等待")
            .unwrap_err();
        assert!(err.is_cancelled());
    }
}
