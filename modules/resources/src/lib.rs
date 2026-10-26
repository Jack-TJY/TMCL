//! tmcl-resources：Modrinth / CurseForge 客户端、资源浏览、整合包。
//!
//! 功能模块 crate，只依赖 `core/*`，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`modrinth`]：Modrinth API v2 客户端，含按文件哈希查询与更新检查，遵守速率限制。
//! - [`browse`]：按实例的游戏版本与加载器统一搜索 mod、资源包、光影、数据包。
//! - [`deps`]：递归解析必需前置，安装前检查不兼容。
//! - [`install`]：生成安装计划并下载到实例的对应目录。

pub mod browse;
pub mod deps;
pub mod install;
pub mod modrinth;

pub use browse::{
    BrowseQuery, InstanceTarget, ResourceChannel, ResourceHit, ResourceKind, ResourcePage,
    ResourceSort, ResourceVersion,
};
pub use deps::{Conflict, InstalledVersion, Resolution, Resolved, UnavailableDependency};
pub use install::{InstallPlan, InstallRequest, PlannedFile, ResourceInstaller};
