//! tmcl-loader：Fabric / Quilt / Forge / NeoForge 安装。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`list`]：列出与游戏版本匹配的加载器版本；
//! - [`fabric`]、[`quilt`]：下载 profile JSON，写成继承原版的版本 JSON；
//! - [`forge`]、[`neoforge`]：下载安装器 jar，解析 `install_profile.json`，下载依赖库并在本地 Java 上
//!   运行 [`processors`]；
//! - [`LoaderInstaller::install`]：按种类分派。
//!
//! 安装结果是 `shared/versions/<id>/<id>.json` 中一个带 `inheritsFrom` 的版本，`<id>` 由
//! [`version_id`] 决定。版本 JSON 在其余文件全部就绪后才原子写入，安装中途失败或取消时
//! 不会留下可被选中的半成品版本；实例的 `loader` 字段应在安装成功后再由调用方更新。
//!
//! 下载走 [`Downloader`] 的下载源：Forge、NeoForge、Fabric 在 `auto` 策略下先试 BMCLAPI
//! 再回退官方源，Quilt 只走官方源（BMCLAPI 未收录）。

pub mod fabric;
pub mod forge;
mod installer;
pub mod list;
pub mod neoforge;
pub mod processors;
mod profile;
pub mod quilt;

use std::path::PathBuf;

use tmcl_common::{AppResult, DataPaths, TaskCtx};
use tmcl_game::{InstallOptions, InstallReport, VersionJson};
use tmcl_net::Downloader;

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
    /// BMCLAPI 根地址：下载源启用镜像时，Forge 列表与推荐版本先从这里获取。
    pub bmclapi: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            fabric_meta: "https://meta.fabricmc.net".to_owned(),
            quilt_meta: "https://meta.quiltmc.org".to_owned(),
            forge_maven: "https://maven.minecraftforge.net".to_owned(),
            forge_files: "https://files.minecraftforge.net".to_owned(),
            neoforge_maven: "https://maven.neoforged.net/releases".to_owned(),
            bmclapi: "https://bmclapi2.bangbang93.com".to_owned(),
        }
    }
}

/// 运行 Forge / NeoForge 处理步骤使用的 Java，与 `instance.json` 的 `java.mode` 对应。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum JavaChoice {
    /// 按原版要求的主版本自动探测，没有时下载（[`tmcl_java::ensure`]）。
    #[default]
    Auto,
    /// 使用指定的 `java` 可执行文件。
    Path(PathBuf),
}

/// 安装加载器所需的环境：下载引擎、数据目录与各端点。
#[derive(Debug, Clone)]
pub struct LoaderInstaller {
    downloader: Downloader,
    paths: DataPaths,
    endpoints: Endpoints,
    game_options: InstallOptions,
    java: JavaChoice,
}

impl LoaderInstaller {
    /// 使用官方端点创建。
    pub fn new(downloader: Downloader, paths: DataPaths) -> Self {
        Self {
            downloader,
            paths,
            endpoints: Endpoints::default(),
            game_options: InstallOptions::default(),
            java: JavaChoice::Auto,
        }
    }

    /// 替换加载器端点（测试用本地服务器）。
    pub fn with_endpoints(mut self, endpoints: Endpoints) -> Self {
        self.endpoints = endpoints;
        self
    }

    /// 替换补全原版时使用的选项（版本清单与资源地址）。
    pub fn with_game_options(mut self, options: InstallOptions) -> Self {
        self.game_options = options;
        self
    }

    /// 指定运行处理步骤的 Java。
    pub fn with_java(mut self, java: JavaChoice) -> Self {
        self.java = java;
        self
    }

    /// 在 `game_version` 上安装 `kind` 的 `loader_version`，必要时先补全原版。
    ///
    /// 成功时返回写入的版本号；失败或取消时不写入版本 JSON，已下载的共享文件保留以便重试。
    pub async fn install(
        &self,
        kind: LoaderKind,
        game_version: &str,
        loader_version: &str,
        ctx: &TaskCtx,
    ) -> AppResult<InstalledLoader> {
        match kind {
            LoaderKind::Fabric => fabric::install(self, game_version, loader_version, ctx).await,
            LoaderKind::Quilt => quilt::install(self, game_version, loader_version, ctx).await,
            LoaderKind::Forge => forge::install(self, game_version, loader_version, ctx).await,
            LoaderKind::Neoforge => {
                neoforge::install(self, game_version, loader_version, ctx).await
            }
        }
    }

    /// 运行 `parent`（原版）的处理步骤使用的 Java 可执行文件。
    async fn java_for(&self, parent: &VersionJson, ctx: &TaskCtx) -> AppResult<PathBuf> {
        match &self.java {
            JavaChoice::Path(path) => Ok(path.clone()),
            JavaChoice::Auto => {
                let java_dir = self.paths.java_dir();
                let runtime = tmcl_java::RuntimeInstaller::new(self.downloader.clone(), &java_dir);
                let component = parent
                    .java_version
                    .as_ref()
                    .and_then(|j| j.component.as_deref());
                let install = tmcl_java::ensure(
                    &runtime,
                    &tmcl_java::Sources::system(&java_dir),
                    parent.java_major(),
                    component,
                    ctx,
                )
                .await?;
                Ok(install.path)
            }
        }
    }

    /// 下载引擎。
    pub fn downloader(&self) -> &Downloader {
        &self.downloader
    }

    /// 数据目录。
    pub fn paths(&self) -> &DataPaths {
        &self.paths
    }

    /// 端点。
    pub fn endpoints(&self) -> &Endpoints {
        &self.endpoints
    }
}

/// 一次安装的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledLoader {
    /// 写入的版本号，见 [`version_id`]。
    pub version_id: String,
    /// 原版与加载器文件的下载统计。
    pub report: InstallReport,
}
