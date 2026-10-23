//! tmcl-instance：实例 CRUD、导入导出。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`model`]：`instance.json` 结构与 `schema` 迁移；
//! - [`store`]：创建、列表（SQLite 索引 + 启动对账）、读取、修改、重命名、
//!   删除到回收站。

pub mod model;
pub mod store;
#[cfg(test)]
mod test_util;

pub use model::{
    INSTANCE_SCHEMA, InstanceConfig, JavaSetting, LoaderKind, LoaderSpec, MemorySetting,
    WindowSetting,
};
pub use store::{InstanceStore, InstanceSummary, ReconcileReport, TRASH_RETENTION};
