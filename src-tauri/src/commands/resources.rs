//! `resources_*` 命令：按实例搜索 Modrinth 资源、列出版本、生成安装计划并安装。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tmcl_common::{AppError, AppResult, TaskCtx};
use tmcl_instance::InstanceStore;
use tmcl_resources::browse::{self, BrowseQuery};
use tmcl_resources::modrinth::ModrinthClient;
use tmcl_resources::{
    InstallPlan, InstallRequest, InstanceTarget, ResourceInstaller, ResourceKind, ResourcePage,
    ResourceSort, ResourceVersion,
};
use ts_rs::TS;

use crate::events::{TaskStarted, spawn_task};
use crate::state::AppState;

/// 资源安装任务的种类名。
pub const RESOURCES_INSTALL_KIND: &str = "resources_install";

/// `resources_search` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesSearchArgs {
    /// 按该实例的游戏版本与加载器过滤。
    pub instance_id: String,
    /// 资源种类。
    pub kind: ResourceKind,
    /// 关键词，可为空。
    #[serde(default)]
    pub query: String,
    /// 排序方式，默认相关度。
    #[serde(default)]
    #[ts(optional)]
    pub sort: Option<ResourceSort>,
    /// 跳过的条数。
    #[serde(default)]
    #[ts(optional)]
    pub offset: Option<u32>,
    /// 每页条数，默认 20，最多 100。
    #[serde(default)]
    #[ts(optional)]
    pub limit: Option<u32>,
}

/// `resources_versions` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesVersionsArgs {
    /// 按该实例的游戏版本与加载器过滤。
    pub instance_id: String,
    /// 资源种类。
    pub kind: ResourceKind,
    /// Modrinth 项目 ID。
    pub project_id: String,
}

/// `resources_versions` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ResourceVersionList {
    /// 适用于该实例的版本，新版本在前。
    pub versions: Vec<ResourceVersion>,
}

/// `resources_install_plan` 与 `resources_install` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesInstallArgs {
    /// 安装到的实例。
    pub instance_id: String,
    /// 资源种类，决定安装目录。
    pub kind: ResourceKind,
    /// Modrinth 版本 ID，来自 `resources_versions`。
    pub version_id: String,
    /// 数据包安装到的存档（`saves/` 下的目录名），其他种类忽略。
    #[serde(default)]
    #[ts(optional)]
    pub world: Option<String>,
}

impl ResourcesInstallArgs {
    fn request(&self) -> InstallRequest {
        InstallRequest {
            version_id: self.version_id.clone(),
            kind: self.kind,
            world: self.world.clone(),
        }
    }
}

/// 按实例的游戏版本与加载器搜索资源。
#[tauri::command]
pub async fn resources_search(
    state: State<'_, AppState>,
    args: ResourcesSearchArgs,
) -> Result<ResourcePage, AppError> {
    let target = target(&state, &args.instance_id)?;
    let query = BrowseQuery {
        query: args.query,
        sort: args.sort.unwrap_or_default(),
        offset: args.offset.unwrap_or_default(),
        limit: args.limit.unwrap_or_default(),
    };
    let ctx = TaskCtx::noop("resources_search");
    browse::search(state.modrinth()?, &target, args.kind, &query, &ctx).await
}

/// 列出项目中适用于该实例的版本。
#[tauri::command]
pub async fn resources_versions(
    state: State<'_, AppState>,
    args: ResourcesVersionsArgs,
) -> Result<ResourceVersionList, AppError> {
    let target = target(&state, &args.instance_id)?;
    let ctx = TaskCtx::noop("resources_versions");
    let versions = browse::versions(
        state.modrinth()?,
        &target,
        args.kind,
        &args.project_id,
        &ctx,
    )
    .await?;
    Ok(ResourceVersionList { versions })
}

/// 生成安装计划：要下载的文件（含递归的必需前置）、不兼容的项目与缺失的前置。不写文件。
#[tauri::command]
pub async fn resources_install_plan(
    state: State<'_, AppState>,
    args: ResourcesInstallArgs,
) -> Result<InstallPlan, AppError> {
    let installer = installer(&state, &args.instance_id)?;
    let ctx = TaskCtx::noop("resources_install_plan");
    installer.plan(&args.request(), &ctx).await
}

/// 安装资源及其前置，立即返回任务 ID。任务中重新生成计划，有冲突或缺失的前置时失败，不写文件。
#[tauri::command]
pub async fn resources_install<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: ResourcesInstallArgs,
) -> Result<TaskStarted, AppError> {
    let installer = installer(&state, &args.instance_id)?;
    let request = args.request();
    Ok(spawn_task(
        &app,
        RESOURCES_INSTALL_KIND,
        move |ctx| async move { installer.install(&request, &ctx).await.map(drop) },
    ))
}

/// 读取实例的游戏版本与加载器；实例不存在返回 `INSTANCE_NOT_FOUND`。
fn target(state: &AppState, instance_id: &str) -> AppResult<InstanceTarget> {
    let config = state.with_db(|paths, db| InstanceStore::new(paths, db).get(instance_id))?;
    Ok(InstanceTarget {
        game_version: config.game_version,
        loader: config.loader.map(|l| l.kind.as_str().to_owned()),
    })
}

fn installer(state: &AppState, instance_id: &str) -> AppResult<ResourceInstaller> {
    let target = target(state, instance_id)?;
    let game_dir = state.data()?.instance_game_dir(instance_id);
    let client: ModrinthClient = state.modrinth()?.clone();
    Ok(ResourceInstaller::new(
        client,
        state.downloader()?,
        game_dir,
        target,
    ))
}
