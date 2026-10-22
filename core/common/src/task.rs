//! 长耗时任务的上下文、取消与进度（PLAN.md 第 3.3、3.4 节）。
//!
//! 核心层与模块层的长耗时函数都接收 `&TaskCtx`，在合适的节点调用
//! [`TaskCtx::check_cancelled`]，并通过 [`TaskCtx::report`] 上报进度。
//! `src-tauri` 实现 [`ProgressSink`] 把进度转成 `task:progress` 事件，
//! 并用 [`ThrottledSink`] 限制到每 200 ms 最多一次。

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// `task:progress` 事件的最小间隔。
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// 任务 ID（UUID v4），序列化为字符串（newtype 默认即按内部值序列化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(type = "string")]
pub struct TaskId(Uuid);

impl TaskId {
    /// 生成一个新的随机 ID。
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// `task:progress` 事件负载。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TaskProgress {
    /// 任务 ID。
    pub task_id: TaskId,
    /// 任务种类，例如 `game_install`。
    pub kind: String,
    /// 当前阶段，例如 `libraries`；前端以 `task.stage.<stage>` 作为 i18n key。
    pub stage: String,
    /// 已完成的条目数。
    #[ts(type = "number")]
    pub done: u64,
    /// 条目总数，未知时为 0。
    #[ts(type = "number")]
    pub total: u64,
    /// 已传输字节数。
    #[ts(type = "number")]
    pub bytes_done: u64,
    /// 总字节数，未知时为 0。
    #[ts(type = "number")]
    pub bytes_total: u64,
    /// 当前速度（字节/秒），由 [`ThrottledSink`] 计算。
    #[ts(type = "number")]
    pub speed_bps: u64,
}

/// 任务的最终状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// 成功完成。
    Succeeded,
    /// 失败，`TaskFinished::error` 带错误。
    Failed,
    /// 被取消。
    Cancelled,
}

/// `task:finished` 事件负载，每个任务恰好一次。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TaskFinished {
    /// 任务 ID。
    pub task_id: TaskId,
    /// 最终状态。
    pub state: TaskState,
    /// 失败原因，仅 `failed` 时存在。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<AppError>,
}

impl TaskFinished {
    /// 由任务函数的返回值得出最终状态；取消错误映射为 `cancelled`。
    pub fn from_result<T>(task_id: TaskId, result: &AppResult<T>) -> Self {
        let (state, error) = match result {
            Ok(_) => (TaskState::Succeeded, None),
            Err(e) if e.is_cancelled() => (TaskState::Cancelled, None),
            Err(e) => (TaskState::Failed, Some(e.clone())),
        };
        Self {
            task_id,
            state,
            error,
        }
    }
}

/// 进度接收端。实现必须廉价且不阻塞。
pub trait ProgressSink: Send + Sync {
    /// 接收一次进度。
    fn report(&self, progress: &TaskProgress);
}

/// 丢弃所有进度。
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSink;

impl ProgressSink for NoopSink {
    fn report(&self, _progress: &TaskProgress) {}
}

/// 记录所有进度，供测试断言。
#[derive(Debug, Default)]
pub struct VecSink {
    events: Mutex<Vec<TaskProgress>>,
}

impl VecSink {
    /// 创建空的记录器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回目前收到的所有进度的副本。
    pub fn events(&self) -> Vec<TaskProgress> {
        lock(&self.events).clone()
    }
}

impl ProgressSink for VecSink {
    fn report(&self, progress: &TaskProgress) {
        lock(&self.events).push(progress.clone());
    }
}

/// 限流并计算速度的包装：同一阶段内每个间隔最多转发一次，
/// 但阶段切换、首条和完成（`done == total` 或 `bytes_done == bytes_total`）总会转发。
pub struct ThrottledSink<S> {
    inner: S,
    interval: Duration,
    state: Mutex<ThrottleState>,
}

#[derive(Default)]
struct ThrottleState {
    last: Option<(Instant, String, u64)>,
}

