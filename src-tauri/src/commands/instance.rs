//! `instance_*` 命令：实例的增删改查、复制与 `.tmclpack` 导入导出。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tmcl_common::{AppError, AppResult, DataPaths, Db, ErrorCode, TaskCtx};
use tmcl_instance::store::is_valid_id;
use tmcl_instance::{InstanceConfig, InstanceStore, InstanceSummary, LoaderSpec};
use ts_rs::TS;

use crate::commands::settings::open_in_file_manager;
use crate::events::{TaskStarted, spawn_task};
use crate::state::AppState;

/// 复制实例任务的种类名。
pub const INSTANCE_DUPLICATE_KIND: &str = "instance_duplicate";
/// 导出实例任务的种类名。
pub const INSTANCE_EXPORT_KIND: &str = "instance_export";
/// 导入实例任务的种类名。
pub const INSTANCE_IMPORT_KIND: &str = "instance_import";

/// `instance_list` / `instance_refresh` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct InstanceList {
    /// 最近启动的在前，从未启动的按创建时间倒序；损坏的实例带 `error`。
    pub instances: Vec<InstanceSummary>,
}

/// 只带实例 ID 的参数（`instance_get`、`instance_delete`、`instance_game_entries`）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct InstanceIdArgs {
    /// 实例 ID。
    pub id: String,
}

/// `instance_create` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct InstanceCreateArgs {
    /// 显示名称。
    pub name: String,
    /// 游戏版本号。
    pub game_version: String,
    /// 模组加载器，省略表示原版。
    #[serde(default)]
    #[ts(optional)]
    pub loader: Option<LoaderSpec>,
}

/// `instance_save` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct InstanceSaveArgs {
    /// 修改后的完整配置（实例必须已存在）。
    pub config: InstanceConfig,
}

/// `instance_rename` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct InstanceRenameArgs {
    /// 实例 ID。
    pub id: String,
    /// 新名称。
    pub name: String,
}

/// `instance_game_entries` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct InstanceGameEntries {
    /// `game/` 下的顶层条目名，按名称排序。
    pub entries: Vec<String>,
}

/// `instance_duplicate` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct InstanceDuplicateArgs {
    /// 源实例 ID。
    pub id: String,
    /// 新实例名称。
    pub name: String,
    /// 是否复制存档（`game/saves/`）。
    pub include_saves: bool,
}

/// `instance_export` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct InstanceExportArgs {
    /// 实例 ID。
    pub id: String,
    /// 目标文件的绝对路径。
    pub dest: String,
    /// 要包含的 `game/` 顶层条目名（见 `instance_game_entries`）。
    pub include: Vec<String>,
}

/// `instance_import` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct InstanceImportArgs {
    /// `.tmclpack` 文件的绝对路径。
    pub pack: String,
}

/// 读取实例索引。
#[tauri::command]
pub async fn instance_list(state: State<'_, AppState>) -> Result<InstanceList, AppError> {
    state.with_db(|paths, db| list(&InstanceStore::new(paths, db)))
}

/// 与磁盘重新对账后读取实例索引（用户手动改动实例目录后使用）。
#[tauri::command]
pub async fn instance_refresh(state: State<'_, AppState>) -> Result<InstanceList, AppError> {
    state.with_db(|paths, db| {
        let store = InstanceStore::new(paths, db);
        store.reconcile()?;
        list(&store)
    })
}

/// 读取实例配置。
#[tauri::command]
pub async fn instance_get(
    state: State<'_, AppState>,
    args: InstanceIdArgs,
) -> Result<InstanceConfig, AppError> {
    state.with_db(|paths, db| InstanceStore::new(paths, db).get(&args.id))
}

/// 新建实例（只写配置，不下载游戏文件）。
#[tauri::command]
pub async fn instance_create(
    state: State<'_, AppState>,
    args: InstanceCreateArgs,
) -> Result<InstanceConfig, AppError> {
    state.with_db(|paths, db| {
        InstanceStore::new(paths, db).create(&args.name, &args.game_version, args.loader)
    })
}

/// 保存修改后的配置，返回规整后的配置。
#[tauri::command]
pub async fn instance_save(
    state: State<'_, AppState>,
    args: InstanceSaveArgs,
) -> Result<InstanceConfig, AppError> {
    state.with_db(|paths, db| InstanceStore::new(paths, db).save(&args.config))
}

/// 重命名实例（ID 与目录不变）。
#[tauri::command]
pub async fn instance_rename(
    state: State<'_, AppState>,
    args: InstanceRenameArgs,
) -> Result<InstanceConfig, AppError> {
    state.with_db(|paths, db| InstanceStore::new(paths, db).rename(&args.id, &args.name))
}

