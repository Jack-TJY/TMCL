//! `sync_*` 命令：设置与按键同步（PLAN.md M2-09），业务逻辑在 `tmcl-sync`。

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;
use tmcl_common::{AppError, AppResult, ErrorCode};
use tmcl_instance::InstanceStore;
use tmcl_sync::{Category, OptionChange, SyncOption, SyncProfile, apply, profile};
use ts_rs::TS;

use crate::state::AppState;

/// `sync_get_master` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SyncMaster {
    /// 主配置；从未保存过时为 `null`。
    pub profile: Option<SyncProfile>,
}

/// `sync_capture` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SyncCaptureArgs {
    /// 来源实例 ID。
    pub instance_id: String,
}

/// `sync_save_master` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SyncSaveMasterArgs {
    /// 手动编辑后的选项（1.13 以后的格式），整体替换主配置。
    pub options: Vec<SyncOption>,
}

/// `sync_preview` / `sync_apply` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SyncTargetsArgs {
    /// 目标实例 ID。
    pub instance_ids: Vec<String>,
    /// 要同步的类别。
    pub categories: Vec<Category>,
}

/// 一个目标实例的预览。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SyncTargetPreview {
    /// 实例 ID。
    pub instance_id: String,
    /// 实例的游戏版本，决定按键格式。
    pub game_version: String,
    /// 将要发生的修改，为空表示无需修改。
    pub changes: Vec<OptionChange>,
    /// 是否有可恢复的备份（`sync_restore` 可用）。
    pub has_backup: bool,
}

/// `sync_preview` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SyncPreview {
    /// 与参数中的实例顺序一致。
    pub targets: Vec<SyncTargetPreview>,
}

/// 一个目标实例的应用结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SyncTargetApplied {
    /// 实例 ID。
    pub instance_id: String,
    /// 修改的选项数。
    pub changed: u32,
    /// 原 `options.txt` 的备份路径；没有修改或原文件不存在时为 `null`。
    pub backup: Option<String>,
}

/// `sync_apply` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SyncApplyResult {
    /// 与参数中的实例顺序一致。
    pub targets: Vec<SyncTargetApplied>,
}

/// `sync_restore` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SyncRestoreArgs {
    /// 实例 ID。
    pub instance_id: String,
}

/// `sync_restore` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SyncRestoreResult {
    /// 是否恢复了备份；没有备份时为 `false`，文件不变。
    pub restored: bool,
}

/// 读取主配置。
#[tauri::command]
pub async fn sync_get_master(state: State<'_, AppState>) -> Result<SyncMaster, AppError> {
    Ok(SyncMaster {
        profile: profile::load_master(state.data()?)?,
    })
}

/// 从实例的 `options.txt` 采集主配置并保存。
#[tauri::command]
pub async fn sync_capture(
    state: State<'_, AppState>,
    args: SyncCaptureArgs,
) -> Result<SyncProfile, AppError> {
    let (game_dir, game_version) = instance_target(&state, &args.instance_id)?;
    let file = apply::read_existing(&game_dir)?.ok_or_else(|| {
        let path = game_dir.join(apply::OPTIONS_FILE);
        AppError::from(std::io::Error::from(std::io::ErrorKind::NotFound))
            .with_param("path", path.display())
    })?;
    let master = SyncProfile {
        source_instance: Some(args.instance_id),
        options: profile::capture_options(&file, &game_version),
        source_version: Some(game_version),
        updated_at: now_millis(),
    };
    profile::save_master(state.data()?, &master)?;
    Ok(master)
}

/// 用手动编辑的选项整体替换主配置。
#[tauri::command]
pub async fn sync_save_master(
    state: State<'_, AppState>,
    args: SyncSaveMasterArgs,
) -> Result<SyncProfile, AppError> {
    let master = SyncProfile {
        source_instance: None,
        source_version: None,
        updated_at: now_millis(),
        options: args.options,
    };
    profile::save_master(state.data()?, &master)?;
    Ok(master)
}

/// 预览把主配置应用到各目标实例时的修改，不写文件。
#[tauri::command]
pub async fn sync_preview(
    state: State<'_, AppState>,
    args: SyncTargetsArgs,
) -> Result<SyncPreview, AppError> {
    let master = require_master(&state)?;
    let mut targets = Vec::new();
    for instance_id in args.instance_ids {
        let (game_dir, game_version) = instance_target(&state, &instance_id)?;
        let file = apply::read_options(&game_dir)?;
        targets.push(SyncTargetPreview {
            changes: apply::plan(&master, &file, &game_version, &args.categories),
            has_backup: apply::latest_backup(&game_dir).is_some(),
            instance_id,
            game_version,
        });
    }
    Ok(SyncPreview { targets })
}

/// 把主配置应用到各目标实例；先确认所有实例存在，再逐个写入。
#[tauri::command]
pub async fn sync_apply(
    state: State<'_, AppState>,
    args: SyncTargetsArgs,
) -> Result<SyncApplyResult, AppError> {
    let master = require_master(&state)?;
    let resolved = args
        .instance_ids
        .into_iter()
        .map(|id| instance_target(&state, &id).map(|target| (id, target)))
        .collect::<AppResult<Vec<_>>>()?;
    let mut targets = Vec::new();
    for (instance_id, (game_dir, game_version)) in resolved {
        std::fs::create_dir_all(&game_dir)
            .map_err(|e| AppError::from(e).with_param("path", game_dir.display()))?;
        let applied = apply::apply(&game_dir, &game_version, &master, &args.categories)?;
        targets.push(SyncTargetApplied {
            instance_id,
            changed: u32::try_from(applied.changed).unwrap_or(u32::MAX),
            backup: applied.backup.map(|p| p.display().to_string()),
        });
    }
    Ok(SyncApplyResult { targets })
}

/// 用最近一份备份恢复实例的 `options.txt`（撤销最近一次应用）。
#[tauri::command]
pub async fn sync_restore(
    state: State<'_, AppState>,
    args: SyncRestoreArgs,
) -> Result<SyncRestoreResult, AppError> {
    let (game_dir, _) = instance_target(&state, &args.instance_id)?;
    Ok(SyncRestoreResult {
        restored: apply::restore_latest(&game_dir)?.is_some(),
    })
}

/// 实例的 `game/` 目录与游戏版本；实例不存在返回 `INSTANCE_NOT_FOUND`。
fn instance_target(state: &AppState, id: &str) -> AppResult<(PathBuf, String)> {
    state.with_db(|paths, db| {
        let config = InstanceStore::new(paths, db).get(id)?;
        Ok((paths.instance_game_dir(id), config.game_version))
    })
}

/// 已保存的主配置；尚未保存时返回 `COMMON_INTERNAL`。
fn require_master(state: &AppState) -> AppResult<SyncProfile> {
    profile::load_master(state.data()?)?.ok_or_else(|| {
        AppError::new(ErrorCode::CommonInternal).with_detail("sync master has not been saved")
    })
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}
