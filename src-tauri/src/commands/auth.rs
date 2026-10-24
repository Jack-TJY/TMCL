//! `auth_*` 命令：账号列表、默认账号、删除，添加离线与外置登录账号（PLAN.md M1-08），
//! 微软设备码登录（M0-10）。

use std::fmt;
use std::path::Path;

use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime, State};
use tmcl_auth::accounts::{self, Account, AccountList, AuthlibServer};
use tmcl_auth::yggdrasil::{Yggdrasil, resolve_server};
use tmcl_auth::{authlib, store};
use tmcl_common::{AppError, ErrorCode, TaskCtx};
use tmcl_net::HttpClient;
use ts_rs::TS;

use crate::commands::settings::open_in_file_manager;
use crate::events::{AUTH_DEVICE_CODE, DeviceCode, TaskStarted, emit, spawn_task};
use crate::state::AppState;

/// 微软登录任务的种类。
pub const AUTH_MS_KIND: &str = "auth_ms";

/// 以账号 ID 为参数的命令参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct AuthIdArgs {
    /// 账号 ID。
    pub id: String,
}

/// `auth_add_offline` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct AuthAddOfflineArgs {
    /// 玩家名，须符合 `^[A-Za-z0-9_]{3,16}$`。
    pub name: String,
}

/// `auth_authlib_server` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct AuthAuthlibServerArgs {
    /// 用户输入的皮肤站首页或 API 地址。
    pub api_url: String,
}

/// `auth_add_authlib` 的参数。密码只用于本次登录，不保存、不写日志。
#[derive(Clone, PartialEq, Eq, Deserialize, TS)]
pub struct AuthAddAuthlibArgs {
    /// `auth_authlib_server` 返回的 API 地址。
    pub api_url: String,
    /// 皮肤站登录名（通常是邮箱）。
    pub username: String,
    /// 密码。
    pub password: String,
}

impl fmt::Debug for AuthAddAuthlibArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthAddAuthlibArgs")
            .field("api_url", &self.api_url)
            .field("username", &"***")
            .field("password", &"***")
            .finish()
    }
}

/// `auth_ms_open_verification` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct AuthMsOpenArgs {
    /// `auth:device-code` 给出的验证页地址。
    pub uri: String,
}

/// 账号列表与默认账号。
#[tauri::command]
pub async fn auth_list(state: State<'_, AppState>) -> Result<AccountList, AppError> {
    state.with_db(|_, db| accounts::list(db))
}

/// 设为启动时使用的账号；不存在时返回 `AUTH_ACCOUNT_NOT_FOUND`。
#[tauri::command]
pub async fn auth_set_default(
    state: State<'_, AppState>,
    args: AuthIdArgs,
) -> Result<(), AppError> {
    state.with_db(|_, db| accounts::set_default(db, &args.id))
}

/// 删除账号及其在系统密钥库中的令牌。
#[tauri::command]
pub async fn auth_remove(state: State<'_, AppState>, args: AuthIdArgs) -> Result<(), AppError> {
    let secrets = state.secrets.clone();
    state.with_db(|_, db| accounts::remove(db, secrets.as_ref(), &args.id))
}

/// 添加离线账号（仅限本地测试）并设为默认；同名账号已存在时返回该账号。
#[tauri::command]
pub async fn auth_add_offline(
    state: State<'_, AppState>,
    args: AuthAddOfflineArgs,
) -> Result<Account, AppError> {
    let secrets = state.secrets.clone();
    state.with_db(|_, db| accounts::add_offline(db, secrets.as_ref(), &args.name))
}

/// 读取皮肤站名称与规整后的 API 地址；不是皮肤站时返回 `AUTH_AUTHLIB_SERVER_INVALID`。
#[tauri::command]
pub async fn auth_authlib_server(args: AuthAuthlibServerArgs) -> Result<AuthlibServer, AppError> {
    let ctx = TaskCtx::noop("auth");
    let server = resolve_server(&HttpClient::new()?, &args.api_url, &ctx).await?;
    Ok(AuthlibServer {
        api_url: server.api_url,
        name: server.name,
    })
}

