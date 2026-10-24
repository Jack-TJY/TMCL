// 后端事件订阅（PLAN.md 第 3.3 节）。启动长耗时任务前先订阅，避免漏掉首条进度。
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DeviceCode } from "../bindings/DeviceCode";
import type { TaskFinished } from "../bindings/TaskFinished";
import type { TaskProgress } from "../bindings/TaskProgress";

/** 事件名，与 `src-tauri/src/events.rs` 一致。 */
export const EVENTS = {
  taskProgress: "task:progress",
  taskFinished: "task:finished",
  authDeviceCode: "auth:device-code",
} as const;

/** 订阅任务进度，返回取消订阅函数。 */
export function onTaskProgress(handler: (progress: TaskProgress) => void): Promise<UnlistenFn> {
  return listen<TaskProgress>(EVENTS.taskProgress, (event) => handler(event.payload));
}

/** 订阅任务结束，返回取消订阅函数。 */
export function onTaskFinished(handler: (finished: TaskFinished) => void): Promise<UnlistenFn> {
  return listen<TaskFinished>(EVENTS.taskFinished, (event) => handler(event.payload));
}

/** 订阅微软登录的设备码，返回取消订阅函数。 */
export function onDeviceCode(handler: (code: DeviceCode) => void): Promise<UnlistenFn> {
  return listen<DeviceCode>(EVENTS.authDeviceCode, (event) => handler(event.payload));
}
