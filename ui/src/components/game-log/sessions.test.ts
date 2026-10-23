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

import { useGameSessions } from "./sessions";

function emit(name: string, payload: unknown) {
  listeners.get(name)?.({ payload });
}

const target = { id: "i1", name: "生存" };

describe("game sessions", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    listeners.clear();
    tauriInvoke.mockReset();
    tauriInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "task_list") return { tasks: [] };
      if (cmd === "launch_start") return { task_id: "t1" };
      if (cmd === "launch_kill") return { killed: true };
      return {};
    });
  });

  it("starts via the task center and follows game events", async () => {
    const games = useGameSessions();
    const launched = games.launch(target);
    await vi.waitFor(() => expect(listeners.has("task:finished")).toBe(true));
    await vi.waitFor(() =>
      expect(tauriInvoke).toHaveBeenCalledWith("launch_start", {
        args: { instance_id: "i1", account: { kind: "offline", name: "Player" } },
      }),
    );
    expect(games.sessions.i1.status).toBe("starting");
    emit("task:finished", { task_id: "t1", state: "succeeded" });
    await launched;
    expect(games.sessions.i1.status).toBe("running");

    emit("game:log", { instance_id: "i1", pid: 7, stream: "stdout", lines: ["a", "b"] });
    expect(games.sessions.i1.lines.map((l) => l.text)).toEqual(["a", "b"]);
    expect(games.sessions.i1.pid).toBe(7);

    await games.kill("i1");
    expect(tauriInvoke).toHaveBeenLastCalledWith("launch_kill", { args: { instance_id: "i1" } });
    emit("game:exited", { instance_id: "i1", pid: 7, exit_code: null, crashed: false });
    expect(games.sessions.i1.status).toBe("exited");
  });

  it("reports a failed launch task as an error", async () => {
    const games = useGameSessions();
    const launched = games.launch(target);
    await vi.waitFor(() =>
      expect(tauriInvoke).toHaveBeenCalledWith("launch_start", expect.anything()),
    );
    const error = { code: "JAVA_NOT_FOUND", params: { major: "17" }, detail: null };
    emit("task:finished", { task_id: "t1", state: "failed", error });
    await expect(launched).rejects.toEqual(error);
    expect(games.sessions.i1.status).toBe("failed");
    expect(games.sessions.i1.error).toEqual(error);
  });
});
