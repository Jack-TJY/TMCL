//! `app_*` 命令：应用信息、本项目的发布版本与演示任务。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};
use ts_rs::TS;

use crate::events::{TaskStarted, spawn_task};
use crate::state::AppState;

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

/// 本项目的一个发布版本（GitHub Release）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ReleaseNote {
    /// 标签，例如 `v0.1.0-alpha.1`。
    pub tag: String,
    /// 标题；发布时没填写则为空字符串。
    pub name: String,
    /// 发布时间（RFC 3339）。
    pub published_at: String,
    /// 是否为预发布。
    pub prerelease: bool,
    /// 发布页地址。
    pub url: String,
}

/// `app_releases` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ReleaseList {
    /// 最近的发布版本，新的在前，最多 [`RELEASES_LIMIT`] 个。
    pub releases: Vec<ReleaseNote>,
}

/// 本项目 GitHub Releases 的 API 地址（`docs/contracts/endpoints.md`）。
pub const RELEASES_URL: &str = "https://api.github.com/repos/Jack-TJY/TMCL/releases?per_page=20";
/// `app_releases` 最多返回的版本数。
pub const RELEASES_LIMIT: usize = 10;
/// 缓存在这个时长内直接使用。GitHub 未认证接口限每小时 60 次，主页每次打开都会读取。
const RELEASES_MAX_AGE: Duration = Duration::from_secs(60 * 60);
const RELEASES_CACHE_KEY: &str = "app.releases";

/// 演示任务的种类名，写入 `TaskProgress::kind`。
pub const DEMO_TASK_KIND: &str = "app_demo";

/// 返回版本号、平台与功能开关。
#[tauri::command]
pub async fn app_info() -> Result<AppInfo, AppError> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        ms_login_enabled: tmcl_auth::ms_login_enabled(),
    })
}

/// 本项目最近的发布版本，供主页“更新日志”组件使用。
///
/// 1 小时内使用 SQLite 中的缓存；获取失败时退回旧缓存，没有缓存则返回获取时的错误
/// （网络错误为 `NET_*`，响应不是预期的 JSON 为 `COMMON_JSON`）。草稿不会出现在结果中。
#[tauri::command]
pub async fn app_releases(state: State<'_, AppState>) -> Result<ReleaseList, AppError> {
    load_releases(&state).await
}

/// 写入 kv 表的缓存内容。
#[derive(Debug, Serialize, Deserialize)]
struct CachedReleases {
    /// 获取时间（unix 秒）。
    fetched_at: u64,
    releases: Vec<ReleaseNote>,
}

/// GitHub `GET /repos/{owner}/{repo}/releases` 响应中用到的字段。
#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    html_url: String,
}

async fn load_releases(state: &AppState) -> AppResult<ReleaseList> {
    let now = unix_now();
    let cached = state
        .with_db(|_, db| db.kv_get(RELEASES_CACHE_KEY))
        .inspect_err(|e| tracing::warn!(error = %e, "读取发布版本缓存失败"))
        .ok()
        .flatten()
        .and_then(|body| serde_json::from_str::<CachedReleases>(&body).ok());
    if let Some(cached) = &cached
        && now.saturating_sub(cached.fetched_at) < RELEASES_MAX_AGE.as_secs()
    {
        return Ok(ReleaseList {
            releases: cached.releases.clone(),
        });
    }
    match fetch_releases(state).await {
        Ok(releases) => {
            let body = serde_json::to_string(&CachedReleases {
                fetched_at: now,
                releases: releases.clone(),
            })?;
            if let Err(e) = state.with_db(|_, db| db.kv_set(RELEASES_CACHE_KEY, &body)) {
                tracing::warn!(error = %e, "写入发布版本缓存失败");
            }
            Ok(ReleaseList { releases })
        }
        Err(e) => match cached {
            Some(cached) => {
                tracing::warn!(error = %e, "获取发布版本失败，使用旧缓存");
                Ok(ReleaseList {
                    releases: cached.releases,
                })
            }
            None => Err(e),
        },
    }
}

async fn fetch_releases(state: &AppState) -> AppResult<Vec<ReleaseNote>> {
    let ctx = TaskCtx::noop("app_releases");
    let bytes = state
        .downloader()?
        .get_bytes(&state.releases_url, &ctx)
        .await?;
    parse_releases(&bytes).map_err(|e| e.with_param("url", &state.releases_url))
}

