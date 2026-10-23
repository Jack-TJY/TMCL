import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import { layoutGet, layoutSave } from "./layout";

describe("layout commands", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    tauriInvoke.mockResolvedValue({ widgets: null, skipped: 0, backup: null });
  });

  it("invokes layout commands", async () => {
    expect((await layoutGet()).widgets).toBeNull();
    expect(tauriInvoke).toHaveBeenLastCalledWith("layout_get", undefined);
    const widgets = [{ uid: "a", type: "news", x: 0, y: 0, w: 4, h: 3, settings: {} }];
    await layoutSave({ widgets });
    expect(tauriInvoke).toHaveBeenLastCalledWith("layout_save", { args: { widgets } });
  });
});
