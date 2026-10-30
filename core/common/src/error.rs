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
    /// Minecraft 服务拒绝本启动器的 Azure 应用（`login_with_xbox` 返回 403，应用未获 Mojang 批准或批准被撤销）。
    AuthMcAppNotApproved,
    /// 账号拥有 Minecraft 但还没有创建玩家档案（`/minecraft/profile` 返回 404）。
    AuthMcNoProfile,
    /// 账号没有购买 Minecraft: Java Edition（权益列表为空且没有玩家档案）。
    AuthMcNotOwned,
    /// 设备码在用户完成登录前过期（`expired_token`）。
    AuthMsCodeExpired,
    /// 用户在微软登录页拒绝了授权（`authorization_declined`）。
    AuthMsDeclined,
    /// 构建时没有注入 `TMCL_MS_CLIENT_ID`，微软登录不可用。
    AuthMsDisabled,
    /// 微软账号的刷新令牌已失效（`invalid_grant`）或密钥库中没有令牌，需要重新登录。
    AuthMsSessionExpired,
    /// 没有可用于启动的账号（账号列表为空或未选默认账号）。
    AuthNoAccount,
    /// 离线账号用户名不符合 `^[A-Za-z0-9_]{3,16}$`。
    AuthOfflineInvalidName,
    /// 无法读写系统密钥库（Windows 凭据管理器、macOS 钥匙串、Linux Secret Service）。
    AuthSecretStore,
    /// XSTS 返回 `XErr` 2148916238：未成年账号需要加入家庭组。
    AuthXstsChildAccount,
    /// XSTS 以其他 `XErr` 拒绝授权（如地区不可用、需要成人验证）。
    AuthXstsDenied,
    /// XSTS 返回 `XErr` 2148916233：微软账号还没有 Xbox 档案。
    AuthXstsNoXboxAccount,
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
    /// EasyTier 安装包的 SHA-256 与内置值不符，或包内缺少所需文件。
    P2pBinaryInvalid,
    /// EasyTier 节点地址无效（不是 `协议://主机[:端口]`，或协议不受支持）。
    P2pNodeInvalid,
    /// 房间码格式错误或校验不通过（Scaffolding `U/NNNN-NNNN-SSSS-SSSS`）。
    P2pRoomCodeInvalid,
    /// EasyTier 进程意外退出（启动失败或运行中崩溃）。
    P2pTunnelExited,
    /// 当前操作系统或 CPU 架构没有可用的 EasyTier 程序。
    P2pUnsupportedPlatform,
    /// 本次构建没有 CurseForge API Key，CurseForge 来源不可用。
    ResourcesCurseforgeDisabled,
    /// 必需前置在 Modrinth 上没有适用于该实例的版本（或项目已不存在）。
    ResourcesDependencyUnavailable,
    /// 实例中找不到要启用、禁用或删除的资源文件。
    ResourcesFileNotFound,
    /// 要安装的资源与已安装或同批安装的资源不兼容（`incompatible` 依赖）。
    ResourcesIncompatible,
    /// CurseForge 上的作者禁止第三方下载该文件，需要用户在网页上手动下载。
    ResourcesManualDownload,
    /// 整合包文件无效：不是 zip、缺少或损坏 `modrinth.index.json`，或其中的文件路径、哈希、下载地址非法。
    ResourcesModpackInvalid,
    /// 整合包的格式版本、游戏或依赖（加载器）不受支持。
    ResourcesModpackUnsupported,
    /// Modrinth 上找不到要安装的版本，或该版本没有文件。
    ResourcesVersionNotFound,
    /// 数据包要安装到的存档不存在或名称非法。
    ResourcesWorldNotFound,
    /// SakuraFrp 账号已被冻结（`/user/info` 返回 `ban`）。
    SakuraAccountBanned,
    /// 已在樱花穿透联机中，同一时间只能有一个房间。
    SakuraAlreadyRunning,
    /// SakuraFrp 接口拒绝请求（4xx），`msg` 为服务端给出的说明。
    SakuraApiError,
    /// frpc 无法启动、提前退出或超时没有给出联机地址。
    SakuraFrpcFailed,
    /// 官方没有提供适用于当前平台的 frpc。
    SakuraFrpcUnavailable,
    /// SakuraFrp 访问密钥无效或已重置（接口返回 401）。
    SakuraKeyInvalid,
    /// 还没有保存 SakuraFrp 访问密钥。
    SakuraKeyMissing,
    /// 公开房间选了不允许穿透公开游戏服务器的节点。
    SakuraNodeNotPublic,
    /// 所选节点不存在、离线或已满载。
    SakuraNodeUnavailable,
    /// SakuraFrp 账号未完成实名认证。
    SakuraRealnameRequired,
    /// 皮肤文件不是 PNG，或尺寸不是 64×64 / 64×32（正版上传时服务器拒收也用此码）。
    SkinInvalidImage,
    /// 本地皮肤库中没有该皮肤（已被删除或 ID 无效）。
    SkinNotFound,
    /// 设置同步主配置无效：`master.json` 损坏、格式版本不受支持，或保存的选项键/值非法。
    SyncMasterInvalid,
    /// 还没有保存设置同步的主配置。
    SyncNoMaster,
    /// 作为来源的实例还没有 `options.txt`（从未启动过游戏）。
    SyncSourceNoOptions,
}