/// 解析 GitHub Releases 响应：去掉草稿与没有发布时间的条目，按发布时间从新到旧取前
/// [`RELEASES_LIMIT`] 个。
fn parse_releases(bytes: &[u8]) -> AppResult<Vec<ReleaseNote>> {
    let raw: Vec<GithubRelease> = serde_json::from_slice(bytes)?;
    let mut releases: Vec<ReleaseNote> = raw
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            Some(ReleaseNote {
                published_at: r.published_at?,
                name: r.name.unwrap_or_default(),
                tag: r.tag_name,
                prerelease: r.prerelease,
                url: r.html_url,
            })
        })
        .collect();
    // GitHub 的发布时间都是 `Z` 结尾的 UTC 时间，按字符串比较即按时间比较。
    releases.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    releases.truncate(RELEASES_LIMIT);
    Ok(releases)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
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

    use tmcl_common::{DataPaths, VecSink};
    use tmcl_net::testing::{Fault, TestServer};

    use super::*;

    const RELEASES: &str = r#"[
        {"tag_name": "v0.1.0-alpha.1", "name": "Alpha 1", "draft": false, "prerelease": true,
         "published_at": "2026-11-01T08:00:00Z",
         "html_url": "https://github.com/Jack-TJY/TMCL/releases/tag/v0.1.0-alpha.1"},
        {"tag_name": "v0.2.0", "name": null, "draft": false, "prerelease": false,
         "published_at": "2026-12-01T08:00:00Z",
         "html_url": "https://github.com/Jack-TJY/TMCL/releases/tag/v0.2.0"},
        {"tag_name": "v0.3.0", "name": "draft", "draft": true, "prerelease": false,
         "published_at": null,
         "html_url": "https://github.com/Jack-TJY/TMCL/releases/tag/untagged-1"}
    ]"#;

    fn start_server() -> TestServer {
        tauri::async_runtime::block_on(TestServer::start([(
            "/releases",
            RELEASES.as_bytes().to_vec(),
        )]))
    }

    /// 指向本地服务器的状态；返回的 `TempDir` 需要活到测试结束。
    fn releases_state(server: &TestServer) -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let mut state = AppState::with_data(Ok(DataPaths::at(dir.path())));
        state.releases_url = server.url("/releases");
        (state, dir)
    }

    #[test]
    fn parses_and_sorts_releases() {
        let releases = parse_releases(RELEASES.as_bytes()).unwrap();
        let tags: Vec<&str> = releases.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["v0.2.0", "v0.1.0-alpha.1"]);
        assert_eq!(releases[0].name, "");
        assert!(releases[1].prerelease);
        assert_eq!(
            releases[1].url,
            "https://github.com/Jack-TJY/TMCL/releases/tag/v0.1.0-alpha.1"
        );
    }

    #[test]
    fn keeps_at_most_limit_releases() {
        let many: Vec<String> = (1..=15)
            .map(|i| {
                format!(
                    r#"{{"tag_name": "v0.{i}.0", "published_at": "2026-01-{i:02}T00:00:00Z", "html_url": "u"}}"#
                )
            })
            .collect();
        let body = format!("[{}]", many.join(","));
        let releases = parse_releases(body.as_bytes()).unwrap();
        assert_eq!(releases.len(), RELEASES_LIMIT);
        assert_eq!(releases[0].tag, "v0.15.0");
    }

    #[test]
    fn rejects_unexpected_json() {
        let err = parse_releases(br#"{"message": "Not Found"}"#).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonJson);
    }

    #[test]
    fn releases_are_cached() {
        let server = start_server();
        let (state, _dir) = releases_state(&server);
        let first = tauri::async_runtime::block_on(load_releases(&state)).unwrap();
        assert_eq!(first.releases.len(), 2);
        let again = tauri::async_runtime::block_on(load_releases(&state)).unwrap();
        assert_eq!(again, first);
        assert_eq!(server.hits(), 1);
    }

    #[test]
    fn stale_cache_is_used_when_fetch_fails() {
        let server = start_server();
        let (state, _dir) = releases_state(&server);
        let stale = CachedReleases {
            fetched_at: 0,
            releases: vec![ReleaseNote {
                tag: "v0.0.1".into(),
                name: "old".into(),
                published_at: "2026-01-01T00:00:00Z".into(),
                prerelease: false,
                url: "u".into(),
            }],
        };
        let body = serde_json::to_string(&stale).unwrap();
        state
            .with_db(|_, db| db.kv_set(RELEASES_CACHE_KEY, &body))
            .unwrap();
        server.fail("/releases", [Fault::Status(403)]);
        let list = tauri::async_runtime::block_on(load_releases(&state)).unwrap();
        assert_eq!(list.releases, stale.releases);
    }

    #[test]
    fn fetch_error_without_cache_is_returned() {
        let server = start_server();
        let (state, _dir) = releases_state(&server);
        server.fail("/releases", [Fault::Status(403)]);
        let err = tauri::async_runtime::block_on(load_releases(&state)).unwrap_err();
        assert_eq!(err.code, ErrorCode::NetHttpStatus);
        assert_eq!(err.params["status"], "403");
    }

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
        assert_eq!(info.ms_login_enabled, tmcl_auth::ms_login_enabled());
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