/// 删除实例：移到回收站，7 天后清理。
#[tauri::command]
pub async fn instance_delete(
    state: State<'_, AppState>,
    args: InstanceIdArgs,
) -> Result<(), AppError> {
    state.with_db(|paths, db| InstanceStore::new(paths, db).delete(&args.id))
}

/// 列出 `game/` 下的顶层条目，供导出时选择。
#[tauri::command]
pub async fn instance_game_entries(
    state: State<'_, AppState>,
    args: InstanceIdArgs,
) -> Result<InstanceGameEntries, AppError> {
    state.with_db(|paths, db| {
        let entries = InstanceStore::new(paths, db).game_entries(&args.id)?;
        Ok(InstanceGameEntries { entries })
    })
}

/// 在系统文件管理器中打开实例的游戏目录（`game/`，不存在时先创建）；损坏的实例也可打开。
#[tauri::command]
pub async fn instance_open_dir(
    state: State<'_, AppState>,
    args: InstanceIdArgs,
) -> Result<(), AppError> {
    let paths = prepare(&state)?;
    open_in_file_manager(&game_dir(&paths, &args.id)?)
}

/// 已存在实例的 `game/` 目录，按需创建。
fn game_dir(paths: &DataPaths, id: &str) -> AppResult<PathBuf> {
    if !is_valid_id(id) || !paths.instance_dir(id).is_dir() {
        return Err(AppError::new(ErrorCode::InstanceNotFound).with_param("id", id));
    }
    let dir = paths.instance_game_dir(id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::from(e).with_param("path", dir.display()))?;
    Ok(dir)
}

/// 复制实例，立即返回任务 ID；完成后重新读取实例列表即可看到新实例。
#[tauri::command]
pub async fn instance_duplicate<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: InstanceDuplicateArgs,
) -> Result<TaskStarted, AppError> {
    let paths = prepare(&state)?;
    Ok(spawn_task(&app, INSTANCE_DUPLICATE_KIND, move |ctx| {
        blocking(paths, ctx, move |store, ctx| {
            store
                .duplicate(&args.id, &args.name, args.include_saves, ctx)
                .map(drop)
        })
    }))
}

/// 导出实例为 `.tmclpack`，立即返回任务 ID。
#[tauri::command]
pub async fn instance_export<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: InstanceExportArgs,
) -> Result<TaskStarted, AppError> {
    let dest = absolute_path(&args.dest)?;
    let paths = prepare(&state)?;
    Ok(spawn_task(&app, INSTANCE_EXPORT_KIND, move |ctx| {
        blocking(paths, ctx, move |store, ctx| {
            store.export_pack(&args.id, &dest, &args.include, ctx)
        })
    }))
}

/// 从 `.tmclpack` 导入为新实例，立即返回任务 ID。
#[tauri::command]
pub async fn instance_import<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: InstanceImportArgs,
) -> Result<TaskStarted, AppError> {
    let pack = absolute_path(&args.pack)?;
    let paths = prepare(&state)?;
    Ok(spawn_task(&app, INSTANCE_IMPORT_KIND, move |ctx| {
        blocking(paths, ctx, move |store, ctx| {
            store.import_pack(&pack, ctx).map(drop)
        })
    }))
}

fn list(store: &InstanceStore<'_>) -> AppResult<InstanceList> {
    Ok(InstanceList {
        instances: store.list()?,
    })
}

/// 确保数据库已打开（迁移与对账已完成），返回数据目录供后台任务使用。
fn prepare(state: &AppState) -> AppResult<DataPaths> {
    state.with_db(|paths, _| Ok(paths.clone()))
}

/// 在阻塞线程上用独立的数据库连接执行实例操作，避免长时间占用共享连接。
async fn blocking<F>(paths: DataPaths, ctx: TaskCtx, job: F) -> AppResult<()>
where
    F: FnOnce(&InstanceStore<'_>, &TaskCtx) -> AppResult<()> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let db = Db::open(&paths.db_file())?;
        job(&InstanceStore::new(&paths, &db), &ctx)
    })
    .await
    .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?
}

/// 文件路径参数必须是绝对路径（相对路径的基准目录不确定）。
fn absolute_path(path: &str) -> AppResult<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(AppError::new(ErrorCode::CommonInternal)
            .with_detail("instance: file path must be absolute"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_dir_requires_existing_instance() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        for id in ["missing", "../escape", ""] {
            let err = game_dir(&paths, id).unwrap_err();
            assert_eq!(err.code, ErrorCode::InstanceNotFound);
        }
        std::fs::create_dir_all(paths.instance_dir("abc")).unwrap();
        let game = game_dir(&paths, "abc").unwrap();
        assert!(game.is_dir());
        assert_eq!(game, paths.instance_game_dir("abc"));
    }

    #[test]
    fn requires_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let abs = dir.path().join("a.tmclpack");
        assert_eq!(absolute_path(abs.to_str().unwrap()).unwrap(), abs);
        let err = absolute_path("a.tmclpack").unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonInternal);
    }
}
