//! tmcl-launch：启动参数拼装、启动前检查、进程管理与日志流。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`precheck`]：补全文件、选择 Java、解压 natives，得到 [`PreparedLaunch`]；
//! - [`args`]：拼装命令，占位符见 [`placeholders`]，classpath 见 [`classpath`]；
//! - [`process`]：启动进程、批量转发日志、识别崩溃、强制结束。

pub mod args;
pub mod classpath;
pub mod placeholders;
pub mod precheck;
pub mod process;

pub use args::{LaunchAccount, LaunchCommand, LaunchSpec, build_command};
pub use precheck::{LaunchEnv, PreparedLaunch, prepare, version_id};
pub use process::{ExitInfo, GameProcess, KillHandle, LogStream, is_crash_line, spawn};
