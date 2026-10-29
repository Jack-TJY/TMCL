// `resources_*` 命令封装（PLAN.md M2-03）。
import type { ResourceImportResult } from "../bindings/ResourceImportResult";
import type { ResourcesImportArgs } from "../bindings/ResourcesImportArgs";
import { invoke } from "./invoke";

export type { ResourceImportResult };
export type { ResourceImportSkipReason } from "../bindings/ResourceImportSkipReason";

/** 把本地文件复制进实例的资源目录；同名、非 `.jar` / `.zip` 或不存在的文件被跳过。 */
export function resourcesImport(args: ResourcesImportArgs): Promise<ResourceImportResult> {
  return invoke<ResourceImportResult>("resources_import", { args });
}
