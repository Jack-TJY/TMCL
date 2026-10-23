// `task_*` 命令封装。
import type { TaskCancelArgs } from "../bindings/TaskCancelArgs";
import type { TaskCancelResult } from "../bindings/TaskCancelResult";
import type { TaskList } from "../bindings/TaskList";
import { invoke } from "./invoke";

/** 运行中的任务与最近结束的至多 50 条历史。 */
export function taskList(): Promise<TaskList> {
  return invoke<TaskList>("task_list");
}

/** 请求取消任务；任务不存在或已结束时 `cancelled` 为 `false`。 */
export function taskCancel(args: TaskCancelArgs): Promise<TaskCancelResult> {
  return invoke<TaskCancelResult>("task_cancel", { args });
}
