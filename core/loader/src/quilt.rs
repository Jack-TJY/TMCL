//! Quilt：从 `meta.quiltmc.org` 下载 profile JSON，直接写成继承原版的版本 JSON。
//!
//! BMCLAPI 未收录 Quilt，始终走官方源。

use tmcl_common::{AppResult, TaskCtx};

use crate::list::segment;
use crate::profile::install_profile;
use crate::{InstalledLoader, LoaderInstaller, LoaderKind};

/// profile JSON 地址：`<meta>/v3/versions/loader/<游戏>/<加载器>/profile/json`。
pub fn profile_url(meta: &str, game_version: &str, loader_version: &str) -> String {
    format!(
        "{meta}/v3/versions/loader/{}/{}/profile/json",
        segment(game_version),
        segment(loader_version)
    )
}

/// 在 `game_version` 上安装 Quilt `loader_version`，必要时先补全原版。
///
/// 阶段依次为原版补全的各阶段、`loader_profile`、`loader_libraries`。元数据中没有该组合时返回
/// `LOADER_VERSION_NOT_FOUND`；失败或取消时不写入版本 JSON。
pub async fn install(
    installer: &LoaderInstaller,
    game_version: &str,
    loader_version: &str,
    ctx: &TaskCtx,
) -> AppResult<InstalledLoader> {
    let url = profile_url(
        &installer.endpoints().quilt_meta,
        game_version,
        loader_version,
    );
    install_profile(
        installer,
        LoaderKind::Quilt,
        game_version,
        loader_version,
        &url,
        ctx,
    )
    .await
}
