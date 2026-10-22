//! `app_*` 命令：应用信息与演示任务。

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};
use ts_rs::TS;

use crate::events::{TaskStarted, spawn_task};

/// `app_info` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct AppInfo {
    /// 启动器版本号，例如 `0.1.0`。
    pub version: String,
    /// 操作系统：`windows` | `macos` | `linux`。
    pub os: String,
    /// CPU 架构，例如 `x86_64`、`aarch64`。
    pub arch: String,
    /// 构建时是否注入了微软登录所需的 Client ID。
    pub ms_login_enabled: bool,
}

/// `app_demo_task` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct AppDemoTaskArgs {
    /// 步数，1–100。
    pub steps: u32,
    /// 每步耗时（毫秒），不超过 5000。
    pub step_ms: u32,
    /// 走到第几步时故意失败，用于演示 `task:finished` 的错误。
    #[serde(default)]
    #[ts(optional)]
    pub fail_at: Option<u32>,
}

/// 演示任务的种类名，写入 `TaskProgress::kind`。
pub const DEMO_TASK_KIND: &str = "app_demo";

/// 返回版本号、平台与功能开关。
#[tauri::command]
pub async fn app_info() -> Result<AppInfo, AppError> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        // M0-08 接入构建期密钥注入后改为读取 tmcl-auth 的开关。
        ms_login_enabled: false,
    })
}

/// 启动一个模拟进度的演示任务，立即返回任务 ID。
///
/// 参数越界时直接返回 `COMMON_INTERNAL`，用于演示命令失败时的结构化错误。
#[tauri::command]
pub async fn app_demo_task<R: Runtime>(
    app: AppHandle<R>,
    args: AppDemoTaskArgs,
) -> Result<TaskStarted, AppError> {
    validate_demo_args(&args)?;
    Ok(spawn_task(&app, DEMO_TASK_KIND, move |ctx| {
        run_demo(ctx, args)
    }))
}

fn validate_demo_args(args: &AppDemoTaskArgs) -> AppResult<()> {
    if !(1..=100).contains(&args.steps) || args.step_ms > 5000 {
        return Err(AppError::new(ErrorCode::CommonInternal)
            .with_param("steps", args.steps)
            .with_param("step_ms", args.step_ms)
            .with_detail("app_demo_task: steps must be 1..=100 and step_ms <= 5000"));
    }
    Ok(())
}

async fn run_demo(ctx: TaskCtx, args: AppDemoTaskArgs) -> AppResult<()> {
    let total = u64::from(args.steps);
    for step in 0..=args.steps {
        ctx.check_cancelled()?;
        if args.fail_at == Some(step) {
            return Err(AppError::new(ErrorCode::CommonInternal)
                .with_param("step", step)
                .with_detail("app_demo_task failed on purpose"));
        }
        ctx.report(ProgressUpdate::new("demo").items(u64::from(step), total));
        if step < args.steps {
            tokio::time::sleep(Duration::from_millis(u64::from(args.step_ms))).await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tmcl_common::VecSink;

    use super::*;

    fn args(steps: u32, fail_at: Option<u32>) -> AppDemoTaskArgs {
        AppDemoTaskArgs {
            steps,
            step_ms: 0,
            fail_at,
        }
    }

    #[test]
    fn info_reports_platform() {
        let info = tauri::async_runtime::block_on(app_info()).unwrap();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.os, std::env::consts::OS);
        assert!(!info.ms_login_enabled);
    }

    #[test]
    fn rejects_out_of_range_args() {
        let err = validate_demo_args(&args(0, None)).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonInternal);
        assert_eq!(err.params["steps"], "0");
        assert!(validate_demo_args(&args(101, None)).is_err());
        assert!(validate_demo_args(&args(3, None)).is_ok());
    }

    #[test]
    fn demo_reports_every_step() {
        let sink = Arc::new(VecSink::new());
        let ctx = TaskCtx::new(DEMO_TASK_KIND, sink.clone());
        tauri::async_runtime::block_on(run_demo(ctx, args(3, None))).unwrap();
        let done: Vec<u64> = sink.events().iter().map(|p| p.done).collect();
        assert_eq!(done, [0, 1, 2, 3]);
        assert!(
            sink.events()
                .iter()
                .all(|p| p.total == 3 && p.kind == DEMO_TASK_KIND)
        );
    }

    #[test]
    fn demo_fails_at_requested_step() {
        let sink = Arc::new(VecSink::new());
        let ctx = TaskCtx::new(DEMO_TASK_KIND, sink.clone());
        let err = tauri::async_runtime::block_on(run_demo(ctx, args(3, Some(2)))).unwrap_err();
        assert_eq!(err.params["step"], "2");
        assert_eq!(sink.events().len(), 2);
    }

    #[test]
    fn demo_stops_when_cancelled() {
        let ctx = TaskCtx::noop(DEMO_TASK_KIND);
        ctx.cancel.cancel();
        let err = tauri::async_runtime::block_on(run_demo(ctx, args(3, None))).unwrap_err();
        assert!(err.is_cancelled());
    }
}
