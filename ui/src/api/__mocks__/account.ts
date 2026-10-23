// 账号管理尚未完成（M1-08），按 AGENTS.md §6.3 先用替身：启动时使用一个离线账号（仅限本地测试）。
// M1-08 提供账号命令后改为读取默认账号，并删除本文件。
import type { LaunchAccountArgs } from "../../bindings/LaunchAccountArgs";

const STORAGE_KEY = "tmcl.mock.offlineName";
const DEFAULT_NAME = "Player";

/** 启动用的离线账号：用户名取自 localStorage（便于调试时修改），默认 `Player`。 */
export function mockLaunchAccount(): LaunchAccountArgs {
  let name = DEFAULT_NAME;
  try {
    name = localStorage.getItem(STORAGE_KEY) || DEFAULT_NAME;
  } catch {
    // 无法访问 localStorage 时使用默认值。
  }
  return { kind: "offline", name };
}
