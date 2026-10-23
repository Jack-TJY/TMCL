// `instance_*` 命令封装。
import type { InstanceConfig } from "../bindings/InstanceConfig";
import type { InstanceCreateArgs } from "../bindings/InstanceCreateArgs";
import type { InstanceDuplicateArgs } from "../bindings/InstanceDuplicateArgs";
import type { InstanceExportArgs } from "../bindings/InstanceExportArgs";
import type { InstanceGameEntries } from "../bindings/InstanceGameEntries";
import type { InstanceIdArgs } from "../bindings/InstanceIdArgs";
import type { InstanceImportArgs } from "../bindings/InstanceImportArgs";
import type { InstanceList } from "../bindings/InstanceList";
import type { InstanceRenameArgs } from "../bindings/InstanceRenameArgs";
import type { InstanceSaveArgs } from "../bindings/InstanceSaveArgs";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** 任务种类名，与后端 `INSTANCE_*_KIND` 一致。 */
export const INSTANCE_TASK_KINDS = {
  duplicate: "instance_duplicate",
  export: "instance_export",
  import: "instance_import",
} as const;

/** 读取实例列表；损坏的实例带 `error`。 */
export function instanceList(): Promise<InstanceList> {
  return invoke<InstanceList>("instance_list");
}

/** 与磁盘重新对账后读取实例列表。 */
export function instanceRefresh(): Promise<InstanceList> {
  return invoke<InstanceList>("instance_refresh");
}

/** 读取实例配置。 */
export function instanceGet(args: InstanceIdArgs): Promise<InstanceConfig> {
  return invoke<InstanceConfig>("instance_get", { args });
}

/** 新建实例（不下载游戏文件）。 */
export function instanceCreate(args: InstanceCreateArgs): Promise<InstanceConfig> {
  return invoke<InstanceConfig>("instance_create", { args });
}

/** 保存修改后的配置，返回规整后的配置。 */
export function instanceSave(args: InstanceSaveArgs): Promise<InstanceConfig> {
  return invoke<InstanceConfig>("instance_save", { args });
}

/** 重命名实例。 */
export function instanceRename(args: InstanceRenameArgs): Promise<InstanceConfig> {
  return invoke<InstanceConfig>("instance_rename", { args });
}

/** 删除实例（移到回收站，7 天后清理）。 */
export function instanceDelete(args: InstanceIdArgs): Promise<void> {
  return invoke<void>("instance_delete", { args });
}

/** `game/` 下的顶层条目，供导出时选择。 */
export function instanceGameEntries(args: InstanceIdArgs): Promise<InstanceGameEntries> {
  return invoke<InstanceGameEntries>("instance_game_entries", { args });
}

/** 复制实例，立即返回任务 ID；先订阅任务事件再调用。 */
export function instanceDuplicate(args: InstanceDuplicateArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("instance_duplicate", { args });
}

/** 导出为 `.tmclpack`（`dest` 为绝对路径），立即返回任务 ID。 */
export function instanceExport(args: InstanceExportArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("instance_export", { args });
}

/** 从 `.tmclpack`（绝对路径）导入，立即返回任务 ID。 */
export function instanceImport(args: InstanceImportArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("instance_import", { args });
}
