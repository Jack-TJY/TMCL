//! 统一错误类型 `AppError` 与稳定错误码 `ErrorCode`（PLAN.md 第 3.1 节）。
//!
//! 后端只返回错误码和插值参数，不返回面向用户的文字；前端以 `errors.<CODE>`
//! 作为 i18n key 渲染。所有错误码登记在 `docs/contracts/errors.md`。

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::log::redact;

/// 稳定的机器可读错误码，序列化为 `<模块>_<原因>` 形式的全大写字符串。
///
/// 删除或改名属于契约变更；新增错误码需同时登记 `docs/contracts/errors.md`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// 任务被用户或上层取消。
    CommonCancelled,
    /// 配置文件的 `schema` 高于当前程序支持的版本。
    CommonConfigSchemaUnsupported,
    /// 无法确定或创建数据目录。
    CommonDataDirUnavailable,
    /// SQLite 打开、迁移或查询失败。
    CommonDb,
    /// 未归类的内部错误（程序缺陷）。
    CommonInternal,
    /// 文件系统读写失败。
    CommonIo,
    /// JSON 序列化或反序列化失败。
    CommonJson,
    /// 日志系统初始化失败。
    CommonLogInit,
    /// 版本 JSON 格式错误或缺少必需字段。
    GameVersionJsonInvalid,
    /// 版本清单中没有该版本。
    GameVersionNotFound,
    /// 无法连接服务器（DNS、TCP 或 TLS 失败）。
    NetConnect,
    /// 下载文件的 SHA1 与期望值不一致。
    NetHashMismatch,
    /// 服务器返回了非 2xx 状态码。
    NetHttpStatus,
    /// 其他 HTTP 请求错误（请求构造、响应读取、解码等）。
    NetRequest,
    /// 下载文件的大小与期望值不一致。
    NetSizeMismatch,
    /// 连接或读取超时。
    NetTimeout,
}

impl ErrorCode {
    /// 返回序列化后的错误码字符串，例如 `COMMON_IO`。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommonCancelled => "COMMON_CANCELLED",
            Self::CommonConfigSchemaUnsupported => "COMMON_CONFIG_SCHEMA_UNSUPPORTED",
            Self::CommonDataDirUnavailable => "COMMON_DATA_DIR_UNAVAILABLE",
            Self::CommonDb => "COMMON_DB",
            Self::CommonInternal => "COMMON_INTERNAL",
            Self::CommonIo => "COMMON_IO",
            Self::CommonJson => "COMMON_JSON",
            Self::CommonLogInit => "COMMON_LOG_INIT",
            Self::GameVersionJsonInvalid => "GAME_VERSION_JSON_INVALID",
            Self::GameVersionNotFound => "GAME_VERSION_NOT_FOUND",
            Self::NetConnect => "NET_CONNECT",
            Self::NetHashMismatch => "NET_HASH_MISMATCH",
            Self::NetHttpStatus => "NET_HTTP_STATUS",
            Self::NetRequest => "NET_REQUEST",
            Self::NetSizeMismatch => "NET_SIZE_MISMATCH",
            Self::NetTimeout => "NET_TIMEOUT",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 跨层传递的统一错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, TS)]
#[error("{code}{}", .detail.as_deref().map(|d| format!(": {d}")).unwrap_or_default())]
pub struct AppError {
    /// 稳定的机器可读错误码。
    pub code: ErrorCode,
    /// 供前端 i18n 插值的参数。
    pub params: BTreeMap<String, String>,
    /// 仅用于日志与“复制详情”，构造时已脱敏。
    pub detail: Option<String>,
}

/// `Result<T, AppError>` 的简写。
pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    /// 用错误码创建一个不带参数和详情的错误。
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            params: BTreeMap::new(),
            detail: None,
        }
    }

    /// 追加一个 i18n 插值参数。
    pub fn with_param(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.params.insert(key.into(), value.to_string());
        self
    }

    /// 设置详情；内容会经过 [`redact`] 脱敏。
    pub fn with_detail(mut self, detail: impl AsRef<str>) -> Self {
        self.detail = Some(redact(detail.as_ref()).into_owned());
        self
    }

    /// 任务取消错误的快捷构造。
    pub fn cancelled() -> Self {
        Self::new(ErrorCode::CommonCancelled)
    }

    /// 是否为取消错误。
    pub fn is_cancelled(&self) -> bool {
        self.code == ErrorCode::CommonCancelled
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorCode::CommonIo).with_detail(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(ErrorCode::CommonJson).with_detail(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_serializes_as_screaming_snake_case() {
        let json = serde_json::to_string(&ErrorCode::CommonConfigSchemaUnsupported).unwrap();
        assert_eq!(json, "\"COMMON_CONFIG_SCHEMA_UNSUPPORTED\"");
    }

    #[test]
    fn as_str_matches_serde() {
        for code in [
            ErrorCode::CommonCancelled,
            ErrorCode::CommonConfigSchemaUnsupported,
            ErrorCode::CommonDataDirUnavailable,
            ErrorCode::CommonDb,
            ErrorCode::CommonInternal,
            ErrorCode::CommonIo,
            ErrorCode::CommonJson,
            ErrorCode::CommonLogInit,
            ErrorCode::GameVersionJsonInvalid,
            ErrorCode::GameVersionNotFound,
            ErrorCode::NetConnect,
            ErrorCode::NetHashMismatch,
            ErrorCode::NetHttpStatus,
            ErrorCode::NetRequest,
            ErrorCode::NetSizeMismatch,
            ErrorCode::NetTimeout,
        ] {
            let json = serde_json::to_string(&code).unwrap();
            assert_eq!(json, format!("\"{}\"", code.as_str()));
        }
    }

    #[test]
    fn error_serializes_with_params_and_detail() {
        let err = AppError::new(ErrorCode::CommonIo)
            .with_param("path", "config.json")
            .with_detail("permission denied");
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "code": "COMMON_IO",
                "params": { "path": "config.json" },
                "detail": "permission denied",
            })
        );
        assert_eq!(err.to_string(), "COMMON_IO: permission denied");
    }

    #[test]
    fn detail_is_redacted() {
        let err = AppError::new(ErrorCode::CommonInternal).with_detail("access_token=abc123");
        assert_eq!(err.detail.as_deref(), Some("access_token=***"));
    }

    #[test]
    fn display_without_detail_is_code() {
        assert_eq!(AppError::cancelled().to_string(), "COMMON_CANCELLED");
        assert!(AppError::cancelled().is_cancelled());
    }
}
