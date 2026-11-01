// 新建实例后继续安装资源（“一键新建实例”）：取默认版本，计划无冲突、无缺失前置时直接发起
// resources_install 任务，否则交给调用方打开安装对话框让用户处理。
import { toAppError } from "../../api/invoke";
import {
  resourcesInstall,
  resourcesInstallPlan,
  resourcesVersions,
  RESOURCES_INSTALL_KIND,
  type ResourceHit,
  type ResourceKind,
} from "../../api/resources";
import type { AppError } from "../../bindings/AppError";
import type { TaskId } from "../../bindings/TaskId";
import type { TaskRequest } from "../../stores/tasks";
import { defaultVersion, planBlocked } from "./resources";

export interface AutoInstallTarget {
  hit: ResourceHit;
  kind: ResourceKind;
  instanceId: string;
  instanceName: string;
}

export type AutoInstallResult =
  | { state: "started"; taskId: TaskId }
  /** 没有适用版本、计划有冲突或缺少前置：需要用户在安装对话框中处理。 */
  | { state: "needs_dialog" }
  | { state: "failed"; error: AppError };

/** 为新实例安装资源；`start` 一般为任务中心的 `tasks.start`。 */
export async function autoInstall(
  target: AutoInstallTarget,
  start: (request: TaskRequest) => Promise<TaskId>,
): Promise<AutoInstallResult> {
  const { hit, kind, instanceId, instanceName } = target;
  try {
    const versions = await resourcesVersions({
      instance_id: instanceId,
      kind,
      project_id: hit.project_id,
      platform: hit.platform,
    });
    const version = defaultVersion(versions);
    if (!version) return { state: "needs_dialog" };
    const args = { instance_id: instanceId, kind, version_id: version.id, platform: hit.platform };
    const plan = await resourcesInstallPlan(args);
    if (planBlocked(plan)) return { state: "needs_dialog" };
    const taskId = await start({
      kind: RESOURCES_INSTALL_KIND,
      label: `${hit.title} → ${instanceName}`,
      start: () => resourcesInstall(args),
      instanceId,
    });
    return { state: "started", taskId };
  } catch (err) {
    return { state: "failed", error: toAppError(err) };
  }
}
