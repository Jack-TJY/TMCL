// `game_*` 命令封装。
import type { GameInstallArgs } from "../bindings/GameInstallArgs";
import type { GameVersionList } from "../bindings/GameVersionList";
import type { GameVersionListArgs } from "../bindings/GameVersionListArgs";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** 游戏文件下载任务的种类名，与后端 `GAME_INSTALL_KIND` 一致。 */
export const GAME_INSTALL_KIND = "game_install";

/**
 * 获取官方版本清单，新版本在前。`refresh` 为真时跳过 1 小时缓存强制联网；
 * 联网失败但有旧缓存时返回旧缓存，并在 `stale` 中给出原因。
 */
export function gameVersionList(args?: GameVersionListArgs): Promise<GameVersionList> {
  return invoke<GameVersionList>("game_version_list", args ? { args } : undefined);
}

/** 下载并校验运行该版本所需的文件，立即返回任务 ID；先订阅任务事件再调用。 */
export function gameInstall(args: GameInstallArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("game_install", { args });
}
