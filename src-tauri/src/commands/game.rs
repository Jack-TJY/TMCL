//! `game_*` 命令：版本清单与游戏文件下载。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_game::{
    ManifestPolicy, ManifestStore, VersionManifest, VersionType, install_version,
    load_manifest_checked,
};
use ts_rs::TS;

use crate::events::{TaskStarted, spawn_task};
use crate::state::AppState;

/// 游戏文件下载任务的种类名。
pub const GAME_INSTALL_KIND: &str = "game_install";

/// 版本清单中的一个版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct GameVersion {
    /// 版本号，例如 `1.20.1`。
    pub id: String,
    /// `release` | `snapshot` | `old_beta` | `old_alpha` | `other`。
    pub kind: String,
    /// 发布时间（RFC 3339）。
    pub release_time: String,
}

/// `game_version_list` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct GameVersionList {
    /// 最新正式版。
    pub latest_release: String,
    /// 最新快照。
    pub latest_snapshot: String,
    /// 所有版本，新版本在前。
    pub versions: Vec<GameVersion>,
    /// 联网获取失败、返回的是旧缓存时为获取时的错误，界面据此提示；否则为 `null`。
    pub stale: Option<AppError>,
}

/// `game_version_list` 的参数；省略时等同 `refresh: false`。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, TS)]
pub struct GameVersionListArgs {
    /// 忽略 1 小时内的缓存，强制联网获取（“刷新”按钮）。
    pub refresh: bool,
}

/// `game_install` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct GameInstallArgs {
    /// 要下载的版本号。
    pub version_id: String,
}

/// 获取官方版本清单；1 小时内使用 SQLite 中的缓存（`refresh` 为真时跳过），获取失败时退回旧缓存并在
/// `stale` 中给出原因。
#[tauri::command]
pub async fn game_version_list(
    state: State<'_, AppState>,
    args: Option<GameVersionListArgs>,
) -> Result<GameVersionList, AppError> {
    let downloader = state.downloader()?;
    let ctx = TaskCtx::noop("game_version_list");
    let policy = ManifestPolicy {
        refresh: args.unwrap_or_default().refresh,
        ..ManifestPolicy::default()
    };
    let loaded = load_manifest_checked(
        &downloader,
        &SharedDb(&state),
        &state.game.manifest_url,
        policy,
        &ctx,
    )
    .await?;
    Ok(to_list(loaded.manifest, loaded.stale))
}

/// 通过应用的共享数据库连接存取版本清单缓存。
struct SharedDb<'a>(&'a AppState);

impl ManifestStore for SharedDb<'_> {
    fn load_manifest(&self, url: &str) -> AppResult<Option<(String, i64)>> {
        self.0.with_db(|_, db| db.load_manifest(url))
    }

    fn save_manifest(&self, url: &str, body: &str) -> AppResult<()> {
        self.0.with_db(|_, db| db.save_manifest(url, body))
    }
}

/// 下载并校验运行该版本所需的全部文件，立即返回任务 ID。
///
/// 进度阶段依次为 `version_json`、`asset_index`、`libraries`、`assets`。
#[tauri::command]
pub async fn game_install<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: GameInstallArgs,
) -> Result<TaskStarted, AppError> {
    validate_version_id(&args.version_id)?;
    let paths = state.data()?.clone();
    let downloader = state.downloader()?;
    let options = state.game.clone();
    Ok(spawn_task(&app, GAME_INSTALL_KIND, move |ctx| async move {
        install_version(&downloader, &paths, &args.version_id, &options, &ctx).await?;
        Ok(())
    }))
}

fn to_list(manifest: VersionManifest, stale: Option<AppError>) -> GameVersionList {
    GameVersionList {
        stale,
        latest_release: manifest.latest.release,
        latest_snapshot: manifest.latest.snapshot,
        versions: manifest
            .versions
            .into_iter()
            .map(|v| GameVersion {
                id: v.id,
                kind: kind_name(v.kind).to_owned(),
                release_time: v.release_time,
            })
            .collect(),
    }
}

fn kind_name(kind: VersionType) -> &'static str {
    match kind {
        VersionType::Release => "release",
        VersionType::Snapshot => "snapshot",
        VersionType::OldBeta => "old_beta",
        VersionType::OldAlpha => "old_alpha",
        VersionType::Other => "other",
    }
}

/// 版本号会成为目录名：拒绝空串、路径分隔符与 `..`。
fn validate_version_id(id: &str) -> AppResult<()> {
    let valid = !id.trim().is_empty()
        && id.len() <= 64
        && !id.contains(['/', '\\', ':'])
        && !id.contains("..");
    if !valid {
        return Err(AppError::new(ErrorCode::CommonInternal)
            .with_detail("game_install: invalid version id"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_version_ids() {
        for ok in ["1.20.1", "23w13a", "1.14 Pre-Release 1"] {
            assert!(validate_version_id(ok).is_ok(), "{ok}");
        }
        for bad in ["", " ", "../x", "a/b", "a\\b", "C:x", &"1".repeat(65)] {
            assert!(validate_version_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn kind_names_match_manifest() {
        assert_eq!(kind_name(VersionType::OldAlpha), "old_alpha");
        assert_eq!(kind_name(VersionType::Release), "release");
    }
}
