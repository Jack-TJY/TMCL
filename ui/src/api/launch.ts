// `launch_*` 命令与 `game:log` / `game:exited` 事件封装。
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { GameExited } from "../bindings/GameExited";
import type { GameLog } from "../bindings/GameLog";
import type { LaunchInstanceArgs } from "../bindings/LaunchInstanceArgs";
import type { LaunchKillResult } from "../bindings/LaunchKillResult";
import type { LaunchRunning } from "../bindings/LaunchRunning";
import type { LaunchStartArgs } from "../bindings/LaunchStartArgs";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** 启动任务的种类名，与后端 `LAUNCH_KIND` 一致。 */
export const LAUNCH_TASK_KIND = "launch";

/** 事件名，与 `src-tauri/src/events.rs` 一致。 */
export const GAME_EVENTS = {
  log: "game:log",
  exited: "game:exited",
} as const;

/** 启动实例：补全文件、选择 Java 后创建游戏进程，任务成功即游戏已在运行。 */
export function launchStart(args: LaunchStartArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("launch_start", { args });
}

/** 强制结束实例的游戏进程；未在运行时 `killed` 为 `false`。 */
export function launchKill(args: LaunchInstanceArgs): Promise<LaunchKillResult> {
  return invoke<LaunchKillResult>("launch_kill", { args });
}

/** 运行中的游戏。 */
export function launchRunning(): Promise<LaunchRunning> {
  return invoke<LaunchRunning>("launch_running");
}

/** 订阅游戏日志，返回取消订阅函数。 */
export function onGameLog(handler: (log: GameLog) => void): Promise<UnlistenFn> {
  return listen<GameLog>(GAME_EVENTS.log, (event) => handler(event.payload));
}

/** 订阅游戏退出，返回取消订阅函数。 */
export function onGameExited(handler: (exited: GameExited) => void): Promise<UnlistenFn> {
  return listen<GameExited>(GAME_EVENTS.exited, (event) => handler(event.payload));
}
