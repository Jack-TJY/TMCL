// `app_*` 命令封装。
import type { AppDemoTaskArgs } from "../bindings/AppDemoTaskArgs";
import type { AppInfo } from "../bindings/AppInfo";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** 演示任务的种类名，与后端 `DEMO_TASK_KIND` 一致。 */
export const DEMO_TASK_KIND = "app_demo";

/** 版本号、平台与功能开关。 */
export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

/** 启动模拟进度的演示任务，立即返回任务 ID。 */
export function appDemoTask(args: AppDemoTaskArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("app_demo_task", { args });
}
