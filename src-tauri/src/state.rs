//! 应用全局状态：在 `lib.rs` 中用 `manage` 注册，命令通过 `State<'_, AppState>` 取用。

use tmcl_common::{AppResult, DataPaths};
use tokio::sync::Mutex;

/// 命令层共享的状态。
///
/// M1-10 会在这里加入全局 `TaskManager`（任务列表、取消、历史），
/// 后续步骤按需加入数据库连接等。
#[derive(Debug)]
pub struct AppState {
    /// 数据目录；解析失败时保留错误，由用到它的命令返回。
    data: AppResult<DataPaths>,
    /// 串行化 `config.json` 的“读取—修改—写回”，避免并发命令互相覆盖。
    pub config_lock: Mutex<()>,
}

impl AppState {
    /// 按启动器安装目录解析数据目录。
    pub fn new() -> Self {
        Self::with_data(DataPaths::resolve())
    }

    /// 使用给定的数据目录（测试用）。
    pub fn with_data(data: AppResult<DataPaths>) -> Self {
        Self {
            data,
            config_lock: Mutex::new(()),
        }
    }

    /// 数据目录；无法确定时返回 `COMMON_DATA_DIR_UNAVAILABLE`。
    pub fn data(&self) -> AppResult<&DataPaths> {
        self.data.as_ref().map_err(Clone::clone)
    }
}
