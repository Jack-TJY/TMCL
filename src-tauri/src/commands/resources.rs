//! `resources_*` 命令：按实例（或不限实例）搜索 Modrinth 资源、列出版本、生成安装计划并安装；
//! 管理已安装资源（列表、从本地导入、启用/禁用、删除、检查更新与更新）；在 Modrinth 或 CurseForge
//! 搜索整合包并安装为新实例，把实例导出为 `.mrpack` 或 CurseForge 整合包。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_instance::InstanceStore;
use tmcl_loader::LoaderInstaller;
use tmcl_resources::browse::{self, BrowseQuery};
use tmcl_resources::modpack::curseforge::{Fallbacks, MIRROR_BASE};
use tmcl_resources::modpack::{export, source};
use tmcl_resources::modrinth::ModrinthClient;
use tmcl_resources::update;
use tmcl_resources::{
    InstallPlan, InstallRequest, InstalledList, InstanceTarget, ModpackExport, ModpackFormat,
    ModpackInfo, ModpackInstaller, ModpackSource, ResourceImportResult, ResourceInstaller,
    ResourceKind, ResourceManager, ResourcePage, ResourcePlatform, ResourceSort, ResourceUpdate,
    ResourceVersion,
};
use ts_rs::TS;

use crate::commands::settings::open_in_file_manager;
use crate::events::{TaskStarted, spawn_task};
use crate::state::AppState;

/// 资源安装任务的种类名。
pub const RESOURCES_INSTALL_KIND: &str = "resources_install";

/// 资源更新任务的种类名。
pub const RESOURCES_UPDATE_KIND: &str = "resources_update";

/// 整合包安装任务的种类名。
pub const RESOURCES_MODPACK_INSTALL_KIND: &str = "resources_modpack_install";

/// 整合包导出任务的种类名。
pub const RESOURCES_MODPACK_EXPORT_KIND: &str = "resources_modpack_export";

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

/// `resources_search_any` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesSearchAnyArgs {
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

/// `resources_installed`、`resources_check_updates` 的参数：实例中的某一类资源。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesDirArgs {
    /// 实例 ID。
    pub instance_id: String,
    /// 资源种类。
    pub kind: ResourceKind,
    /// 数据包所在的存档，其他种类忽略。
    #[serde(default)]
    #[ts(optional)]
    pub world: Option<String>,
}

/// `resources_import` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesImportArgs {
    /// 实例 ID。
    pub instance_id: String,
    /// 资源种类，决定导入的目录。
    pub kind: ResourceKind,
    /// 数据包所在的存档，其他种类忽略。
    #[serde(default)]
    #[ts(optional)]
    pub world: Option<String>,
    /// 要导入的本地文件的绝对路径，来自文件对话框。
    pub paths: Vec<String>,
}

/// `resources_set_enabled` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesSetEnabledArgs {
    /// 实例 ID。
    pub instance_id: String,
    /// 资源种类。
    pub kind: ResourceKind,
    /// 数据包所在的存档，其他种类忽略。
    #[serde(default)]
    #[ts(optional)]
    pub world: Option<String>,
    /// 当前文件名，来自 `resources_installed`。
    pub file_name: String,
    /// 启用（去掉 `.disabled`）或禁用（加上 `.disabled`）。
    pub enabled: bool,
}

/// `resources_set_enabled` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ResourcesSetEnabledResult {
    /// 改名后的文件名。
    pub file_name: String,
}

/// `resources_remove` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesRemoveArgs {
    /// 实例 ID。
    pub instance_id: String,
    /// 资源种类。
    pub kind: ResourceKind,
    /// 数据包所在的存档，其他种类忽略。
    #[serde(default)]
    #[ts(optional)]
    pub world: Option<String>,
    /// 文件名，来自 `resources_installed`。
    pub file_name: String,
}

/// `resources_check_updates` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ResourceUpdateList {
    /// 可更新的资源，按文件名排序。
    pub updates: Vec<ResourceUpdate>,
}

/// `resources_update` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesUpdateArgs {
    /// 实例 ID。
    pub instance_id: String,
    /// 资源种类。
    pub kind: ResourceKind,
    /// 数据包所在的存档，其他种类忽略。
    #[serde(default)]
    #[ts(optional)]
    pub world: Option<String>,
    /// 要更新的文件名；省略时更新全部可更新的资源。
    #[serde(default)]
    #[ts(optional)]
    pub file_names: Option<Vec<String>>,
}

/// `resources_worlds` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesWorldsArgs {
    /// 实例 ID。
    pub instance_id: String,
}

/// `resources_worlds` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ResourceWorldList {
    /// `saves/` 下的存档目录名，按名称排序。
    pub worlds: Vec<String>,
}

