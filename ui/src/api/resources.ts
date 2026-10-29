// `resources_*` 命令封装（PLAN.md M2-03）。
import type { ResourceImportResult } from "../bindings/ResourceImportResult";
import type { ResourcesImportArgs } from "../bindings/ResourcesImportArgs";
import type { ResourcesModpackExportArgs } from "../bindings/ResourcesModpackExportArgs";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** 任务种类名，与后端 `RESOURCES_*_KIND` 一致。 */
export const RESOURCES_TASK_KINDS = {
  modpackExport: "resources_modpack_export",
} as const;

export type { ResourceImportResult };
export type { ResourceImportSkipReason } from "../bindings/ResourceImportSkipReason";

/** 把本地文件复制进实例的资源目录；同名、非 `.jar` / `.zip` 或不存在的文件被跳过。 */
export function resourcesImport(args: ResourcesImportArgs): Promise<ResourceImportResult> {
  return invoke<ResourceImportResult>("resources_import", { args });
}

/** 把实例导出为 `.mrpack`（`dest` 为绝对路径），立即返回任务 ID。 */
export function resourcesModpackExport(args: ResourcesModpackExportArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("resources_modpack_export", { args });
}
