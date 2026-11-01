//! Fabric API：安装 Fabric 加载器时默认一同装入实例的 `mods/`。
//!
//! 绝大多数 Fabric 模组都依赖 Fabric API，缺了它游戏会在启动时崩溃。整合包自带模组，
//! 不走这里。
//!
//! - 版本：Modrinth 上该游戏版本、Fabric 加载器的最新正式版（没有正式版时取最新版）。
//! - 已安装（文件名以 `fabric-api-` 开头，或按 SHA1 在 Modrinth 反查属于 Fabric API）时跳过。
//! - 该游戏版本没有 Fabric API 时跳过并记日志，不算失败。

use std::path::Path;

use tmcl_common::{AppResult, TaskCtx};
use tmcl_net::Downloader;

use crate::browse::{ResourceKind, is_not_found};
use crate::install::{INSTALL_STAGE, file_name, io_error, kind_dir, local_files, scan_installed};
use crate::modrinth::{ModrinthClient, VersionFilter, VersionType};

/// Fabric API 在 Modrinth 上的项目 ID（短名 `fabric-api`）。
pub const FABRIC_API_PROJECT_ID: &str = "P7dR8mSH";

/// [`install_fabric_api`] 的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FabricApiOutcome {
    /// 已下载，带文件名。
    Installed(String),
    /// `mods/` 中已有 Fabric API，未改动。
    AlreadyInstalled,
    /// 该游戏版本没有 Fabric API。
    Unavailable,
}

/// 为实例安装与 `game_version` 匹配的 Fabric API（`game_dir` 为实例的 `game/`）。
///
/// 查询或下载失败返回 `NET_*`；取消返回 `COMMON_CANCELLED`。
pub async fn install_fabric_api(
    client: &ModrinthClient,
    downloader: &Downloader,
    game_dir: &Path,
    game_version: &str,
    ctx: &TaskCtx,
) -> AppResult<FabricApiOutcome> {
    let filter = VersionFilter {
        without_changelog: true,
        ..VersionFilter::for_instance(game_version, "fabric")
    };
    let versions = match client
        .project_versions(FABRIC_API_PROJECT_ID, &filter, ctx)
        .await
    {
        Ok(versions) => versions,
        Err(err) if is_not_found(&err) => Vec::new(),
        Err(err) => return Err(err),
    };
    let pick = |release_only: bool| {
        versions
            .iter()
            .filter(|v| !release_only || v.version_type == VersionType::Release)
            .find_map(|v| v.primary_file().map(|file| (v, file)))
    };
    let Some((version, file)) = pick(true).or_else(|| pick(false)) else {
        tracing::info!(
            game_version,
            "fabric api: no build for this game version, skipped"
        );
        return Ok(FabricApiOutcome::Unavailable);
    };

    let mods = kind_dir(game_dir, ResourceKind::Mod, None)?;
    if is_installed(client, &mods, ctx).await? {
        tracing::info!(game_version, "fabric api: already installed, skipped");
        return Ok(FabricApiOutcome::AlreadyInstalled);
    }
    std::fs::create_dir_all(&mods).map_err(|e| io_error(&mods, &e))?;
    let item = file.download_item(&mods);
    let name = file_name(&item.path);
    downloader
        .download_all(vec![item], INSTALL_STAGE, ctx)
        .await?;
    tracing::info!(game_version, version = %version.version_number, "fabric api: installed");
    Ok(FabricApiOutcome::Installed(name))
}

/// `mods/` 中是否已有 Fabric API（含已禁用的）：先看文件名，再按 SHA1 在 Modrinth 反查。
async fn is_installed(client: &ModrinthClient, mods: &Path, ctx: &TaskCtx) -> AppResult<bool> {
    let files = local_files(mods, ctx).await?;
    if files
        .iter()
        .any(|f| looks_like_fabric_api(&file_name(&f.path)))
    {
        return Ok(true);
    }
    if files.is_empty() {
        return Ok(false);
    }
    let installed = scan_installed(client, &[mods.to_path_buf()], ctx).await?;
    Ok(installed
        .iter()
        .any(|i| i.version.project_id == FABRIC_API_PROJECT_ID))
}

/// Fabric API 的发布文件名形如 `fabric-api-0.92.2+1.20.1.jar`。
fn looks_like_fabric_api(name: &str) -> bool {
    name.to_ascii_lowercase()
        .strip_prefix("fabric-api-")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_fabric_api_file_names() {
        for ok in [
            "fabric-api-0.92.2+1.20.1.jar",
            "Fabric-API-0.100.0+1.21.jar.disabled",
        ] {
            assert!(looks_like_fabric_api(ok), "{ok}");
        }
        for bad in [
            "fabric-api.jar",
            "fabric-api-base-0.4.jar",
            "fabric-language-kotlin-1.10.jar",
            "jade-11.jar",
        ] {
            assert!(!looks_like_fabric_api(bad), "{bad}");
        }
    }
}
