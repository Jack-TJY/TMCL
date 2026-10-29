// `sync_*` 命令封装（PLAN.md M2-09）。
import type { SyncApplyResult } from "../bindings/SyncApplyResult";
import type { SyncCaptureArgs } from "../bindings/SyncCaptureArgs";
import type { SyncMaster } from "../bindings/SyncMaster";
import type { SyncPreview } from "../bindings/SyncPreview";
import type { SyncProfile } from "../bindings/SyncProfile";
import type { SyncRestoreArgs } from "../bindings/SyncRestoreArgs";
import type { SyncRestoreResult } from "../bindings/SyncRestoreResult";
import type { SyncSaveMasterArgs } from "../bindings/SyncSaveMasterArgs";
import type { SyncTargetsArgs } from "../bindings/SyncTargetsArgs";
import { invoke } from "./invoke";

/** 读取主配置；从未保存时 `profile` 为 `null`。 */
export function syncGetMaster(): Promise<SyncMaster> {
  return invoke<SyncMaster>("sync_get_master");
}

/** 从实例的 `options.txt` 采集并覆盖主配置。 */
export function syncCapture(args: SyncCaptureArgs): Promise<SyncProfile> {
  return invoke<SyncProfile>("sync_capture", { args });
}

/** 用手动编辑的选项整体替换主配置。 */
export function syncSaveMaster(args: SyncSaveMasterArgs): Promise<SyncProfile> {
  return invoke<SyncProfile>("sync_save_master", { args });
}

/** 预览把主配置应用到各实例时的修改，不写文件。 */
export function syncPreview(args: SyncTargetsArgs): Promise<SyncPreview> {
  return invoke<SyncPreview>("sync_preview", { args });
}

/** 把主配置中选中的类别写入各实例，修改前自动备份。 */
export function syncApply(args: SyncTargetsArgs): Promise<SyncApplyResult> {
  return invoke<SyncApplyResult>("sync_apply", { args });
}

/** 用最近一份备份恢复实例的 `options.txt`。 */
export function syncRestore(args: SyncRestoreArgs): Promise<SyncRestoreResult> {
  return invoke<SyncRestoreResult>("sync_restore", { args });
}
