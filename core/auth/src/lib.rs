//! tmcl-auth：微软 / 离线 / authlib-injector 账号。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。

pub mod config;

pub use config::{ms_client_id, ms_login_enabled};
