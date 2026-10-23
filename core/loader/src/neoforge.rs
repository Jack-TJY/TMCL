//! NeoForge：从 `maven.neoforged.net` 下载安装器 jar 并安装，见 [`crate::installer`]。
//!
//! 1.20.1 的构建以 `net.neoforged:forge:1.20.1-<版本>` 发布，其余为 `net.neoforged:neoforge:<版本>`。

use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};

use crate::installer::install_from_installer;
use crate::list::{NEOFORGE_LEGACY_GAME, neoforge_game};
use crate::{InstalledLoader, LoaderInstaller, LoaderKind, display_name};

/// 安装器地址；`loader_version` 与 `game_version` 不对应时为 `None`。
pub fn installer_url(maven: &str, game_version: &str, loader_version: &str) -> Option<String> {
    if game_version == NEOFORGE_LEGACY_GAME {
        let v = format!("{game_version}-{loader_version}");
        return Some(format!(
            "{maven}/net/neoforged/forge/{v}/forge-{v}-installer.jar"
        ));
    }
    (neoforge_game(loader_version).as_deref() == Some(game_version)).then(|| {
        format!(
            "{maven}/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar"
        )
    })
}

/// 在 `game_version` 上安装 NeoForge `loader_version`（如 `21.1.209`），必要时先补全原版。
///
/// 版本号与游戏版本不对应或 maven 中没有该版本时返回 `LOADER_VERSION_NOT_FOUND`。
pub async fn install(
    installer: &LoaderInstaller,
    game_version: &str,
    loader_version: &str,
    ctx: &TaskCtx,
) -> AppResult<InstalledLoader> {
    let Some(url) = installer_url(
        &installer.endpoints().neoforge_maven,
        game_version,
        loader_version,
    ) else {
        return Err(AppError::new(ErrorCode::LoaderVersionNotFound)
            .with_param("kind", display_name(LoaderKind::Neoforge))
            .with_param("game", game_version)
            .with_param("version", loader_version));
    };
    install_from_installer(
        installer,
        LoaderKind::Neoforge,
        game_version,
        loader_version,
        &url,
        ctx,
    )
    .await
}
