//! 日志初始化与脱敏层（PLAN.md 第 3.6 节）。
//!
//! 所有日志都经过 [`RedactingWriter`]：`access_token`、`refresh_token`、
//! `Authorization`、`x-api-key` 以及 `--accessToken` 后面的值一律替换为 `***`。
//! 文件日志写入 `<data_root>/logs/`，按天滚动，保留 7 天。

use std::borrow::Cow;
use std::io::{self, Write};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::error::{AppError, AppResult, ErrorCode};

/// 替换敏感值后使用的占位符。
pub const REDACTED: &str = "***";

/// 日志文件名前缀，实际文件名形如 `tmcl.2026-10-22.log`。
pub const LOG_FILE_PREFIX: &str = "tmcl";

/// 日志文件保留的天数。
pub const LOG_RETENTION_DAYS: usize = 7;

/// 每条规则：第 1 个捕获组是保留的“键”部分，其后匹配到的值被替换。
static RULES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // access_token / refresh_token / accessToken / refreshToken，
        // 覆盖 JSON（"key": "v"）、查询串（key=v）和 key: v 形式。
        r#"(?i)((?:access|refresh)_?token["']?\s*[:=]\s*["']?)[^"'&\s,;}]+"#,
        // Authorization 头，整个值（含 Bearer / Basic 前缀）都替换。
        r#"(?i)(authorization["']?\s*[:=]\s*["']?)[^"'\r\n,;}]+"#,
        // x-api-key 头。
        r#"(?i)(x-api-key["']?\s*[:=]\s*["']?)[^"'\s,;}]+"#,
        // 游戏启动参数：--accessToken xxx 或 --accessToken=xxx。
        r#"(--accessToken(?:\s+|=)["']?)[^"'\s]+"#,
    ]
    .iter()
    .map(|pattern| Regex::new(pattern).expect("脱敏规则是常量，必然合法"))
    .collect()
});

/// 对一段文本做脱敏，没有命中任何规则时不分配内存。
pub fn redact(input: &str) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(input);
    for rule in RULES.iter() {
        if rule.is_match(&out) {
            let replaced = rule
                .replace_all(&out, format!("${{1}}{REDACTED}"))
                .into_owned();
            out = Cow::Owned(replaced);
        }
    }
    out
}

/// 写入前对内容做脱敏的 [`Write`] 包装。
///
/// `tracing_subscriber::fmt` 每条事件只调用一次 `write_all`，因此按次脱敏
/// 不会把一个值切成两半。
pub struct RedactingWriter<W: Write> {
    inner: W,
}

impl<W: Write> RedactingWriter<W> {
    /// 包装一个底层写入器。
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        self.inner.write_all(redact(&text).as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// 为任意 [`MakeWriter`] 加上脱敏层。
pub struct RedactingMakeWriter<M> {
    inner: M,
}

impl<M> RedactingMakeWriter<M> {
    /// 包装一个底层 `MakeWriter`。
    pub fn new(inner: M) -> Self {
        Self { inner }
    }
}

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for RedactingMakeWriter<M> {
    type Writer = RedactingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter::new(self.inner.make_writer())
    }
}

/// 日志初始化选项。
#[derive(Debug, Clone)]
pub struct LogOptions {
    /// 最低输出级别。
    pub level: LevelFilter,
    /// 是否同时输出到 stderr（开发时使用）。
    pub stderr: bool,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self {
            level: LevelFilter::INFO,
            stderr: cfg!(debug_assertions),
        }
    }
}

/// 日志后台线程的守卫，必须持有到程序退出，否则尾部日志可能丢失。
#[must_use = "丢弃守卫会立即停止写日志文件"]
pub struct LogGuard {
    _file: WorkerGuard,
}

/// 初始化全局日志：按天滚动写入 `logs_dir`，保留 7 天，全部经过脱敏。
///
/// 每个进程只能调用一次；重复调用返回 `COMMON_LOG_INIT`。
pub fn init_logging(logs_dir: &Path, options: &LogOptions) -> AppResult<LogGuard> {
    let log_init_err = |detail: String| AppError::new(ErrorCode::CommonLogInit).with_detail(detail);

    std::fs::create_dir_all(logs_dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix("log")
        .max_log_files(LOG_RETENTION_DAYS)
        .build(logs_dir)
        .map_err(|e| log_init_err(e.to_string()))?;
    let (file_writer, guard) = tracing_appender::non_blocking(appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(RedactingMakeWriter::new(file_writer));
    let stderr_layer = options.stderr.then(|| {
        tracing_subscriber::fmt::layer().with_writer(RedactingMakeWriter::new(io::stderr))
    });

    tracing_subscriber::registry()
        .with(options.level)
        .with(file_layer)
        .with(stderr_layer)
        .try_init()
        .map_err(|e| log_init_err(e.to_string()))?;

    Ok(LogGuard { _file: guard })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn redacts_query_and_json_tokens() {
        assert_eq!(
            redact("GET /cb?code=1&access_token=abc.def&state=x"),
            "GET /cb?code=1&access_token=***&state=x"
        );
        assert_eq!(
            redact(r#"{"access_token":"eyJ0","refresh_token": "M.R3_BAY","expires_in":3600}"#),
            r#"{"access_token":"***","refresh_token": "***","expires_in":3600}"#
        );
        assert_eq!(
            redact(r#"{"accessToken": "eyJ0"}"#),
            r#"{"accessToken": "***"}"#
        );
    }

    #[test]
    fn redacts_headers() {
        assert_eq!(
            redact("Authorization: Bearer eyJhbGciOi.payload.sig\nAccept: */*"),
            "Authorization: ***\nAccept: */*"
        );
        assert_eq!(redact("x-api-key: test-api-key"), "x-api-key: ***");
        assert_eq!(
            redact(r#"{"X-Api-Key":"test-api-key"}"#),
            r#"{"X-Api-Key":"***"}"#
        );
    }

    #[test]
    fn redacts_launch_argument() {
        assert_eq!(
            redact("java -jar x.jar --username Steve --accessToken eyJ0.abc --version 1.20.1"),
            "java -jar x.jar --username Steve --accessToken *** --version 1.20.1"
        );
        assert_eq!(redact("--accessToken=eyJ0.abc"), "--accessToken=***");
    }

    #[test]
    fn leaves_clean_text_borrowed() {
        let text = "downloading 1.20.1 client.jar (12 MiB)";
        assert!(matches!(redact(text), Cow::Borrowed(_)));
    }

    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn tracing_output_goes_through_redaction() {
        let buffer = Buffer::default();
        let sink = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(RedactingMakeWriter::new(move || sink.clone()))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(args = "--accessToken secret123", "launching");
            tracing::warn!("header Authorization: Bearer secret456");
        });
        let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        assert!(!output.contains("secret123"), "{output}");
        assert!(!output.contains("secret456"), "{output}");
        assert!(output.contains("--accessToken ***"), "{output}");
    }
}
