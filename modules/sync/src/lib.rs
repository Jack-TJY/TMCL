//! tmcl-sync：在实例之间同步 `options.txt` 中的设置与按键（PLAN.md M2-09）。
//!
//! 功能模块 crate，只依赖 `core/*`，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`options`]：解析与写回 `options.txt`，保留未知行与顺序。
//! - [`keymap`]：1.13 以后的键名与 1.12.2 及以前 LWJGL2 键码的双向转换。
//! - [`profile`]：同步类别与主配置 `<data_root>/sync/master.json`。
//! - [`apply`]：按类别与目标版本应用主配置，写入前备份，支持撤销。

pub mod apply;
pub mod keymap;
pub mod options;
pub mod profile;

pub use apply::{Applied, OptionChange};
pub use options::OptionsFile;
pub use profile::{Category, SyncOption, SyncProfile};