/// `resources_open_page` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesOpenPageArgs {
    /// 要在浏览器中打开的 CurseForge 网页（如需要手动下载的文件页）。
    pub url: String,
}

/// `resources_sources` 的返回值：本次构建可用的资源平台。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ResourceSources {
    /// 是否可用 CurseForge（构建时注入了 API Key）；为 `false` 时界面应隐藏 CurseForge。
    pub curseforge: bool,
}

/// `resources_modpack_search` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesModpackSearchArgs {
    /// 搜索哪个平台，默认 Modrinth。
    #[serde(default)]
    #[ts(optional)]
    pub platform: Option<ResourcePlatform>,
    /// 关键词，可为空。
    #[serde(default)]
    pub query: String,
    /// 只要支持该游戏版本的整合包；省略表示不限。
    #[serde(default)]
    #[ts(optional)]
    pub game_version: Option<String>,
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

/// `resources_modpack_versions` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesModpackVersionsArgs {
    /// 项目所在平台，默认 Modrinth。
    #[serde(default)]
    #[ts(optional)]
    pub platform: Option<ResourcePlatform>,
    /// 整合包项目 ID（CurseForge 为十进制数字）。
    pub project_id: String,
    /// 只要支持该游戏版本的版本；省略表示不限。
    #[serde(default)]
    #[ts(optional)]
    pub game_version: Option<String>,
}

/// `resources_modpack_inspect` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesModpackInspectArgs {
    /// 本地 `.mrpack` 或 CurseForge 整合包 zip 的绝对路径。
    pub path: String,
}

/// `resources_modpack_install` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesModpackInstallArgs {
    /// 整合包来源。
    pub source: ModpackSource,
    /// 新实例名称；省略或为空时使用整合包名称。
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
}

/// `resources_modpack_export` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct ResourcesModpackExportArgs {
    /// 要导出的实例。
    pub instance_id: String,
    /// 目标文件（`.mrpack` 或 `.zip`）的绝对路径。
    pub dest: String,
    /// 导出格式，默认 `.mrpack`。
    #[serde(default)]
    #[ts(optional)]
    pub format: Option<ModpackFormat>,
    /// 要包含的 `game/` 顶层条目名（见 `instance_game_entries`）。
    pub include: Vec<String>,
    /// 整合包名称；省略或为空时使用实例名称。
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    /// 整合包版本号；省略或为空时为 `1.0.0`。
    #[serde(default)]
    #[ts(optional)]
    pub version: Option<String>,
    /// 简介。
    #[serde(default)]
    #[ts(optional)]
    pub summary: Option<String>,
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

/// 不按实例过滤，搜索全部资源。
#[tauri::command]
pub async fn resources_search_any(
    state: State<'_, AppState>,
    args: ResourcesSearchAnyArgs,
) -> Result<ResourcePage, AppError> {
    let query = BrowseQuery {
        query: args.query,
        sort: args.sort.unwrap_or_default(),
        offset: args.offset.unwrap_or_default(),
        limit: args.limit.unwrap_or_default(),
    };
    let ctx = TaskCtx::noop("resources_search_any");
    browse::search_any(state.modrinth()?, args.kind, &query, &ctx).await
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

/// 列出实例中某类已安装的资源，按 SHA1 在 Modrinth 反查来源；反查失败时列表照常返回。
#[tauri::command]
pub async fn resources_installed(
    state: State<'_, AppState>,
    args: ResourcesDirArgs,
) -> Result<InstalledList, AppError> {
    let manager = manager(&state, &args.instance_id, args.kind, args.world.as_deref())?;
    manager.list(&TaskCtx::noop("resources_installed")).await
}

/// 把本地文件复制进实例的资源目录，不覆盖同名文件。
#[tauri::command]
pub async fn resources_import(
    state: State<'_, AppState>,
    args: ResourcesImportArgs,
) -> Result<ResourceImportResult, AppError> {
    let manager = manager(&state, &args.instance_id, args.kind, args.world.as_deref())?;
    let paths: Vec<PathBuf> = args.paths.iter().map(PathBuf::from).collect();
    manager.import(&paths)
}

/// 启用或禁用资源文件（`.disabled` 后缀）。
#[tauri::command]
pub async fn resources_set_enabled(
    state: State<'_, AppState>,
    args: ResourcesSetEnabledArgs,
) -> Result<ResourcesSetEnabledResult, AppError> {
    let manager = manager(&state, &args.instance_id, args.kind, args.world.as_deref())?;
    let file_name = manager.set_enabled(&args.file_name, args.enabled)?;
    Ok(ResourcesSetEnabledResult { file_name })
}

/// 删除资源文件。
#[tauri::command]
pub async fn resources_remove(
    state: State<'_, AppState>,
    args: ResourcesRemoveArgs,
) -> Result<(), AppError> {
    let manager = manager(&state, &args.instance_id, args.kind, args.world.as_deref())?;
    manager.remove(&args.file_name)
}

/// 检查已安装资源在 Modrinth 上是否有适用于实例的新版本。
#[tauri::command]
pub async fn resources_check_updates(
    state: State<'_, AppState>,
    args: ResourcesDirArgs,
) -> Result<ResourceUpdateList, AppError> {
    let manager = manager(&state, &args.instance_id, args.kind, args.world.as_deref())?;
    let updates = manager
        .check_updates(&TaskCtx::noop("resources_check_updates"))
        .await?;
    Ok(ResourceUpdateList { updates })
}

/// 更新资源（一键更新时省略 `file_names`），立即返回任务 ID。
#[tauri::command]
pub async fn resources_update<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: ResourcesUpdateArgs,
) -> Result<TaskStarted, AppError> {
    let manager = manager(&state, &args.instance_id, args.kind, args.world.as_deref())?;
    let downloader = state.downloader()?;
    let files = args.file_names;
    Ok(spawn_task(
        &app,
        RESOURCES_UPDATE_KIND,
        move |ctx| async move {
            let done = manager.update(&downloader, files.as_deref(), &ctx).await?;
            Ok(done.len())
        },
    ))
}

