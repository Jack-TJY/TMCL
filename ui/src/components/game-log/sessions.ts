// 游戏会话（PLAN.md M1-13）：每个实例最近一次启动的状态与日志，供实例页、日志窗口和
// 首页“开始游戏”组件（M1-18）共用。启动暂由 api/__mocks__/launch.ts 提供。
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import {
  mockLaunchKill,
  mockLaunchStart,
  mockOnGameExited,
  mockOnGameLog,
  type GameExited,
  type GameLog,
} from "../../api/__mocks__/launch";
import { toAppError } from "../../api/invoke";
import type { AppError } from "../../bindings/AppError";

/** 每个会话保留的日志行数上限，超出时丢弃最早的行。 */
export const MAX_LOG_LINES = 5000;

export type SessionStatus = "starting" | "running" | "exited" | "crashed" | "failed";

export interface LogLine {
  /** 会话内递增的序号，用作列表 key。 */
  seq: number;
  stream: GameLog["stream"];
  text: string;
}

export interface GameSession {
  instanceId: string;
  name: string;
  status: SessionStatus;
  pid: number | null;
  exitCode: number | null;
  /** 启动失败时的错误。 */
  error: AppError | null;
  lines: LogLine[];
  nextSeq: number;
}

/** 启动所需的实例信息。 */
export interface LaunchTarget {
  id: string;
  name: string;
  jvm_args: string[];
}

/** 追加日志并截断到上限（原地修改）。 */
export function appendLines(session: GameSession, stream: LogLine["stream"], lines: string[]) {
  for (const text of lines) session.lines.push({ seq: session.nextSeq++, stream, text });
  const extra = session.lines.length - MAX_LOG_LINES;
  if (extra > 0) session.lines.splice(0, extra);
}

/** 会话是否仍占用实例（启动中或运行中）。 */
export function isActive(session: GameSession | undefined): boolean {
  return session?.status === "starting" || session?.status === "running";
}

export const useGameSessions = defineStore("gameSessions", () => {
  const sessions = ref<Record<string, GameSession>>({});
  let crashHandler: ((session: GameSession) => void) | null = null;
  let subscribed = false;

  const active = computed(() => Object.values(sessions.value).filter((s) => isActive(s)));

  function onLog(log: GameLog) {
    const session = sessions.value[log.instance_id];
    if (!session || !isActive(session)) return;
    session.status = "running";
    session.pid = log.pid;
    appendLines(session, log.stream, log.lines);
  }

  function onExited(exited: GameExited) {
    const session = sessions.value[exited.instance_id];
    if (!session || !isActive(session)) return;
    session.status = exited.crashed ? "crashed" : "exited";
    session.exitCode = exited.exit_code;
    if (exited.crashed) crashHandler?.(session);
  }

  function subscribe() {
    if (subscribed) return;
    subscribed = true;
    mockOnGameLog(onLog);
    mockOnGameExited(onExited);
  }

  /** 设置崩溃时的回调（例如弹出提示），替换之前设置的回调。 */
  function setCrashHandler(handler: (session: GameSession) => void) {
    crashHandler = handler;
  }

  /** 启动实例；已在运行时不重复启动。失败时会话状态为 `failed` 并抛出 AppError。 */
  async function launch(target: LaunchTarget): Promise<void> {
    subscribe();
    if (isActive(sessions.value[target.id])) return;
    sessions.value[target.id] = {
      instanceId: target.id,
      name: target.name,
      status: "starting",
      pid: null,
      exitCode: null,
      error: null,
      lines: [],
      nextSeq: 0,
    };
    try {
      await mockLaunchStart({ instance_id: target.id, jvm_args: target.jvm_args });
    } catch (err) {
      const error = toAppError(err);
      const session = sessions.value[target.id];
      session.status = "failed";
      session.error = error;
      throw error;
    }
  }

  /** 强制结束游戏进程。 */
  async function kill(instanceId: string): Promise<void> {
    await mockLaunchKill({ instance_id: instanceId });
  }

  return { sessions, active, launch, kill, setCrashHandler };
});
