//! `skin_*` 命令（PLAN.md M2-07）：本地皮肤库、外置登录账号的皮肤站。
//!
//! 皮肤库位于 `<data_root>/skins/`，界面经 `skin_library_read` 取回图片内容，不直接访问文件系统。

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Response;
use tmcl_auth::accounts::{self, Account};
use tmcl_auth::{AccountKind, AuthlibServer};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::HttpClient;
use tmcl_skin::library::MAX_SKIN_BYTES;
use tmcl_skin::{SkinEntry, SkinLibrary, SkinVariant, yggdrasil};
use ts_rs::TS;

use crate::commands::settings::open_in_file_manager;
use crate::state::AppState;

/// `skin_library_list` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SkinLibraryList {
    /// 最近加入的在前。
    pub skins: Vec<SkinEntry>,
}

/// `skin_library_import` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SkinImportArgs {
    /// 用户选中的 PNG 的绝对路径。
    pub path: String,
    /// 皮肤名称；省略时用文件名（不含扩展名）。
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    pub variant: SkinVariant,
}

/// 以皮肤 ID 为参数的命令参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SkinIdArgs {
    /// 皮肤 ID（文件内容的 SHA-256）。
    pub id: String,
}

/// `skin_library_update` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SkinUpdateArgs {
    pub id: String,
    pub name: String,
    pub variant: SkinVariant,
}

/// 以账号 ID 为参数的命令参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SkinAccountArgs {
    /// 账号 ID。
    pub account_id: String,
}

/// 外置登录账号的皮肤站页面。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SkinSite {
    pub url: String,
}

fn library(state: &AppState) -> AppResult<SkinLibrary> {
    Ok(SkinLibrary::new(state.data()?.skins_dir()))
}

/// 皮肤库中的所有皮肤。
#[tauri::command]
pub async fn skin_library_list(state: State<'_, AppState>) -> Result<SkinLibraryList, AppError> {
    Ok(SkinLibraryList {
        skins: library(&state)?.list()?,
    })
}

/// 把用户选中的 PNG 加入皮肤库；不是 64×64 / 64×32 的 PNG 返回 `SKIN_INVALID_IMAGE`，
/// 同一张图片已在库中时返回已有的条目。
#[tauri::command]
pub async fn skin_library_import(
    state: State<'_, AppState>,
    args: SkinImportArgs,
) -> Result<SkinEntry, AppError> {
    let source = Path::new(&args.path);
    let io_error = |e: std::io::Error| AppError::from(e).with_param("path", source.display());
    if fs::metadata(source).map_err(io_error)?.len() > MAX_SKIN_BYTES as u64 {
        return Err(AppError::new(ErrorCode::SkinInvalidImage));
    }
    let bytes = fs::read(source).map_err(io_error)?;
    let name = args.name.unwrap_or_else(|| {
        source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    library(&state)?.add(&bytes, &name, args.variant)
}

/// 读取皮肤图片，以二进制返回（前端收到 `ArrayBuffer`）。
#[tauri::command]
pub async fn skin_library_read(
    state: State<'_, AppState>,
    args: SkinIdArgs,
) -> Result<Response, AppError> {
    library(&state)?.read(&args.id).map(Response::new)
}

/// 修改皮肤名称与模型。
#[tauri::command]
pub async fn skin_library_update(
    state: State<'_, AppState>,
    args: SkinUpdateArgs,
) -> Result<SkinEntry, AppError> {
    library(&state)?.update(&args.id, &args.name, args.variant)
}

/// 从皮肤库删除皮肤。
#[tauri::command]
pub async fn skin_library_remove(
    state: State<'_, AppState>,
    args: SkinIdArgs,
) -> Result<(), AppError> {
    library(&state)?.remove(&args.id)
}

/// 外置登录账号的皮肤站；不是外置登录账号返回 `COMMON_INTERNAL`（`params.kind`）。
fn authlib_server(state: &AppState, account_id: &str) -> AppResult<AuthlibServer> {
    let account: Account = state
        .with_db(|_, db| accounts::get(db, account_id))?
        .ok_or_else(|| {
            AppError::new(ErrorCode::AuthAccountNotFound).with_param("id", account_id)
        })?;
    match (account.kind, account.server) {
        (AccountKind::Authlib, Some(server)) => Ok(server),
        (kind, _) => Err(AppError::new(ErrorCode::CommonInternal)
            .with_param("kind", format!("{kind:?}").to_lowercase())),
    }
}

async fn site_url(state: &AppState, account_id: &str) -> AppResult<SkinSite> {
    let server = authlib_server(state, account_id)?;
    let ctx = TaskCtx::noop("skin");
    let url = yggdrasil::site_url(&HttpClient::new()?, &server.api_url, &ctx).await?;
    Ok(SkinSite { url })
}

/// 外置登录账号的皮肤站页面地址（`meta.links.homepage`，没有时为站点根）。
#[tauri::command]
pub async fn skin_site_url(
    state: State<'_, AppState>,
    args: SkinAccountArgs,
) -> Result<SkinSite, AppError> {
    site_url(&state, &args.account_id).await
}

/// 在系统浏览器中打开外置登录账号的皮肤站页面，返回打开的地址。
#[tauri::command]
pub async fn skin_open_site(
    state: State<'_, AppState>,
    args: SkinAccountArgs,
) -> Result<SkinSite, AppError> {
    let site = site_url(&state, &args.account_id).await?;
    open_in_file_manager(Path::new(&site.url))?;
    Ok(site)
}
