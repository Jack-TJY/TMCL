//! 整合包：读取 Modrinth `.mrpack` 并安装为新实例（CurseForge 整合包见 M2-02）。
//!
//! - [`mrpack`]：解析与校验 `modrinth.index.json`，解压 `overrides/`；
//! - [`install`]：新建实例、安装加载器、下载文件，失败时回滚；
//! - [`source`]：在 Modrinth 搜索整合包，把本地文件 / URL / Modrinth 版本准备成本地文件。

pub mod install;
pub mod mrpack;
pub mod source;

pub use install::ModpackInstaller;
pub use mrpack::{ModpackInfo, Mrpack, PackFile};
pub use source::{FetchedPack, ModpackSource};
