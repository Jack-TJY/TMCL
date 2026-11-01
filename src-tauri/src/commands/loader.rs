//! `loader_*` 命令：列出与游戏版本匹配的加载器版本，在实例上安装或更换加载器。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tmcl_common::{AppError, AppResult, DataPaths, Db, ErrorCode, TaskCtx};
use tmcl_instance::{InstanceStore, JavaSetting, LoaderKind, LoaderSpec};
use tmcl_loader::{JavaChoice, LoaderInstaller, LoaderVersion, list_versions};
use tmcl_resources::fabric_api::{FabricApiOutcome, install_fabric_api};
use ts_rs::TS;

use crate::events::{FabricApiFailed, LOADER_FABRIC_API_FAILED, TaskStarted, emit, spawn_task};
use crate::state::AppState;

/// 加载器安装任务的种类名。
pub const LOADER_INSTALL_KIND: &str = "loader_install";

/// `loader_list` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct LoaderListArgs {
    /// 加载器种类。
    pub kind: LoaderKind,
    /// 游戏版本号。
    pub game_version: String,
}

/// `loader_list` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LoaderVersionList {
    /// 可安装的版本，新版本在前；为空表示该加载器不支持此游戏版本。
    pub versions: Vec<LoaderVersion>,
}

/// `loader_install` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct LoaderInstallArgs {
    /// 实例 ID。
    pub instance_id: String,
    /// 加载器种类。
    pub kind: LoaderKind,
    /// 加载器版本，来自 `loader_list`。
    pub version: String,
    /// 加载器为 Fabric 时是否同时把匹配游戏版本的 Fabric API 装进 `mods/`（已安装或该游戏版本
    /// 没有 Fabric API 时跳过）。省略表示不安装。
    #[serde(default)]
    #[ts(optional)]
    pub with_fabric_api: Option<bool>,
}

/// 列出 `kind` 在该游戏版本上可安装的版本，新版本在前。
///
/// Fabric、Quilt 不支持该版本时返回空列表；Forge 不支持 1.7.10 以前的版本，返回
/// `LOADER_UNSUPPORTED_VERSION`。
#[tauri::command]
pub async fn loader_list(
    state: State<'_, AppState>,
    args: LoaderListArgs,
) -> Result<LoaderVersionList, AppError> {
    validate_segment(&args.game_version)?;
    let downloader = state.downloader()?;
    let ctx = TaskCtx::noop("loader_list");
    let versions = list_versions(
        &downloader,
        &state.loader,
        args.kind,
        &args.game_version,
        &ctx,
    )
    .await?;
    Ok(LoaderVersionList { versions })
}

/// 在实例上安装（或更换为）指定的加载器，立即返回任务 ID。
///
/// 必要时先补全原版文件；Fabric 且 `with_fabric_api` 时随后安装 Fabric API。成功后才把实例的
/// `loader` 改为新加载器，失败或取消时实例不变。Fabric API 安装失败（取消除外）不影响加载器：
/// 照常写回并推送 `loader:fabric-api-failed`，由界面提示用户重新安装。
#[tauri::command]
pub async fn loader_install<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: LoaderInstallArgs,
) -> Result<TaskStarted, AppError> {
    validate_segment(&args.version)?;
    let config = state.with_db(|paths, db| InstanceStore::new(paths, db).get(&args.instance_id))?;
    let paths = state.data()?.clone();
    let java = match config.java {
        JavaSetting::Auto => JavaChoice::Auto,
        JavaSetting::Path { path } => JavaChoice::Path(path.into()),
    };
    let installer = LoaderInstaller::new(state.downloader()?, paths.clone())
        .with_endpoints(state.loader.clone())
        .with_game_options(state.game.clone())
        .with_java(java);
    let fabric_api = if args.kind == LoaderKind::Fabric && args.with_fabric_api == Some(true) {
        let game_dir = paths.instance_game_dir(&args.instance_id);
        Some((state.modrinth()?.clone(), state.downloader()?, game_dir))
    } else {
        None
    };
    let game_version = config.game_version;
    let instance_name = config.name;
    let emitter = app.clone();
    Ok(spawn_task(
        &app,
        LOADER_INSTALL_KIND,
        move |ctx| async move {
            installer
                .install(args.kind, &game_version, &args.version, &ctx)
                .await?;
            if let Some((client, downloader, game_dir)) = &fabric_api {
                let result =
                    install_fabric_api(client, downloader, game_dir, &game_version, &ctx).await;
                if let Some(error) = fabric_api_warning(result)? {
                    let payload = FabricApiFailed {
                        task_id: ctx.id,
                        instance_id: args.instance_id.clone(),
                        instance_name,
                        error,
                    };
                    emit(&emitter, LOADER_FABRIC_API_FAILED, &payload);
                }
            }
            ctx.check_cancelled()?;
            let loader = LoaderSpec {
                kind: args.kind,
                version: args.version,
            };
            set_loader(paths, args.instance_id, loader).await
        },
    ))
}

/// Fabric API 的安装结果：取消照常向上返回（任务取消、实例不变）；其他失败只记日志并返回
/// 错误供界面提示，不让加载器安装失败。
fn fabric_api_warning(result: AppResult<FabricApiOutcome>) -> AppResult<Option<AppError>> {
    match result {
        Ok(_) => Ok(None),
        Err(err) if err.is_cancelled() => Err(err),
        Err(err) => {
            tracing::warn!(error = %err, "fabric api: install failed, loader kept");
            Ok(Some(err))
        }
    }
}

/// 安装成功后写回实例的 `loader`；重新读取配置，保留安装期间对实例的其他修改。
async fn set_loader(paths: DataPaths, id: String, loader: LoaderSpec) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = Db::open(&paths.db_file())?;
        let store = InstanceStore::new(&paths, &db);
        let mut config = store.get(&id)?;
        config.loader = Some(loader);
        store.save(&config).map(drop)
    })
    .await
    .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?
}

/// 游戏与加载器版本号会出现在 URL 路径和目录名中：拒绝空串、路径分隔符与 `..`。
fn validate_segment(value: &str) -> AppResult<()> {
    let valid = !value.trim().is_empty()
        && value.len() <= 64
        && !value.contains(['/', '\\', ':', '?', '#'])
        && !value.contains("..");
    if !valid {
        return Err(AppError::new(ErrorCode::CommonInternal).with_detail("loader: invalid version"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabric_api_failure_is_a_warning_but_cancel_is_not() {
        assert_eq!(
            fabric_api_warning(Ok(FabricApiOutcome::Unavailable)).unwrap(),
            None
        );
        let err = fabric_api_warning(Err(AppError::new(ErrorCode::NetConnect)))
            .unwrap()
            .unwrap();
        assert_eq!(err.code, ErrorCode::NetConnect);
        let cancelled = fabric_api_warning(Err(AppError::cancelled())).unwrap_err();
        assert!(cancelled.is_cancelled());
    }

    #[test]
    fn rejects_unsafe_versions() {
        for ok in [
            "1.20.1",
            "0.16.5",
            "47.2.0",
            "1.20.1-47.2.0",
            "0.24.0-beta.9",
        ] {
            assert!(validate_segment(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            " ",
            "../x",
            "a/b",
            "a\\b",
            "C:x",
            "a?b",
            &"1".repeat(65),
        ] {
            assert!(validate_segment(bad).is_err(), "{bad}");
        }
    }
}
