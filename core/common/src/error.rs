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
    /// 背景图片已被删除或无法读取。
    AppearanceImageMissing,
    /// 背景图片不是 png/jpg/webp，或超过大小上限。
    AppearanceImageUnsupported,
    /// 找不到该账号（已被删除或 ID 无效）。
    AuthAccountNotFound,
    /// 地址不是有效的 Yggdrasil 皮肤站（无法解析或元数据缺少 `meta`、`skinDomains`）。
    AuthAuthlibServerInvalid,
    /// 没有可用于启动的账号（账号列表为空或未选默认账号）。
    AuthNoAccount,
    /// 离线账号用户名不符合 `^[A-Za-z0-9_]{3,16}$`。
    AuthOfflineInvalidName,
    /// 无法读写系统密钥库（Windows 凭据管理器、macOS 钥匙串、Linux Secret Service）。
    AuthSecretStore,
    /// 皮肤站拒绝登录（账号或密码错误，或登录过于频繁）。
    AuthYggdrasilInvalidCredentials,
    /// 皮肤站账号下没有角色。
    AuthYggdrasilNoProfile,
    /// 外置登录的令牌已失效且无法刷新，需要重新输入密码。
    AuthYggdrasilSessionExpired,
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
    /// 运行该版本需要的文件缺失，且没有下载地址（由加载器安装器生成）。
    GameFileMissing,
    /// 版本 JSON 格式错误或缺少必需字段。
    GameVersionJsonInvalid,
    /// 版本清单中没有该版本。
    GameVersionNotFound,
    /// 实例的 `instance.json` 缺失或格式错误。
    InstanceCorrupted,
    /// 实例名称为空或过长。
    InstanceInvalidName,
    /// 找不到该实例。
    InstanceNotFound,
    /// `.tmclpack` 文件格式错误或缺少 `instance.json`。
    InstancePackInvalid,
    /// `instance.json` 的 `schema` 为 0 或高于当前程序支持的版本。
    InstanceSchemaUnsupported,
    /// Java 的架构与启动器不一致（例如 64 位系统上的 32 位 Java）。
    JavaArchMismatch,
    /// 找不到满足主版本要求的 Java。
    JavaNotFound,
    /// 无法识别该 Java 的版本或架构。
    JavaProbeFailed,
    /// 官方源与 Adoptium 都没有该平台、该主版本的运行时。
    JavaRuntimeUnavailable,
    /// 手动指定的 Java 主版本与游戏要求不一致（仅作警告）。
    JavaVersionMismatch,
    /// 该实例已在启动或运行中。
    LaunchAlreadyRunning,
    /// 无法启动游戏进程（Java 路径无效或无权限）。
    LaunchSpawnFailed,
    /// 实例需要的游戏版本（含加载器版本）尚未安装。
    LaunchVersionNotInstalled,
    /// 加载器安装器或安装描述文件损坏、格式不受支持。
    LoaderInstallerInvalid,
    /// Forge/NeoForge 安装器的处理步骤（processor）运行失败或输出校验不符。
    LoaderProcessorFailed,
    /// 该加载器不支持这个游戏版本。
    LoaderUnsupportedVersion,
    /// 找不到该游戏版本对应的加载器版本。
    LoaderVersionNotFound,
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
            Self::AppearanceImageMissing => "APPEARANCE_IMAGE_MISSING",
            Self::AppearanceImageUnsupported => "APPEARANCE_IMAGE_UNSUPPORTED",
            Self::AuthAccountNotFound => "AUTH_ACCOUNT_NOT_FOUND",
            Self::AuthAuthlibServerInvalid => "AUTH_AUTHLIB_SERVER_INVALID",
            Self::AuthNoAccount => "AUTH_NO_ACCOUNT",
            Self::AuthOfflineInvalidName => "AUTH_OFFLINE_INVALID_NAME",
            Self::AuthSecretStore => "AUTH_SECRET_STORE",
            Self::AuthYggdrasilInvalidCredentials => "AUTH_YGGDRASIL_INVALID_CREDENTIALS",
            Self::AuthYggdrasilNoProfile => "AUTH_YGGDRASIL_NO_PROFILE",
            Self::AuthYggdrasilSessionExpired => "AUTH_YGGDRASIL_SESSION_EXPIRED",
            Self::CommonCancelled => "COMMON_CANCELLED",
            Self::CommonConfigSchemaUnsupported => "COMMON_CONFIG_SCHEMA_UNSUPPORTED",
            Self::CommonDataDirUnavailable => "COMMON_DATA_DIR_UNAVAILABLE",
            Self::CommonDb => "COMMON_DB",
            Self::CommonInternal => "COMMON_INTERNAL",
            Self::CommonIo => "COMMON_IO",
            Self::CommonJson => "COMMON_JSON",
            Self::CommonLogInit => "COMMON_LOG_INIT",
            Self::GameFileMissing => "GAME_FILE_MISSING",
            Self::GameVersionJsonInvalid => "GAME_VERSION_JSON_INVALID",
            Self::GameVersionNotFound => "GAME_VERSION_NOT_FOUND",
            Self::InstanceCorrupted => "INSTANCE_CORRUPTED",
            Self::InstanceInvalidName => "INSTANCE_INVALID_NAME",
            Self::InstanceNotFound => "INSTANCE_NOT_FOUND",
            Self::InstancePackInvalid => "INSTANCE_PACK_INVALID",
            Self::InstanceSchemaUnsupported => "INSTANCE_SCHEMA_UNSUPPORTED",
            Self::JavaArchMismatch => "JAVA_ARCH_MISMATCH",
            Self::JavaNotFound => "JAVA_NOT_FOUND",
            Self::JavaProbeFailed => "JAVA_PROBE_FAILED",
            Self::JavaRuntimeUnavailable => "JAVA_RUNTIME_UNAVAILABLE",
            Self::JavaVersionMismatch => "JAVA_VERSION_MISMATCH",
            Self::LaunchAlreadyRunning => "LAUNCH_ALREADY_RUNNING",
            Self::LaunchSpawnFailed => "LAUNCH_SPAWN_FAILED",
            Self::LaunchVersionNotInstalled => "LAUNCH_VERSION_NOT_INSTALLED",
            Self::LoaderInstallerInvalid => "LOADER_INSTALLER_INVALID",
            Self::LoaderProcessorFailed => "LOADER_PROCESSOR_FAILED",
            Self::LoaderUnsupportedVersion => "LOADER_UNSUPPORTED_VERSION",
            Self::LoaderVersionNotFound => "LOADER_VERSION_NOT_FOUND",
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
            ErrorCode::AppearanceImageMissing,
            ErrorCode::AppearanceImageUnsupported,
            ErrorCode::AuthAccountNotFound,
            ErrorCode::AuthAuthlibServerInvalid,
            ErrorCode::AuthNoAccount,
            ErrorCode::AuthOfflineInvalidName,
            ErrorCode::AuthSecretStore,
            ErrorCode::AuthYggdrasilInvalidCredentials,
            ErrorCode::AuthYggdrasilNoProfile,
            ErrorCode::AuthYggdrasilSessionExpired,
            ErrorCode::CommonCancelled,
            ErrorCode::CommonConfigSchemaUnsupported,
            ErrorCode::CommonDataDirUnavailable,
            ErrorCode::CommonDb,
            ErrorCode::CommonInternal,
            ErrorCode::CommonIo,
            ErrorCode::CommonJson,
            ErrorCode::CommonLogInit,
            ErrorCode::GameFileMissing,
            ErrorCode::GameVersionJsonInvalid,
            ErrorCode::GameVersionNotFound,
            ErrorCode::InstanceCorrupted,
            ErrorCode::InstanceInvalidName,
            ErrorCode::InstanceNotFound,
            ErrorCode::InstancePackInvalid,
            ErrorCode::InstanceSchemaUnsupported,
            ErrorCode::JavaArchMismatch,
            ErrorCode::JavaNotFound,
            ErrorCode::JavaProbeFailed,
            ErrorCode::JavaRuntimeUnavailable,
            ErrorCode::JavaVersionMismatch,
            ErrorCode::LaunchAlreadyRunning,
            ErrorCode::LaunchSpawnFailed,
            ErrorCode::LaunchVersionNotInstalled,
            ErrorCode::LoaderInstallerInvalid,
            ErrorCode::LoaderProcessorFailed,
            ErrorCode::LoaderUnsupportedVersion,
            ErrorCode::LoaderVersionNotFound,
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
