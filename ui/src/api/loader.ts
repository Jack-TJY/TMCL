// `loader_*` 命令封装。
import type { LoaderInstallArgs } from "../bindings/LoaderInstallArgs";
import type { LoaderListArgs } from "../bindings/LoaderListArgs";
import type { LoaderVersionList } from "../bindings/LoaderVersionList";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** 加载器安装任务的种类名，与后端 `LOADER_INSTALL_KIND` 一致。 */
export const LOADER_INSTALL_KIND = "loader_install";

/** 列出加载器在该游戏版本上可安装的版本，新版本在前；为空表示不支持。 */
export function loaderList(args: LoaderListArgs): Promise<LoaderVersionList> {
  return invoke<LoaderVersionList>("loader_list", { args });
}

/** 在实例上安装或更换加载器，立即返回任务 ID；成功后实例的 `loader` 才会改变。 */
export function loaderInstall(args: LoaderInstallArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("loader_install", { args });
}
