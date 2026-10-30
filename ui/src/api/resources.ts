// `resources_*` 命令封装（PLAN.md M2-03、M2-04、M2-06）。
import type { DetectedPack } from "../bindings/DetectedPack";
import type { InstallPlan } from "../bindings/InstallPlan";
import type { InstalledList } from "../bindings/InstalledList";
import type { ModpackInfo } from "../bindings/ModpackInfo";
import type { PackKind } from "../bindings/PackKind";
import type { ResourceImportResult } from "../bindings/ResourceImportResult";
import type { ResourcePage } from "../bindings/ResourcePage";
import type { ResourceUpdate } from "../bindings/ResourceUpdate";
import type { ResourceUpdateList } from "../bindings/ResourceUpdateList";
import type { ResourcesDirArgs } from "../bindings/ResourcesDirArgs";
import type { ResourcesImportArgs } from "../bindings/ResourcesImportArgs";
import type { ResourcesInstallArgs } from "../bindings/ResourcesInstallArgs";
import type { ResourcesModpackExportArgs } from "../bindings/ResourcesModpackExportArgs";
import type { ResourcesModpackInspectArgs } from "../bindings/ResourcesModpackInspectArgs";
import type { ResourcesModpackInstallArgs } from "../bindings/ResourcesModpackInstallArgs";
import type { ResourcesModpackSearchArgs } from "../bindings/ResourcesModpackSearchArgs";
import type { ResourcesModpackVersionsArgs } from "../bindings/ResourcesModpackVersionsArgs";
import type { ResourcesOpenPageArgs } from "../bindings/ResourcesOpenPageArgs";
import type { ResourcesPackDetectArgs } from "../bindings/ResourcesPackDetectArgs";
import type { ResourcesRemoveArgs } from "../bindings/ResourcesRemoveArgs";
import type { ResourcesSearchAnyArgs } from "../bindings/ResourcesSearchAnyArgs";
import type { ResourcesSearchArgs } from "../bindings/ResourcesSearchArgs";
import type { ResourcesSetEnabledArgs } from "../bindings/ResourcesSetEnabledArgs";
import type { ResourcesSetEnabledResult } from "../bindings/ResourcesSetEnabledResult";
import type { ResourcesUpdateArgs } from "../bindings/ResourcesUpdateArgs";
import type { ResourcesVersionsArgs } from "../bindings/ResourcesVersionsArgs";
import type { ResourcesWorldsArgs } from "../bindings/ResourcesWorldsArgs";
import type { ResourceSources } from "../bindings/ResourceSources";
import type { ResourceVersion } from "../bindings/ResourceVersion";
import type { ResourceVersionList } from "../bindings/ResourceVersionList";
import type { ResourceWorldList } from "../bindings/ResourceWorldList";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** 任务种类名，与后端 `RESOURCES_*_KIND` 一致。 */
export const RESOURCES_TASK_KINDS = {
  modpackExport: "resources_modpack_export",
} as const;

export type {
  InstallPlan,
  InstalledList,
  ModpackInfo,
  ResourceImportResult,
  ResourcePage,
  ResourceUpdate,
  ResourceVersion,
};
export type { ManualDownload } from "../bindings/ManualDownload";
export type { ModpackSource } from "../bindings/ModpackSource";
export type { PackKind } from "../bindings/PackKind";
export type { Conflict } from "../bindings/Conflict";
export type { InstalledResource } from "../bindings/InstalledResource";
export type { PlannedFile } from "../bindings/PlannedFile";
export type { ResourceChannel } from "../bindings/ResourceChannel";
export type { ResourceHit } from "../bindings/ResourceHit";
export type { ResourceImportSkipReason } from "../bindings/ResourceImportSkipReason";
export type { ResourceKind } from "../bindings/ResourceKind";
export type { ResourcePlatform } from "../bindings/ResourcePlatform";
export type { ResourceSort } from "../bindings/ResourceSort";
export type { UnavailableDependency } from "../bindings/UnavailableDependency";

/** 资源安装任务的种类名，与后端 `RESOURCES_INSTALL_KIND` 一致。 */
export const RESOURCES_INSTALL_KIND = "resources_install";
/** 资源更新任务的种类名，与后端 `RESOURCES_UPDATE_KIND` 一致。 */
export const RESOURCES_UPDATE_KIND = "resources_update";
/** 整合包安装任务的种类名，与后端 `RESOURCES_MODPACK_INSTALL_KIND` 一致。 */
export const RESOURCES_MODPACK_INSTALL_KIND = "resources_modpack_install";

/** 按实例的游戏版本与加载器搜索 Modrinth。 */
export function resourcesSearch(args: ResourcesSearchArgs): Promise<ResourcePage> {
  return invoke<ResourcePage>("resources_search", { args });
}

