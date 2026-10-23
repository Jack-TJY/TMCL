//! tmcl-loader：Fabric / Quilt / Forge / NeoForge 安装。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`list`]：列出与游戏版本匹配的加载器版本。
//!
//! 安装结果是 `shared/versions/<id>/<id>.json` 中一个带 `inheritsFrom` 的版本，`<id>` 由
//! [`version_id`] 决定。
//!
//! 下载走 [`tmcl_net::Downloader`] 的下载源：Forge、NeoForge、Fabric 在 `auto` 策略下先试 BMCLAPI
//! 再回退官方源，Quilt 只走官方源（BMCLAPI 未收录）。

pub mod list;

pub use list::{LoaderVersion, compare_versions, list_versions};
pub use tmcl_instance::LoaderKind;

/// 加载器的项目名称（专有名词，不翻译），用作错误码的 `kind` 参数。
pub fn display_name(kind: LoaderKind) -> &'static str {
    match kind {
        LoaderKind::Fabric => "Fabric",
        LoaderKind::Quilt => "Quilt",
        LoaderKind::Forge => "Forge",
        LoaderKind::Neoforge => "NeoForge",
    }
}

/// 安装后的版本号：Fabric、Quilt 与官方 profile 相同（`fabric-loader-<加载器>-<游戏>`），
/// Forge、NeoForge 统一为 `<游戏>-forge-<加载器>`、`<游戏>-neoforge-<加载器>`。
pub fn version_id(kind: LoaderKind, game_version: &str, loader_version: &str) -> String {
    match kind {
        LoaderKind::Fabric => format!("fabric-loader-{loader_version}-{game_version}"),
        LoaderKind::Quilt => format!("quilt-loader-{loader_version}-{game_version}"),
        LoaderKind::Forge => format!("{game_version}-forge-{loader_version}"),
        LoaderKind::Neoforge => format!("{game_version}-neoforge-{loader_version}"),
    }
}

/// 各加载器元数据与 maven 仓库的地址（官方地址，镜像改写由下载源处理）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// Fabric 元数据，`<根>/v2/versions/...`。
    pub fabric_meta: String,
    /// Quilt 元数据，`<根>/v3/versions/...`。
    pub quilt_meta: String,
    /// Forge maven 仓库。
    pub forge_maven: String,
    /// Forge 网站，提供 `promotions_slim.json`。
    pub forge_files: String,
    /// NeoForge maven 仓库（`releases`）。
    pub neoforge_maven: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            fabric_meta: "https://meta.fabricmc.net".to_owned(),
            quilt_meta: "https://meta.quiltmc.org".to_owned(),
            forge_maven: "https://maven.minecraftforge.net".to_owned(),
            forge_files: "https://files.minecraftforge.net".to_owned(),
            neoforge_maven: "https://maven.neoforged.net/releases".to_owned(),
        }
    }
}