/// 用皮肤站账号登录，令牌存入系统密钥库，账号设为默认。
#[tauri::command]
pub async fn auth_add_authlib(
    state: State<'_, AppState>,
    args: AuthAddAuthlibArgs,
) -> Result<Account, AppError> {
    let ctx = TaskCtx::noop("auth");
    let server = resolve_server(&HttpClient::new()?, &args.api_url, &ctx).await?;
    let session = Yggdrasil::new()?
        .authenticate(&server.api_url, &args.username, &args.password, &ctx)
        .await?;
    let secrets = state.secrets.clone();
    state.with_db(|_, db| {
        authlib::save_session(
            db,
            secrets.as_ref(),
            &session,
            AuthlibServer {
                api_url: server.api_url,
                name: server.name,
            },
        )
    })
}

/// 开始微软设备码登录（任务种类 `auth_ms`）：拿到设备码后推送 `auth:device-code`，
/// 用户在浏览器中完成登录后走完 Xbox 与 Minecraft 登录，令牌存入系统密钥库，账号设为默认。
/// 取消任务即放弃登录。构建时没有 Client ID 返回 `AUTH_MS_DISABLED`。
#[tauri::command]
pub async fn auth_ms_begin<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<TaskStarted, AppError> {
    let ms = state.ms.clone()?;
    let handle = app.clone();
    Ok(spawn_task(&app, AUTH_MS_KIND, move |ctx| async move {
        let task_id = ctx.id;
        let session = store::login(
            &ms,
            |code| {
                emit(
                    &handle,
                    AUTH_DEVICE_CODE,
                    &DeviceCode {
                        task_id,
                        user_code: code.user_code.clone(),
                        verification_uri: code.verification_uri.clone(),
                        expires_in: code.expires_in,
                    },
                );
            },
            &ctx,
        )
        .await?;
        let state = handle.state::<AppState>();
        let secrets = state.secrets.clone();
        let account = state.with_db(|_, db| store::save_session(db, secrets.as_ref(), &session))?;
        tracing::info!(account = %account.id, "microsoft account signed in");
        Ok(account)
    }))
}

/// 在系统浏览器中打开设备码验证页。只接受微软域名下的 https 地址，
/// 其他地址返回 `COMMON_INTERNAL`（`params.uri`）。
#[tauri::command]
pub async fn auth_ms_open_verification(args: AuthMsOpenArgs) -> Result<(), AppError> {
    if !is_microsoft_page(&args.uri) {
        return Err(AppError::new(ErrorCode::CommonInternal).with_param("uri", &args.uri));
    }
    open_in_file_manager(Path::new(&args.uri))
}

/// `https://<microsoft.com 或 live.com 的子域名>/<路径>`，且不含会被 shell 解释的字符。
fn is_microsoft_page(uri: &str) -> bool {
    let Some(rest) = uri.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or_default();
    let trusted = ["microsoft.com", "live.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")));
    let safe = uri
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-._~/:?=%".contains(&b));
    trusted && safe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_microsoft_pages_are_opened() {
        for ok in [
            "https://www.microsoft.com/link",
            "https://microsoft.com/devicelogin",
            "https://login.live.com/oauth20_remoteconnect.srf",
        ] {
            assert!(is_microsoft_page(ok), "{ok}");
        }
        for bad in [
            "http://www.microsoft.com/link",
            "https://evilmicrosoft.com/link",
            "https://microsoft.com.evil.test/",
            "https://www.microsoft.com/link&calc",
            "file:///C:/Windows",
            "https://www.microsoft.com/\"x\"",
        ] {
            assert!(!is_microsoft_page(bad), "{bad}");
        }
    }
}
