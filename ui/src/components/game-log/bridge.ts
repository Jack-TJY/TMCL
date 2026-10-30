// 主窗口与独立日志窗口之间的会话同步。游戏会话（含日志）只保存在主窗口的 store 中：
// 日志窗口打开后发出 `ready`，主窗口回复会话快照，此后只推送状态变化和新增的日志行。
import { isTauri } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow, WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ref, watch, type Ref } from "vue";
import { MAX_LOG_LINES, useGameSessions, type GameSession, type LogLine } from "./sessions";

/** 窗口间事件名，只在前端使用。 */
export const LOG_WINDOW_EVENTS = {
  ready: "game-log-window:ready",
  snapshot: "game-log-window:snapshot",
  update: "game-log-window:update",
} as const;

interface Ready {
  instanceId: string;
  label: string;
}

interface Snapshot {
  label: string;
  session: GameSession | null;
}

/** 会话的增量：最新状态与上次推送后新增的日志行。 */
interface Update {
  label: string;
  meta: Pick<GameSession, "name" | "status" | "pid" | "exitCode" | "error">;
  lines: LogLine[];
}

/** 序号不小于 `seq` 的日志行。日志行序号连续，直接按位置截取。 */
export function linesSince(session: GameSession, seq: number): LogLine[] {
  const count = session.nextSeq - seq;
  return count <= 0 ? [] : session.lines.slice(Math.max(0, session.lines.length - count));
}

/** 把增量合并进日志窗口的会话副本（原地修改），跳过已有的行并截断到上限。 */
export function applyUpdate(session: GameSession, update: Pick<Update, "meta" | "lines">) {
  Object.assign(session, update.meta);
  for (const line of update.lines) {
    if (line.seq < session.nextSeq) continue;
    session.lines.push(line);
    session.nextSeq = line.seq + 1;
  }
  const extra = session.lines.length - MAX_LOG_LINES;
  if (extra > 0) session.lines.splice(0, extra);
}

/** 主窗口：响应日志窗口的请求并持续推送会话变化。在主窗口根组件中调用一次。 */
export function serveLogWindows() {
  if (!isTauri()) return;
  const games = useGameSessions();
  const attached = new Map<string, () => void>();

  function attach({ instanceId, label }: Ready) {
    attached.get(label)?.();
    let current: GameSession | undefined;
    let sent = 0;
    const stop = watch(
      () => {
        const s = games.sessions[instanceId];
        return s ? [s, s.name, s.status, s.pid, s.exitCode, s.error, s.nextSeq] : [];
      },
      () => {
        const s = games.sessions[instanceId];
        // 重新启动会换成新的会话对象，此时发送完整快照。
        if (!s || s !== current) {
          current = s;
          sent = s?.nextSeq ?? 0;
          void emitTo<Snapshot>(label, LOG_WINDOW_EVENTS.snapshot, { label, session: s ?? null });
          return;
        }
        const { name, status, pid, exitCode, error } = s;
        const lines = linesSince(s, sent);
        sent = s.nextSeq;
        void emitTo<Update>(label, LOG_WINDOW_EVENTS.update, {
          label,
          meta: { name, status, pid, exitCode, error },
          lines,
        });
      },
      { immediate: true },
    );
    attached.set(label, stop);
    void WebviewWindow.getByLabel(label).then((w) =>
      w?.once("tauri://destroyed", () => {
        if (attached.get(label) === stop) attached.delete(label);
        stop();
      }),
    );
  }

  void listen<Ready>(LOG_WINDOW_EVENTS.ready, (event) => attach(event.payload));
}

/** 日志窗口：向主窗口请求指定实例的会话，返回随主窗口同步的只读副本。 */
export function useMirroredSession(instanceId: string): Ref<GameSession | null> {
  const session = ref<GameSession | null>(null);
  if (!isTauri()) return session;
  const current = getCurrentWebviewWindow();
  const label = current.label;
  void Promise.all([
    current.listen<Snapshot>(LOG_WINDOW_EVENTS.snapshot, ({ payload }) => {
      if (payload.label === label) session.value = payload.session;
    }),
    current.listen<Update>(LOG_WINDOW_EVENTS.update, ({ payload }) => {
      if (payload.label === label && session.value) applyUpdate(session.value, payload);
    }),
  ]).then(() => emitTo<Ready>("main", LOG_WINDOW_EVENTS.ready, { instanceId, label }));
  return session;
}