impl<S: ProgressSink> ThrottledSink<S> {
    /// 以 [`PROGRESS_INTERVAL`] 为间隔包装 `inner`。
    pub fn new(inner: S) -> Self {
        Self::with_interval(inner, PROGRESS_INTERVAL)
    }

    /// 以自定义间隔包装 `inner`。
    pub fn with_interval(inner: S, interval: Duration) -> Self {
        Self {
            inner,
            interval,
            state: Mutex::new(ThrottleState::default()),
        }
    }

    /// 被包装的接收端。
    pub fn inner(&self) -> &S {
        &self.inner
    }
}

impl<S: ProgressSink> ProgressSink for ThrottledSink<S> {
    fn report(&self, progress: &TaskProgress) {
        let now = Instant::now();
        let finished = (progress.total > 0 && progress.done >= progress.total)
            || (progress.bytes_total > 0 && progress.bytes_done >= progress.bytes_total);
        let mut state = lock(&self.state);
        let mut out = progress.clone();
        match &state.last {
            Some((at, stage, bytes)) if *stage == progress.stage => {
                let elapsed = now.duration_since(*at);
                if elapsed < self.interval && !finished {
                    return;
                }
                if out.speed_bps == 0 && !elapsed.is_zero() {
                    let delta = progress.bytes_done.saturating_sub(*bytes);
                    out.speed_bps = (delta as f64 / elapsed.as_secs_f64()) as u64;
                }
            }
            _ => {}
        }
        state.last = Some((now, progress.stage.clone(), progress.bytes_done));
        drop(state);
        self.inner.report(&out);
    }
}

impl<S: ProgressSink + ?Sized> ProgressSink for Arc<S> {
    fn report(&self, progress: &TaskProgress) {
        (**self).report(progress);
    }
}

/// 一次进度更新，由 [`TaskCtx::report`] 补上任务 ID 与种类。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgressUpdate {
    stage: String,
    done: u64,
    total: u64,
    bytes_done: u64,
    bytes_total: u64,
}

impl ProgressUpdate {
    /// 指定阶段名的更新。
    pub fn new(stage: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            ..Self::default()
        }
    }

    /// 设置条目进度。
    pub fn items(mut self, done: u64, total: u64) -> Self {
        self.done = done;
        self.total = total;
        self
    }

    /// 设置字节进度。
    pub fn bytes(mut self, done: u64, total: u64) -> Self {
        self.bytes_done = done;
        self.bytes_total = total;
        self
    }
}

/// 长耗时任务的上下文。
#[derive(Clone)]
pub struct TaskCtx {
    /// 任务 ID。
    pub id: TaskId,
    /// 任务种类，写入每条 [`TaskProgress::kind`]。
    pub kind: String,
    /// 取消令牌；子操作可用 `cancel.child_token()` 派生。
    pub cancel: CancellationToken,
    /// 进度接收端，测试中可用 [`NoopSink`] / [`VecSink`]。
    pub progress: Arc<dyn ProgressSink>,
}

impl TaskCtx {
    /// 创建一个新任务上下文（新 ID、新取消令牌）。
    pub fn new(kind: impl Into<String>, progress: Arc<dyn ProgressSink>) -> Self {
        Self {
            id: TaskId::new(),
            kind: kind.into(),
            cancel: CancellationToken::new(),
            progress,
        }
    }

    /// 不上报进度的上下文，用于测试或不需要进度的调用方。
    pub fn noop(kind: impl Into<String>) -> Self {
        Self::new(kind, Arc::new(NoopSink))
    }

    /// 是否已被取消。
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// 已取消时返回 `COMMON_CANCELLED`，供 `?` 提前退出。
    pub fn check_cancelled(&self) -> AppResult<()> {
        if self.is_cancelled() {
            Err(AppError::cancelled())
        } else {
            Ok(())
        }
    }

