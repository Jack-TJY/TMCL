//! 一批下载的进度聚合：条目数、字节数与速度。
//!
//! 各下载任务并发地往同一个 [`ProgressTracker`] 里累加字节，
//! 调用方定期调用 [`ProgressTracker::report`] 把快照推给 [`TaskCtx`]。

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tmcl_common::{TaskCtx, TaskProgress};

/// 速度的平滑系数：新样本占的权重。
const SMOOTHING: f64 = 0.3;
/// 两次采样的最小间隔，太短的间隔算出的速度噪声很大。
const MIN_SAMPLE: Duration = Duration::from_millis(100);

/// 线程安全的进度累加器。
#[derive(Debug)]
pub struct ProgressTracker {
    stage: String,
    total: u64,
    bytes_total: u64,
    done: AtomicU64,
    bytes_done: AtomicU64,
    speed: Mutex<SpeedMeter>,
}

impl ProgressTracker {
    /// 新建一个阶段的进度，`total` 为条目数，`bytes_total` 为已知总字节数。
    pub fn new(stage: impl Into<String>, total: u64, bytes_total: u64) -> Self {
        Self {
            stage: stage.into(),
            total,
            bytes_total,
            done: AtomicU64::new(0),
            bytes_done: AtomicU64::new(0),
            speed: Mutex::new(SpeedMeter::new(Instant::now())),
        }
    }

    /// 累加已传输的字节。
    pub fn add_bytes(&self, n: u64) {
        self.bytes_done.fetch_add(n, Ordering::Relaxed);
    }

    /// 撤回已计入的字节（例如文件需要从头重下）。
    pub fn sub_bytes(&self, n: u64) {
        // 不会下溢：撤回的字节一定之前加过。
        self.bytes_done.fetch_sub(n, Ordering::Relaxed);
    }

    /// 完成一个条目。
    pub fn finish_item(&self) {
        self.done.fetch_add(1, Ordering::Relaxed);
    }

    /// 已完成的条目数与字节数。
    pub fn snapshot(&self) -> (u64, u64) {
        (
            self.done.load(Ordering::Relaxed),
            self.bytes_done.load(Ordering::Relaxed),
        )
    }

    /// 把当前进度（含平滑后的速度）上报给 `ctx`。
    pub fn report(&self, ctx: &TaskCtx) {
        let (done, bytes_done) = self.snapshot();
        let speed_bps = self
            .speed
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sample(Instant::now(), bytes_done);
        ctx.progress.report(&TaskProgress {
            task_id: ctx.id,
            kind: ctx.kind.clone(),
            stage: self.stage.clone(),
            done,
            total: self.total,
            bytes_done,
            bytes_total: self.bytes_total,
            speed_bps,
        });
    }
}

/// 指数平滑的速度计。
#[derive(Debug)]
struct SpeedMeter {
    last: (Instant, u64),
    speed: f64,
}

impl SpeedMeter {
    fn new(now: Instant) -> Self {
        Self {
            last: (now, 0),
            speed: 0.0,
        }
    }

    /// 记录 `now` 时的累计字节数，返回当前速度（字节/秒）。
    fn sample(&mut self, now: Instant, bytes: u64) -> u64 {
        let elapsed = now.saturating_duration_since(self.last.0);
        if elapsed >= MIN_SAMPLE {
            let instant = bytes.saturating_sub(self.last.1) as f64 / elapsed.as_secs_f64();
            self.speed = if self.speed == 0.0 {
                instant
            } else {
                SMOOTHING * instant + (1.0 - SMOOTHING) * self.speed
            };
            self.last = (now, bytes);
        }
        self.speed.round() as u64
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tmcl_common::VecSink;

    use super::*;

    #[test]
    fn speed_is_smoothed() {
        let start = Instant::now();
        let mut meter = SpeedMeter::new(start);
        assert_eq!(meter.sample(start + Duration::from_millis(10), 999), 0);
        assert_eq!(meter.sample(start + Duration::from_secs(1), 1000), 1000);
        // 下一秒 2000 B/s，平滑后 0.3 * 2000 + 0.7 * 1000。
        assert_eq!(meter.sample(start + Duration::from_secs(2), 3000), 1300);
    }

    #[test]
    fn reports_aggregated_counts() {
        let sink = Arc::new(VecSink::new());
        let ctx = TaskCtx::new("download", sink.clone());
        let tracker = ProgressTracker::new("assets", 3, 300);
        tracker.add_bytes(150);
        tracker.sub_bytes(50);
        tracker.finish_item();
        tracker.report(&ctx);
        let event = sink.events().pop().unwrap();
        assert_eq!(event.stage, "assets");
        assert_eq!((event.done, event.total), (1, 3));
        assert_eq!((event.bytes_done, event.bytes_total), (100, 300));
    }
}
