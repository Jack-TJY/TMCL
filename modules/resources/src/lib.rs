//! tmcl-resources：Modrinth / CurseForge 客户端、资源浏览、整合包。
//!
//! 功能模块 crate，只依赖 `core/*`，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`modrinth`]：Modrinth API v2 的响应结构。

pub mod modrinth;