    /// 上报一次进度。
    pub fn report(&self, update: ProgressUpdate) {
        self.progress.report(&TaskProgress {
            task_id: self.id,
            kind: self.kind.clone(),
            stage: update.stage,
            done: update.done,
            total: update.total,
            bytes_done: update.bytes_done,
            bytes_total: update.bytes_total,
            speed_bps: 0,
        });
    }
}

impl fmt::Debug for TaskCtx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaskCtx")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    #[test]
    fn report_fills_id_and_kind() {
        let sink = Arc::new(VecSink::new());
        let ctx = TaskCtx::new("game_install", sink.clone());
        ctx.report(ProgressUpdate::new("libraries").items(1, 3).bytes(10, 30));
        let events = sink.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].task_id, ctx.id);
        assert_eq!(events[0].kind, "game_install");
        assert_eq!(events[0].stage, "libraries");
        assert_eq!((events[0].done, events[0].total), (1, 3));
        assert_eq!((events[0].bytes_done, events[0].bytes_total), (10, 30));
    }

    #[test]
    fn cancellation_is_observed() {
        let ctx = TaskCtx::noop("demo");
        assert!(ctx.check_cancelled().is_ok());
        let child = ctx.cancel.child_token();
        ctx.cancel.cancel();
        assert!(child.is_cancelled());
        let err = ctx.check_cancelled().unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonCancelled);
    }

    #[test]
    fn finished_state_from_result() {
        let id = TaskId::new();
        let ok: AppResult<()> = Ok(());
        assert_eq!(
            TaskFinished::from_result(id, &ok).state,
            TaskState::Succeeded
        );

        let cancelled: AppResult<()> = Err(AppError::cancelled());
        let finished = TaskFinished::from_result(id, &cancelled);
        assert_eq!(finished.state, TaskState::Cancelled);
        assert!(finished.error.is_none());

        let failed: AppResult<()> = Err(AppError::new(ErrorCode::CommonIo));
        let finished = TaskFinished::from_result(id, &failed);
        assert_eq!(finished.state, TaskState::Failed);
        assert_eq!(finished.error.unwrap().code, ErrorCode::CommonIo);
    }

    #[test]
    fn finished_serialization_omits_missing_error() {
        let finished = TaskFinished {
            task_id: TaskId::new(),
            state: TaskState::Succeeded,
            error: None,
        };
        let value = serde_json::to_value(&finished).unwrap();
        assert_eq!(value["state"], "succeeded");
        assert!(value.get("error").is_none());
        assert!(value["task_id"].is_string());
    }

    #[test]
    fn throttle_drops_bursts_but_keeps_stage_changes_and_completion() {
        let sink = Arc::new(ThrottledSink::with_interval(
            VecSink::new(),
            Duration::from_secs(3600),
        ));
        let ctx = TaskCtx::new("download", sink.clone());
        for i in 0..100 {
            ctx.report(ProgressUpdate::new("assets").items(i, 100));
        }
        ctx.report(ProgressUpdate::new("assets").items(100, 100));
        ctx.report(ProgressUpdate::new("libraries").items(0, 5));
        let stages: Vec<_> = sink
            .inner()
            .events()
            .into_iter()
            .map(|p| (p.stage, p.done))
            .collect();
        assert_eq!(
            stages,
            vec![
                ("assets".to_owned(), 0),
                ("assets".to_owned(), 100),
                ("libraries".to_owned(), 0),
            ]
        );
    }

    #[test]
    fn throttle_computes_speed() {
        let sink = ThrottledSink::with_interval(VecSink::new(), Duration::ZERO);
        let ctx = TaskCtx::new("download", Arc::new(NoopSink));
        let make = |bytes| TaskProgress {
            task_id: ctx.id,
            kind: "download".into(),
            stage: "client".into(),
            done: 0,
            total: 1,
            bytes_done: bytes,
            bytes_total: 10_000_000,
            speed_bps: 0,
        };
        sink.report(&make(0));
        std::thread::sleep(Duration::from_millis(20));
        sink.report(&make(1_000_000));
        let events = sink.inner().events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].speed_bps, 0);
        assert!(events[1].speed_bps > 0);
    }
}