/** 不按实例过滤，搜索全部资源。 */
export function resourcesSearchAny(args: ResourcesSearchAnyArgs): Promise<ResourcePage> {
  return invoke<ResourcePage>("resources_search_any", { args });
}

/** 项目适用于该实例的版本，新版本在前。 */
export async function resourcesVersions(args: ResourcesVersionsArgs): Promise<ResourceVersion[]> {
  return (await invoke<ResourceVersionList>("resources_versions", { args })).versions;
}

/** 生成安装计划：要下载的文件（含前置）、冲突与找不到的前置。 */
export function resourcesInstallPlan(args: ResourcesInstallArgs): Promise<InstallPlan> {
  return invoke<InstallPlan>("resources_install_plan", { args });
}

/** 按计划安装，返回任务 ID。 */
export function resourcesInstall(args: ResourcesInstallArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("resources_install", { args });
}

/** 实例中某一类已安装的资源。 */
export function resourcesInstalled(args: ResourcesDirArgs): Promise<InstalledList> {
  return invoke<InstalledList>("resources_installed", { args });
}

/** 把本地文件复制进实例的资源目录；同名、非 `.jar` / `.zip` 或不存在的文件被跳过。 */
export function resourcesImport(args: ResourcesImportArgs): Promise<ResourceImportResult> {
  return invoke<ResourceImportResult>("resources_import", { args });
}

/** 把实例导出为 `.mrpack`（`dest` 为绝对路径），立即返回任务 ID。 */
export function resourcesModpackExport(args: ResourcesModpackExportArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("resources_modpack_export", { args });
}

/** 启用或禁用，返回改名后的文件名。 */
export async function resourcesSetEnabled(args: ResourcesSetEnabledArgs): Promise<string> {
  return (await invoke<ResourcesSetEnabledResult>("resources_set_enabled", { args })).file_name;
}

/** 删除一个已安装的资源文件。 */
export function resourcesRemove(args: ResourcesRemoveArgs): Promise<void> {
  return invoke<void>("resources_remove", { args });
}

/** 实例 `saves/` 下的存档目录名，用于安装数据包。 */
export async function resourcesWorlds(args: ResourcesWorldsArgs): Promise<string[]> {
  return (await invoke<ResourceWorldList>("resources_worlds", { args })).worlds;
}

/** 检查某一类已安装资源在 Modrinth 上是否有更新，按文件名排序。 */
export async function resourcesCheckUpdates(args: ResourcesDirArgs): Promise<ResourceUpdate[]> {
  return (await invoke<ResourceUpdateList>("resources_check_updates", { args })).updates;
}

/** 更新指定文件（省略 `file_names` 时更新全部可更新的资源），返回任务 ID。 */
export function resourcesUpdate(args: ResourcesUpdateArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("resources_update", { args });
}

/** 本次构建可用的资源平台；没有 CurseForge Key 时 `curseforge` 为 `false`，界面应隐藏。 */
export function resourcesSources(): Promise<ResourceSources> {
  return invoke<ResourceSources>("resources_sources");
}

/** 在系统浏览器中打开 CurseForge 网页（手动下载禁止第三方分发的文件）。 */
export function resourcesOpenPage(args: ResourcesOpenPageArgs): Promise<void> {
  return invoke<void>("resources_open_page", { args });
}

/** 在 Modrinth 或 CurseForge 搜索整合包。 */
export function resourcesModpackSearch(args: ResourcesModpackSearchArgs): Promise<ResourcePage> {
  return invoke<ResourcePage>("resources_modpack_search", { args });
}

/** 整合包项目的版本，新版本在前。 */
export async function resourcesModpackVersions(
  args: ResourcesModpackVersionsArgs,
): Promise<ResourceVersion[]> {
  return (await invoke<ResourceVersionList>("resources_modpack_versions", { args })).versions;
}

/** 读取并校验本地整合包（`.mrpack` 或 CurseForge zip），返回概要。 */
export function resourcesModpackInspect(args: ResourcesModpackInspectArgs): Promise<ModpackInfo> {
  return invoke<ModpackInfo>("resources_modpack_inspect", { args });
}

/** 识别本地压缩包类型：整合包、TMCL 实例包或其他启动器的实例。 */
export async function resourcesPackDetect(args: ResourcesPackDetectArgs): Promise<PackKind> {
  return (await invoke<DetectedPack>("resources_pack_detect", { args })).kind;
}

/** 把整合包安装为新实例，返回任务 ID。 */
export function resourcesModpackInstall(args: ResourcesModpackInstallArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("resources_modpack_install", { args });
}
