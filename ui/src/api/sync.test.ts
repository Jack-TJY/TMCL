import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import {
  syncApply,
  syncCapture,
  syncGetAuto,
  syncGetMaster,
  syncPreview,
  syncRestore,
  syncSaveMaster,
  syncSetAuto,
} from "./sync";

describe("sync commands", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    tauriInvoke.mockResolvedValue({});
  });

  it("invokes sync commands with args", async () => {
    await syncGetMaster();
    expect(tauriInvoke).toHaveBeenLastCalledWith("sync_get_master", undefined);
    await syncCapture({ instance_id: "a" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("sync_capture", { args: { instance_id: "a" } });
    const options = [{ key: "fov", value: "0.5" }];
    await syncSaveMaster({ options });
    expect(tauriInvoke).toHaveBeenLastCalledWith("sync_save_master", { args: { options } });
    const targets = { instance_ids: ["a", "b"], categories: ["keys" as const] };
    await syncPreview(targets);
    expect(tauriInvoke).toHaveBeenLastCalledWith("sync_preview", { args: targets });
    await syncApply(targets);
    expect(tauriInvoke).toHaveBeenLastCalledWith("sync_apply", { args: targets });
    await syncGetAuto();
    expect(tauriInvoke).toHaveBeenLastCalledWith("sync_get_auto", undefined);
    const auto = { enabled: true, categories: ["keys" as const] };
    await syncSetAuto(auto);
    expect(tauriInvoke).toHaveBeenLastCalledWith("sync_set_auto", { args: auto });
    await syncRestore({ instance_id: "b" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("sync_restore", { args: { instance_id: "b" } });
  });

  it("rejects with AppError", async () => {
    const err = { code: "SYNC_NO_MASTER", params: {}, detail: null };
    tauriInvoke.mockRejectedValueOnce(err);
    await expect(syncPreview({ instance_ids: [], categories: [] })).rejects.toEqual(err);
  });
});
