import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler);
    return () => listeners.delete(name);
  }),
}));

import type { AppError } from "../bindings/AppError";
import type { TaskInfo } from "../bindings/TaskInfo";
import type { TaskProgress } from "../bindings/TaskProgress";
import { isStalled, STALL_MS, useTasksStore } from "./tasks";

function emit(name: string, payload: unknown) {
  listeners.get(name)?.({ payload });
}

function progress(taskId: string, bytesDone: number, bytesTotal: number): TaskProgress {
  return {
    task_id: taskId,
    kind: "game_install",
    stage: "assets",
    done: 0,
    total: 0,
    bytes_done: bytesDone,
    bytes_total: bytesTotal,
    speed_bps: 1024,
  };
}

const NETWORK_ERROR: AppError = { code: "NET_TIMEOUT", params: {}, detail: "timed out" };

describe("tasks store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    tauriInvoke.mockReset();
    listeners.clear();
  });

  it("loads existing tasks without a restart action", async () => {
    const existing: TaskInfo = {
      task_id: "old",
      kind: "game_install",
      status: "failed",
      started_at: 1,
      finished_at: 2,
      progress: null,
      error: NETWORK_ERROR,
    };
    tauriInvoke.mockResolvedValueOnce({ tasks: [existing] });
    const store = useTasksStore();
    await store.init();
    expect(store.entries).toHaveLength(1);
    expect(store.entries[0]).toMatchObject({ task_id: "old", label: null, restart: null });
  });

  it("tracks progress and completion of a started task", async () => {
    tauriInvoke.mockResolvedValueOnce({ tasks: [] });
    const store = useTasksStore();
    const start = vi.fn().mockResolvedValue({ task_id: "t1" });
    await store.start({ kind: "game_install", label: "1.20.1", start });

    emit("task:progress", progress("t1", 50, 100));
    expect(store.running).toHaveLength(1);
    expect(store.overallProgress).toBe(0.5);

    emit("task:finished", { task_id: "t1", state: "failed", error: NETWORK_ERROR });
    expect(store.entries[0]).toMatchObject({ status: "failed", error: NETWORK_ERROR });
    expect(store.overallProgress).toBeNull();
  });

  it("applies a finish event that arrives before start returns", async () => {
    tauriInvoke.mockResolvedValueOnce({ tasks: [] });
    const store = useTasksStore();
    await store.init();
    const start = vi.fn(async () => {
      emit("task:progress", progress("t2", 1, 1));
      emit("task:finished", { task_id: "t3", state: "succeeded" });
      return { task_id: "t3" };
    });
    await store.start({ kind: "game_install", label: "a", start });
    expect(store.entries.find((e) => e.task_id === "t3")?.status).toBe("succeeded");
  });

  it("retries a failed task with the same request", async () => {
    tauriInvoke.mockResolvedValueOnce({ tasks: [] });
    const store = useTasksStore();
    const start = vi
      .fn()
      .mockResolvedValueOnce({ task_id: "a" })
      .mockResolvedValueOnce({ task_id: "b" });
    await store.start({ kind: "game_install", label: "1.20.1", start });
    emit("task:finished", { task_id: "a", state: "failed", error: NETWORK_ERROR });

    expect(await store.retry("a")).toBe("b");
    expect(start).toHaveBeenCalledTimes(2);
    expect(store.entries.map((e) => e.task_id)).toEqual(["b"]);
    expect(store.entries[0]).toMatchObject({ label: "1.20.1", status: "running" });
  });

  it("marks an instance as installing while its task runs", async () => {
    tauriInvoke.mockResolvedValueOnce({ tasks: [] });
    const store = useTasksStore();
    const start = vi
      .fn()
      .mockResolvedValueOnce({ task_id: "a" })
      .mockResolvedValueOnce({ task_id: "b" });
    await store.start({ kind: "loader_install", label: "x", start, instanceId: "inst" });
    expect(store.installing("inst")).toBe(true);
    expect(store.installing("other")).toBe(false);

    emit("task:finished", { task_id: "a", state: "failed", error: NETWORK_ERROR });
    expect(store.installing("inst")).toBe(false);

    await store.retry("a");
    expect(store.installing("inst")).toBe(true);
    emit("task:finished", { task_id: "b", state: "succeeded" });
    expect(store.installing("inst")).toBe(false);
  });

  it("detects a running task whose progress stopped advancing", async () => {
    vi.useFakeTimers();
    try {
      tauriInvoke.mockResolvedValueOnce({ tasks: [] });
      const store = useTasksStore();
      await store.start({
        kind: "game_install",
        label: "x",
        start: async () => ({ task_id: "s" }),
      });
      const entry = () => store.entries[0];
      // 还没有进度时不算停滞（仍在准备）。
      expect(isStalled(entry(), Date.now() + STALL_MS)).toBe(false);

      emit("task:progress", progress("s", 10, 100));
      vi.advanceTimersByTime(STALL_MS - 1);
      expect(isStalled(entry(), Date.now())).toBe(false);
      // 只有速度变化、字节数不变的进度事件不算前进。
      emit("task:progress", { ...progress("s", 10, 100), speed_bps: 0 });
      vi.advanceTimersByTime(1);
      expect(isStalled(entry(), Date.now())).toBe(true);

      emit("task:progress", progress("s", 20, 100));
      expect(isStalled(entry(), Date.now())).toBe(false);

      emit("task:finished", { task_id: "s", state: "failed", error: NETWORK_ERROR });
      expect(isStalled(entry(), Date.now() + STALL_MS)).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not retry running tasks", async () => {
    tauriInvoke.mockResolvedValueOnce({ tasks: [] });
    const store = useTasksStore();
    const start = vi.fn().mockResolvedValue({ task_id: "a" });
    await store.start({ kind: "game_install", label: "x", start });
    expect(await store.retry("a")).toBeNull();
  });

  it("rejects with an AppError when the command fails", async () => {
    tauriInvoke.mockResolvedValueOnce({ tasks: [] });
    const store = useTasksStore();
    const start = vi.fn().mockRejectedValue(new Error("boom"));
    await expect(store.start({ kind: "k", label: null, start })).rejects.toMatchObject({
      code: "COMMON_INTERNAL",
    });
    expect(store.entries).toHaveLength(0);
  });

  it("clears finished tasks but keeps running ones", async () => {
    tauriInvoke.mockResolvedValueOnce({ tasks: [] });
    const store = useTasksStore();
    await store.start({ kind: "k", label: null, start: async () => ({ task_id: "a" }) });
    await store.start({ kind: "k", label: null, start: async () => ({ task_id: "b" }) });
    emit("task:finished", { task_id: "a", state: "cancelled" });
    store.clearFinished();
    expect(store.entries.map((e) => e.task_id)).toEqual(["b"]);
  });
});