impl ErrorCode {
    /// 返回序列化后的错误码字符串，例如 `COMMON_IO`。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppearanceImageMissing => "APPEARANCE_IMAGE_MISSING",
            Self::AppearanceImageUnsupported => "APPEARANCE_IMAGE_UNSUPPORTED",
            Self::AuthAccountNotFound => "AUTH_ACCOUNT_NOT_FOUND",
            Self::AuthAuthlibServerInvalid => "AUTH_AUTHLIB_SERVER_INVALID",
            Self::AuthMcAppNotApproved => "AUTH_MC_APP_NOT_APPROVED",
            Self::AuthMcNoProfile => "AUTH_MC_NO_PROFILE",
            Self::AuthMcNotOwned => "AUTH_MC_NOT_OWNED",
            Self::AuthMsCodeExpired => "AUTH_MS_CODE_EXPIRED",
            Self::AuthMsDeclined => "AUTH_MS_DECLINED",
            Self::AuthMsDisabled => "AUTH_MS_DISABLED",
            Self::AuthMsSessionExpired => "AUTH_MS_SESSION_EXPIRED",
            Self::AuthNoAccount => "AUTH_NO_ACCOUNT",
            Self::AuthOfflineInvalidName => "AUTH_OFFLINE_INVALID_NAME",
            Self::AuthSecretStore => "AUTH_SECRET_STORE",
            Self::AuthXstsChildAccount => "AUTH_XSTS_CHILD_ACCOUNT",
            Self::AuthXstsDenied => "AUTH_XSTS_DENIED",
            Self::AuthXstsNoXboxAccount => "AUTH_XSTS_NO_XBOX_ACCOUNT",
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
            Self::P2pBinaryInvalid => "P2P_BINARY_INVALID",
            Self::P2pNodeInvalid => "P2P_NODE_INVALID",
            Self::P2pRoomCodeInvalid => "P2P_ROOM_CODE_INVALID",
            Self::P2pTunnelExited => "P2P_TUNNEL_EXITED",
            Self::P2pUnsupportedPlatform => "P2P_UNSUPPORTED_PLATFORM",
            Self::ResourcesCurseforgeDisabled => "RESOURCES_CURSEFORGE_DISABLED",
            Self::ResourcesDependencyUnavailable => "RESOURCES_DEPENDENCY_UNAVAILABLE",
            Self::ResourcesFileNotFound => "RESOURCES_FILE_NOT_FOUND",
            Self::ResourcesIncompatible => "RESOURCES_INCOMPATIBLE",
            Self::ResourcesManualDownload => "RESOURCES_MANUAL_DOWNLOAD",
            Self::ResourcesModpackInvalid => "RESOURCES_MODPACK_INVALID",
            Self::ResourcesModpackUnsupported => "RESOURCES_MODPACK_UNSUPPORTED",
            Self::ResourcesVersionNotFound => "RESOURCES_VERSION_NOT_FOUND",
            Self::ResourcesWorldNotFound => "RESOURCES_WORLD_NOT_FOUND",
            Self::SakuraAccountBanned => "SAKURA_ACCOUNT_BANNED",
            Self::SakuraAlreadyRunning => "SAKURA_ALREADY_RUNNING",
            Self::SakuraApiError => "SAKURA_API_ERROR",
            Self::SakuraFrpcFailed => "SAKURA_FRPC_FAILED",
            Self::SakuraFrpcUnavailable => "SAKURA_FRPC_UNAVAILABLE",
            Self::SakuraKeyInvalid => "SAKURA_KEY_INVALID",
            Self::SakuraKeyMissing => "SAKURA_KEY_MISSING",
            Self::SakuraNodeNotPublic => "SAKURA_NODE_NOT_PUBLIC",
            Self::SakuraNodeUnavailable => "SAKURA_NODE_UNAVAILABLE",
            Self::SakuraRealnameRequired => "SAKURA_REALNAME_REQUIRED",
            Self::SkinInvalidImage => "SKIN_INVALID_IMAGE",
            Self::SkinNotFound => "SKIN_NOT_FOUND",
            Self::SyncMasterInvalid => "SYNC_MASTER_INVALID",
            Self::SyncNoMaster => "SYNC_NO_MASTER",
            Self::SyncSourceNoOptions => "SYNC_SOURCE_NO_OPTIONS",
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
            ErrorCode::AuthMcAppNotApproved,
            ErrorCode::AuthMcNoProfile,
            ErrorCode::AuthMcNotOwned,
            ErrorCode::AuthMsCodeExpired,
            ErrorCode::AuthMsDeclined,
            ErrorCode::AuthMsDisabled,
            ErrorCode::AuthMsSessionExpired,
            ErrorCode::AuthNoAccount,
            ErrorCode::AuthOfflineInvalidName,
            ErrorCode::AuthSecretStore,
            ErrorCode::AuthXstsChildAccount,
            ErrorCode::AuthXstsDenied,
            ErrorCode::AuthXstsNoXboxAccount,
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
            ErrorCode::P2pBinaryInvalid,
            ErrorCode::P2pNodeInvalid,
            ErrorCode::P2pRoomCodeInvalid,
            ErrorCode::P2pTunnelExited,
            ErrorCode::P2pUnsupportedPlatform,
            ErrorCode::ResourcesCurseforgeDisabled,
            ErrorCode::ResourcesDependencyUnavailable,
            ErrorCode::ResourcesFileNotFound,
            ErrorCode::ResourcesIncompatible,
            ErrorCode::ResourcesManualDownload,
            ErrorCode::ResourcesModpackInvalid,
            ErrorCode::ResourcesModpackUnsupported,
            ErrorCode::ResourcesVersionNotFound,
            ErrorCode::ResourcesWorldNotFound,
            ErrorCode::SakuraAccountBanned,
            ErrorCode::SakuraAlreadyRunning,
            ErrorCode::SakuraApiError,
            ErrorCode::SakuraFrpcFailed,
            ErrorCode::SakuraFrpcUnavailable,
            ErrorCode::SakuraKeyInvalid,
            ErrorCode::SakuraKeyMissing,
            ErrorCode::SakuraNodeNotPublic,
            ErrorCode::SakuraNodeUnavailable,
            ErrorCode::SakuraRealnameRequired,
            ErrorCode::SkinInvalidImage,
            ErrorCode::SkinNotFound,
            ErrorCode::SyncMasterInvalid,
            ErrorCode::SyncNoMaster,
            ErrorCode::SyncSourceNoOptions,
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
