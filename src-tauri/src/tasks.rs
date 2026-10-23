//! 全局任务管理器：记录所有经 [`spawn_task`](crate::events::spawn_task) 启动的任务，
//! 支持列表、取消，并保留最近 [`HISTORY_LIMIT`] 条已结束任务。

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tmcl_common::{AppError, TaskCtx, TaskFinished, TaskId, TaskProgress, TaskState};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

/// 保留的已结束任务数。
pub const HISTORY_LIMIT: usize = 50;

/// 任务当前状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// 运行中。
    Running,
    /// 成功完成。
    Succeeded,
    /// 失败。
    Failed,
    /// 被取消。
    Cancelled,
}

impl From<TaskState> for TaskStatus {
    fn from(state: TaskState) -> Self {
        match state {
            TaskState::Succeeded => Self::Succeeded,
            TaskState::Failed => Self::Failed,
            TaskState::Cancelled => Self::Cancelled,
        }
    }
}

/// 任务列表中的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct TaskInfo {
    /// 任务 ID。
    pub task_id: TaskId,
    /// 任务种类，例如 `game_install`。
    pub kind: String,
    /// 当前状态。
    pub status: TaskStatus,
    /// 开始时间（unix 毫秒）。
    #[ts(type = "number")]
    pub started_at: u64,
    /// 结束时间（unix 毫秒），运行中为 `null`。
    #[ts(type = "number | null")]
    pub finished_at: Option<u64>,
    /// 最近一次推送的进度，尚未上报过时为 `null`。
    pub progress: Option<TaskProgress>,
    /// 失败原因，仅 `failed` 时存在。
    pub error: Option<AppError>,
}

struct Running {
    info: TaskInfo,
    cancel: CancellationToken,
}

#[derive(Default)]
struct Inner {
    running: HashMap<TaskId, Running>,
    /// 已结束的任务，最新的在前。
    history: VecDeque<TaskInfo>,
}

/// 全局任务管理器，放在 [`AppState`](crate::state::AppState) 中。
#[derive(Default)]
pub struct TaskManager {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for TaskManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskManager")
            .field("running", &self.running_count())
            .finish_non_exhaustive()
    }
}

impl TaskManager {
    /// 创建空的管理器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一个刚启动的任务。
    pub fn start(&self, ctx: &TaskCtx) {
        let info = TaskInfo {
            task_id: ctx.id,
            kind: ctx.kind.clone(),
            status: TaskStatus::Running,
            started_at: now_millis(),
            finished_at: None,
            progress: None,
            error: None,
        };
        let cancel = ctx.cancel.clone();
        self.lock().running.insert(ctx.id, Running { info, cancel });
    }

    /// 记录最近一次进度。
    pub fn progress(&self, progress: &TaskProgress) {
        if let Some(task) = self.lock().running.get_mut(&progress.task_id) {
            task.info.progress = Some(progress.clone());
        }
    }

    /// 任务结束：移入历史，超出 [`HISTORY_LIMIT`] 的最旧记录被丢弃。
    pub fn finish(&self, finished: &TaskFinished) {
        let mut inner = self.lock();
        let Some(task) = inner.running.remove(&finished.task_id) else {
            return;
        };
        let mut info = task.info;
        info.status = finished.state.into();
        info.finished_at = Some(now_millis());
        info.error = finished.error.clone();
        inner.history.push_front(info);
        inner.history.truncate(HISTORY_LIMIT);
    }

    /// 请求取消运行中的任务；任务不存在或已结束时返回 `false`。
    ///
    /// 任务在下一个检查点退出，届时推送 `task:finished`（`cancelled`）。
    pub fn cancel(&self, task_id: TaskId) -> bool {
        match self.lock().running.get(&task_id) {
            Some(task) => {
                task.cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// 所有任务：运行中的按开始时间先后在前，随后是历史（最近结束的在前）。
    pub fn list(&self) -> Vec<TaskInfo> {
        let inner = self.lock();
        let mut running: Vec<TaskInfo> = inner.running.values().map(|t| t.info.clone()).collect();
        running.sort_by_key(|t| (t.started_at, t.task_id));
        running.extend(inner.history.iter().cloned());
        running
    }

    /// 运行中的任务数。
    pub fn running_count(&self) -> usize {
        self.lock().running.len()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use tmcl_common::ErrorCode;

    use super::*;

    fn finished(ctx: &TaskCtx, result: Result<(), AppError>) -> TaskFinished {
        TaskFinished::from_result(ctx.id, &result)
    }

    #[test]
    fn tracks_lifecycle_and_progress() {
        let manager = TaskManager::new();
        let ctx = TaskCtx::noop("demo");
        manager.start(&ctx);
        assert_eq!(manager.running_count(), 1);

        let progress = TaskProgress {
            task_id: ctx.id,
            kind: "demo".into(),
            stage: "x".into(),
            done: 1,
            total: 2,
            bytes_done: 0,
            bytes_total: 0,
            speed_bps: 0,
        };
        manager.progress(&progress);
        assert_eq!(manager.list()[0].progress.as_ref(), Some(&progress));

        let err = AppError::new(ErrorCode::CommonIo);
        manager.finish(&finished(&ctx, Err(err)));
        let list = manager.list();
        assert_eq!(manager.running_count(), 0);
        assert_eq!(list[0].status, TaskStatus::Failed);
        assert_eq!(list[0].error.as_ref().unwrap().code, ErrorCode::CommonIo);
        assert!(list[0].finished_at.is_some());
    }

    #[test]
    fn cancel_triggers_token_only_while_running() {
        let manager = TaskManager::new();
        let ctx = TaskCtx::noop("demo");
        manager.start(&ctx);
        assert!(manager.cancel(ctx.id));
        assert!(ctx.is_cancelled());

        manager.finish(&finished(&ctx, Err(AppError::cancelled())));
        assert_eq!(manager.list()[0].status, TaskStatus::Cancelled);
        assert!(!manager.cancel(ctx.id));
        assert!(!manager.cancel(TaskId::new()));
    }

    #[test]
    fn running_first_and_history_is_bounded() {
        let manager = TaskManager::new();
        let mut last = None;
        for _ in 0..HISTORY_LIMIT + 5 {
            let ctx = TaskCtx::noop("done");
            manager.start(&ctx);
            manager.finish(&finished(&ctx, Ok(())));
            last = Some(ctx.id);
        }
        let running = TaskCtx::noop("running");
        manager.start(&running);

        let list = manager.list();
        assert_eq!(list.len(), HISTORY_LIMIT + 1);
        assert_eq!(list[0].task_id, running.id);
        assert_eq!(list[1].task_id, last.unwrap());
        assert!(list[1..].iter().all(|t| t.status == TaskStatus::Succeeded));
    }
}