/// 列出实例的存档，供选择数据包的安装目标。
#[tauri::command]
pub async fn resources_worlds(
    state: State<'_, AppState>,
    args: ResourcesWorldsArgs,
) -> Result<ResourceWorldList, AppError> {
    target(&state, &args.instance_id)?;
    let game_dir = state.data()?.instance_game_dir(&args.instance_id);
    Ok(ResourceWorldList {
        worlds: update::worlds(&game_dir)?,
    })
}

/// 在系统浏览器中打开 CurseForge 网页，用于手动下载作者禁止第三方分发的文件。
/// 只接受 `https://www.curseforge.com/` 下的地址，其他地址返回 `COMMON_INTERNAL`（`params.url`）。
#[tauri::command]
pub async fn resources_open_page(args: ResourcesOpenPageArgs) -> Result<(), AppError> {
    if !is_curseforge_page(&args.url) {
        return Err(AppError::new(ErrorCode::CommonInternal).with_param("url", &args.url));
    }
    open_in_file_manager(std::path::Path::new(&args.url))
}

/// `https://www.curseforge.com/<路径>`，且不含会被 shell 解释的字符。
fn is_curseforge_page(url: &str) -> bool {
    url.strip_prefix("https://www.curseforge.com/").is_some()
        && url
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~/:?=%".contains(&b))
}

/// 本次构建可用的资源平台。
#[tauri::command]
pub fn resources_sources(state: State<'_, AppState>) -> ResourceSources {
    ResourceSources {
        curseforge: state.curseforge.is_some(),
    }
}

/// 在 Modrinth 或 CurseForge 搜索整合包；没有 CurseForge Key 时搜索 CurseForge 返回
/// `RESOURCES_CURSEFORGE_DISABLED`。
#[tauri::command]
pub async fn resources_modpack_search(
    state: State<'_, AppState>,
    args: ResourcesModpackSearchArgs,
) -> Result<ResourcePage, AppError> {
    let query = BrowseQuery {
        query: args.query,
        sort: args.sort.unwrap_or_default(),
        offset: args.offset.unwrap_or_default(),
        limit: args.limit.unwrap_or_default(),
    };
    let ctx = TaskCtx::noop("resources_modpack_search");
    let game_version = args.game_version.as_deref();
    match args.platform.unwrap_or_default() {
        ResourcePlatform::Modrinth => {
            source::search(state.modrinth()?, game_version, &query, &ctx).await
        }
        ResourcePlatform::Curseforge => {
            source::search_curseforge(state.curseforge()?, game_version, &query, &ctx).await
        }
    }
}

/// 列出整合包项目的版本。
#[tauri::command]
pub async fn resources_modpack_versions(
    state: State<'_, AppState>,
    args: ResourcesModpackVersionsArgs,
) -> Result<ResourceVersionList, AppError> {
    let ctx = TaskCtx::noop("resources_modpack_versions");
    let (id, game_version) = (&args.project_id, args.game_version.as_deref());
    let versions = match args.platform.unwrap_or_default() {
        ResourcePlatform::Modrinth => {
            source::versions(state.modrinth()?, id, game_version, &ctx).await?
        }
        ResourcePlatform::Curseforge => {
            source::versions_curseforge(state.curseforge()?, id, game_version, &ctx).await?
        }
    };
    Ok(ResourceVersionList { versions })
}

