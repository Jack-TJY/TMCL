//! `auth_*` 命令：账号列表、默认账号、删除，添加离线与外置登录账号（PLAN.md M1-08）。
//! 微软登录（`auth_ms_begin`）由 M0-10 追加。

use std::fmt;

use serde::Deserialize;
use tauri::State;
use tmcl_auth::accounts::{self, Account, AccountList, AuthlibServer};
use tmcl_auth::authlib;
use tmcl_auth::yggdrasil::{Yggdrasil, resolve_server};
use tmcl_common::{AppError, TaskCtx};
use tmcl_net::HttpClient;
use ts_rs::TS;

use crate::state::AppState;

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
