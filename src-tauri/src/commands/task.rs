//! `task_*` 命令：全局任务列表与取消。

use serde::{Deserialize, Serialize};
use tauri::State;
use tmcl_common::{AppError, TaskId};
use ts_rs::TS;

use crate::state::AppState;
use crate::tasks::TaskInfo;

/// `task_list` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct TaskList {
    /// 运行中的任务（按开始时间先后）在前，随后是最近结束的至多 50 条。
    pub tasks: Vec<TaskInfo>,
}

/// `task_cancel` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct TaskCancelArgs {
    /// 要取消的任务。
    pub task_id: TaskId,
}

/// `task_cancel` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct TaskCancelResult {
    /// 是否向运行中的任务发出了取消请求；任务不存在或已结束时为 `false`。
    pub cancelled: bool,
}

/// 列出运行中的任务与最近的历史。
#[tauri::command]
pub async fn task_list(state: State<'_, AppState>) -> Result<TaskList, AppError> {
    Ok(TaskList {
        tasks: state.tasks.list(),
    })
}

/// 请求取消任务；任务随后以 `task:finished`（`cancelled`）结束。
#[tauri::command]
pub async fn task_cancel(
    state: State<'_, AppState>,
    args: TaskCancelArgs,
) -> Result<TaskCancelResult, AppError> {
    Ok(TaskCancelResult {
        cancelled: state.tasks.cancel(args.task_id),
    })
}
