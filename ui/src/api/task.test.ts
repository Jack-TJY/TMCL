import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import { gameInstall, gameVersionList } from "./game";
import { taskCancel, taskList } from "./task";

describe("task and game commands", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    tauriInvoke.mockResolvedValue({});
  });

  it("invokes task commands with args", async () => {
    await taskList();
    expect(tauriInvoke).toHaveBeenLastCalledWith("task_list", undefined);
    await taskCancel({ task_id: "t1" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("task_cancel", { args: { task_id: "t1" } });
  });

  it("invokes game commands with args", async () => {
    await gameVersionList();
    expect(tauriInvoke).toHaveBeenLastCalledWith("game_version_list", undefined);
    await gameInstall({ version_id: "1.20.1" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("game_install", {
      args: { version_id: "1.20.1" },
    });
  });
});
