//! 事件名与任务事件桥接（PLAN.md 第 3.2、3.3 节，`docs/contracts/ipc.md`）。
//!
//! 长耗时命令调用 [`spawn_task`]：立即返回 [`TaskStarted`]，任务在后台运行，
//! 进度经 [`ThrottledSink`] 限流后以 `task:progress` 推送，结束时恰好推送一次 `task:finished`。
//! 任务同时登记到全局 [`TaskManager`]，供 `task_list` / `task_cancel` 使用。

use std::future::Future;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tmcl_common::{
    AppResult, ProgressSink, TaskCtx, TaskFinished, TaskId, TaskProgress, ThrottledSink,
};
use ts_rs::TS;

use crate::state::AppState;
use crate::tasks::TaskManager;

/// 任务进度，负载 [`TaskProgress`]，最多每 200 ms 一次。
pub const TASK_PROGRESS: &str = "task:progress";
/// 任务结束，负载 [`TaskFinished`]，每个任务恰好一次。
pub const TASK_FINISHED: &str = "task:finished";
/// 游戏输出，负载 [`GameLog`]，同一来源的连续行最多每 100 ms 批量推送一次。
pub const GAME_LOG: &str = "game:log";
/// 游戏进程退出，负载 [`GameExited`]，每次启动恰好一次。
pub const GAME_EXITED: &str = "game:exited";
// `auth:device-code`（M0-10）由对应步骤在此追加。

/// 游戏输出的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum GameLogStream {
    /// 标准输出。
    Stdout,
    /// 标准错误。
    Stderr,
}

/// `game:log` 的负载：一批同一来源的日志行，访问令牌已替换为 `***`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct GameLog {
    /// 实例 ID。
    pub instance_id: String,
    /// 游戏进程 ID。
    pub pid: u32,
    /// 来源。
    pub stream: GameLogStream,
    /// 日志行（不含换行符）。
    pub lines: Vec<String>,
}

/// `game:exited` 的负载。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct GameExited {
    /// 实例 ID。
    pub instance_id: String,
    /// 游戏进程 ID。
    pub pid: u32,
    /// 退出码；被强制结束或被信号结束时可能为 `null`。
    pub exit_code: Option<i32>,
    /// 是否崩溃：未被强制结束，且退出码非 0 或日志中出现崩溃特征。
    pub crashed: bool,
}

/// 长耗时命令的返回值：任务已在后台启动。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct TaskStarted {
    /// 任务 ID，与之后的 `task:progress` / `task:finished` 对应。
    pub task_id: TaskId,
}

/// 向所有窗口广播一个事件；失败只记日志（窗口可能已关闭）。
pub fn emit<R: Runtime, P: Serialize + Clone>(app: &AppHandle<R>, event: &str, payload: &P) {
    if let Err(err) = app.emit(event, payload.clone()) {
        tracing::warn!(event, error = %err, "failed to emit event");
    }
}

/// 把进度转成 `task:progress` 事件、并记入任务管理器的接收端。
pub struct EventSink<R: Runtime> {
    app: AppHandle<R>,
    tasks: Arc<TaskManager>,
}

impl<R: Runtime> EventSink<R> {
    /// 创建接收端。
    pub fn new(app: AppHandle<R>, tasks: Arc<TaskManager>) -> Self {
        Self { app, tasks }
    }
}

impl<R: Runtime> ProgressSink for EventSink<R> {
    fn report(&self, progress: &TaskProgress) {
        self.tasks.progress(progress);
        emit(&self.app, TASK_PROGRESS, progress);
    }
}

/// 在后台运行一个任务并立即返回其 ID。
///
/// `job` 收到的 [`TaskCtx`] 已接好限流后的事件接收端；返回 `Err(AppError::cancelled())`
/// 时结束状态为 `cancelled`。前端应在调用命令之前订阅任务事件，避免漏掉首条进度。
pub fn spawn_task<R, F, Fut, T>(app: &AppHandle<R>, kind: &str, job: F) -> TaskStarted
where
    R: Runtime,
    F: FnOnce(TaskCtx) -> Fut + Send + 'static,
    Fut: Future<Output = AppResult<T>> + Send + 'static,
    T: Send + 'static,
{
    let tasks = app
        .try_state::<AppState>()
        .map(|state| state.tasks.clone())
        .unwrap_or_default();
    let sink = Arc::new(ThrottledSink::new(EventSink::new(
        app.clone(),
        tasks.clone(),
    )));
    let ctx = TaskCtx::new(kind, sink);
    let task_id = ctx.id;
    tasks.start(&ctx);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = job(ctx).await;
        if let Err(err) = &result
            && !err.is_cancelled()
        {
            tracing::warn!(%task_id, error = %err, "task failed");
        }
        let finished = TaskFinished::from_result(task_id, &result);
        tasks.finish(&finished);
        emit(&app, TASK_FINISHED, &finished);
    });
    TaskStarted { task_id }
}