/// 读取并校验本地整合包（`.mrpack` 或 CurseForge zip），返回概要，不写文件。
///
/// CurseForge 整合包会向 API 查询文件，概要中列出需要手动下载的文件。
#[tauri::command]
pub async fn resources_modpack_inspect(
    state: State<'_, AppState>,
    args: ResourcesModpackInspectArgs,
) -> Result<ModpackInfo, AppError> {
    let installer = modpack_installer(&state)?;
    Ok(installer.inspect(args.path.as_ref()).await?.info)
}

/// 把整合包安装为新实例，立即返回任务 ID；失败或取消时不留下新实例。
#[tauri::command]
pub async fn resources_modpack_install<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: ResourcesModpackInstallArgs,
) -> Result<TaskStarted, AppError> {
    let installer = modpack_installer(&state)?;
    let client = state.modrinth()?.clone();
    let curseforge = state.curseforge.clone();
    let downloader = state.downloader()?;
    let cache_dir = state.data()?.cache_dir().join("modpacks");
    Ok(spawn_task(
        &app,
        RESOURCES_MODPACK_INSTALL_KIND,
        move |ctx| async move {
            let pack = source::fetch(
                &args.source,
                &client,
                curseforge.as_ref(),
                &downloader,
                &cache_dir,
                &ctx,
            )
            .await?;
            let result = installer
                .install(&pack.path, args.name.as_deref(), &ctx)
                .await;
            pack.cleanup();
            result.map(drop)
        },
    ))
}

/// 把实例导出为 `.mrpack` 或 CurseForge 整合包，立即返回任务 ID；没有 CurseForge Key 时导出
/// CurseForge 格式返回 `RESOURCES_CURSEFORGE_DISABLED`。
#[tauri::command]
pub async fn resources_modpack_export<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: ResourcesModpackExportArgs,
) -> Result<TaskStarted, AppError> {
    let config = state.with_db(|paths, db| InstanceStore::new(paths, db).get(&args.instance_id))?;
    let game_dir = state.data()?.instance_game_dir(&config.id);
    let format = args.format.unwrap_or(ModpackFormat::Mrpack);
    let client = state.modrinth()?.clone();
    let curseforge = match format {
        ModpackFormat::Mrpack => None,
        ModpackFormat::Curseforge => Some(state.curseforge()?.clone()),
    };
    let pack = ModpackExport {
        name: non_empty(args.name).unwrap_or(config.name),
        version: non_empty(args.version).unwrap_or_else(|| "1.0.0".to_owned()),
        summary: non_empty(args.summary),
        game_version: config.game_version,
        loader: config.loader,
        include: args.include,
    };
    let dest = PathBuf::from(args.dest);
    Ok(spawn_task(
        &app,
        RESOURCES_MODPACK_EXPORT_KIND,
        move |ctx| async move {
            match &curseforge {
                None => export::export(&client, &game_dir, &dest, &pack, &ctx).await,
                Some(cf) => export::export_curseforge(cf, &game_dir, &dest, &pack, &ctx).await,
            }
            .map(drop)
        },
    ))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// 新实例的 Java 为自动，因此加载器安装器使用默认的 [`tmcl_loader::JavaChoice::Auto`]。
fn modpack_installer(state: &AppState) -> AppResult<ModpackInstaller> {
    let loader = LoaderInstaller::new(state.downloader()?, state.data()?.clone())
        .with_endpoints(state.loader.clone())
        .with_game_options(state.game.clone());
    // CurseForge 作者禁止分发的文件：先找 Modrinth 上的同一文件，再试 MCIM 镜像（维护者已知悉风险）。
    let fallbacks = Fallbacks {
        modrinth: state.modrinth().ok().cloned(),
        mirror: Some(MIRROR_BASE.to_owned()),
    };
    Ok(ModpackInstaller::new(loader, state.game.clone())
        .with_curseforge(state.curseforge.clone())
        .with_fallbacks(fallbacks))
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

fn manager(
    state: &AppState,
    instance_id: &str,
    kind: ResourceKind,
    world: Option<&str>,
) -> AppResult<ResourceManager> {
    let target = target(state, instance_id)?;
    let game_dir = state.data()?.instance_game_dir(instance_id);
    ResourceManager::new(state.modrinth()?.clone(), target, &game_dir, kind, world)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_curseforge_pages_are_opened() {
        assert!(is_curseforge_page(
            "https://www.curseforge.com/minecraft/mc-mods/jei/files/9352806"
        ));
        for bad in [
            "http://www.curseforge.com/minecraft",
            "https://www.curseforge.com.evil.example/x",
            "https://evil.example/https://www.curseforge.com/",
            "https://www.curseforge.com/a&calc",
            "https://www.curseforge.com/a b",
            "file:///C:/Windows",
        ] {
            assert!(!is_curseforge_page(bad), "{bad}");
        }
    }
}
