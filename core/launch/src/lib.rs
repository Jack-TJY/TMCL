//! tmcl-launch：启动参数拼装、进程管理、日志流。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。

pub mod args;
pub mod process;

pub use args::{LaunchAccount, LaunchCommand, LaunchSpec, build_command};
pub use process::{ExitInfo, GameProcess, LogStream, spawn};
