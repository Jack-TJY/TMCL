// 启动游戏尚无后端（M1-07 启动器未完成），按 AGENTS.md §6.3 先用替身。M1-07 与命令层提供
// `launch_*` 命令和 `game:log` / `game:exited` 事件（PLAN.md 第 3.3 节）后，新建 `api/launch.ts`
// 封装真实命令与事件、改用 `bindings/` 中的类型，并删除本文件。
// 开发模式（`pnpm dev`）下模拟一次游戏运行，便于调试日志窗口；正式构建中启动直接失败。
import type { AppError } from "../../bindings/AppError";

/** `game:log` 事件负载（PLAN.md 第 3.3 节）。 */
export interface GameLog {
  instance_id: string;
  pid: number;
  stream: "stdout" | "stderr";
  lines: string[];
}

/** `game:exited` 事件负载（PLAN.md 第 3.3 节）；被强制结束时 `exit_code` 可能为 `null`。 */
export interface GameExited {
  instance_id: string;
  pid: number;
  exit_code: number | null;
  crashed: boolean;
}

type Handler<T> = (payload: T) => void;

const logHandlers = new Set<Handler<GameLog>>();
const exitHandlers = new Set<Handler<GameExited>>();
const running = new Map<string, { pid: number; timer: ReturnType<typeof setInterval> }>();
let nextPid = 4000;

/** 订阅游戏日志，返回取消订阅函数。 */
export function mockOnGameLog(handler: Handler<GameLog>): () => void {
  logHandlers.add(handler);
  return () => logHandlers.delete(handler);
}

/** 订阅游戏退出，返回取消订阅函数。 */
export function mockOnGameExited(handler: Handler<GameExited>): () => void {
  exitHandlers.add(handler);
  return () => exitHandlers.delete(handler);
}

function exit(instanceId: string, exitCode: number | null, crashed: boolean) {
  const game = running.get(instanceId);
  if (!game) return;
  clearInterval(game.timer);
  running.delete(instanceId);
  for (const h of exitHandlers) {
    h({ instance_id: instanceId, pid: game.pid, exit_code: exitCode, crashed });
  }
}

/** 模拟的日志：JVM 参数含 `-Dtmcl.mock.crash=true` 时模拟崩溃。 */
function script(crash: boolean): [GameLog["stream"], string][] {
  const lines: [GameLog["stream"], string][] = [
    ["stdout", "[main/INFO]: Loading Minecraft with mock launcher"],
    ["stdout", "[main/INFO]: Setting user: Player"],
    ["stdout", "[Render thread/INFO]: Backend library: LWJGL version 3.3.1"],
    ["stderr", "[Render thread/WARN]: Missing sound for event: minecraft:mock"],
    ["stdout", "[Render thread/INFO]: Reloading ResourceManager: vanilla"],
    ["stdout", "[Render thread/INFO]: Sound engine started"],
  ];
  if (crash) {
    lines.push(
      ["stderr", "[Render thread/ERROR]: Reported exception thrown!"],
      ["stderr", "java.lang.IllegalStateException: mock crash"],
      ["stderr", "\tat net.minecraft.client.Minecraft.run(Minecraft.java:0)"],
    );
  }
  return lines;
}

/**
 * 启动实例。开发模式下每 300 ms 推送一行日志，日志结束后保持运行直到 `mockLaunchKill`；
 * 模拟崩溃时以退出码 1 结束。实例已在运行时直接返回。
 */
export async function mockLaunchStart(args: {
  instance_id: string;
  jvm_args: string[];
}): Promise<void> {
  if (!import.meta.env.DEV) {
    const error: AppError = {
      code: "COMMON_INTERNAL",
      params: {},
      detail: "launch: launcher is not implemented yet (M1-07)",
    };
    throw error;
  }
  if (running.has(args.instance_id)) return;
  const crash = args.jvm_args.includes("-Dtmcl.mock.crash=true");
  const lines = script(crash);
  const pid = nextPid++;
  const timer = setInterval(() => {
    const next = lines.shift();
    if (next) {
      const [stream, line] = next;
      for (const h of logHandlers) h({ instance_id: args.instance_id, pid, stream, lines: [line] });
    } else if (crash) {
      exit(args.instance_id, 1, true);
    }
  }, 300);
  running.set(args.instance_id, { pid, timer });
}

/** 强制结束实例的游戏进程；未在运行时不做任何事。 */
export async function mockLaunchKill(args: { instance_id: string }): Promise<void> {
  exit(args.instance_id, null, false);
}
