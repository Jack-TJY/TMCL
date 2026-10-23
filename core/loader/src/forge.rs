//! Forge：从 `maven.minecraftforge.net` 下载安装器 jar 并安装，见 [`crate::installer`]。
//!
//! 支持 1.7.10 起的版本；1.12.2 的早期构建与更早的版本使用旧格式安装器。

use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};

use crate::installer::install_from_installer;
use crate::list::forge_versions;
use crate::{InstalledLoader, LoaderInstaller, LoaderKind, display_name};

/// 安装器地址：`<maven>/net/minecraftforge/forge/<maven 版本>/forge-<maven 版本>-installer.jar`。
pub fn installer_url(maven: &str, maven_version: &str) -> String {
    format!("{maven}/net/minecraftforge/forge/{maven_version}/forge-{maven_version}-installer.jar")
}

/// 在 `game_version` 上安装 Forge `loader_version`（如 `47.2.0`），必要时先补全原版。
///
/// 1.7.10 以前返回 `LOADER_UNSUPPORTED_VERSION`；maven 中没有该版本时返回 `LOADER_VERSION_NOT_FOUND`。
pub async fn install(
    installer: &LoaderInstaller,
    game_version: &str,
    loader_version: &str,
    ctx: &TaskCtx,
) -> AppResult<InstalledLoader> {
    let endpoints = installer.endpoints();
    let versions = forge_versions(installer.downloader(), endpoints, game_version, ctx).await?;
    let Some((_, maven_version)) = versions.into_iter().find(|(v, _)| v == loader_version) else {
        return Err(AppError::new(ErrorCode::LoaderVersionNotFound)
            .with_param("kind", display_name(LoaderKind::Forge))
            .with_param("game", game_version)
            .with_param("version", loader_version));
    };
    install_from_installer(
        installer,
        LoaderKind::Forge,
        game_version,
        loader_version,
        &installer_url(&endpoints.forge_maven, &maven_version),
        ctx,
    )
    .await
}
