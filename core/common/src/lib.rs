//! tmcl-common：错误码、路径、配置、日志脱敏、任务与进度、SQLite。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。

pub mod config;
pub mod db;
pub mod diagnostics;
pub mod error;
pub mod log;
pub mod paths;
pub mod task;

pub use config::{Config, LoadedConfig};
pub use db::Db;
pub use error::{AppError, AppResult, ErrorCode};
pub use log::{LogGuard, LogOptions, init_logging, redact};
pub use paths::{DataPaths, LinuxEnv, write_atomic};
pub use task::{
    NoopSink, ProgressSink, ProgressUpdate, TaskCtx, TaskFinished, TaskId, TaskProgress, TaskState,
    ThrottledSink, VecSink,
};
