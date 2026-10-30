import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import { DEFAULT_BACKGROUND } from "../theme/background";
import { settingsGet, settingsOpenDataDir, settingsSet } from "./settings";

describe("settings commands", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    tauriInvoke.mockResolvedValue({});
  });

  it("invokes settings commands", async () => {
    await settingsGet();
    expect(tauriInvoke).toHaveBeenLastCalledWith("settings_get", undefined);
    await settingsOpenDataDir();
    expect(tauriInvoke).toHaveBeenLastCalledWith("settings_open_data_dir", undefined);
    const settings = {
      theme: "dark" as const,
      appearance: {
        accent: null,
        window_material: true,
        material: "acrylic" as const,
        material_opacity: 0,
        background: DEFAULT_BACKGROUND,
      },
      memory: { min_mb: 1024, max_mb: 4096 },
      download: { max_concurrency: 16, source: "auto" },
    };
    await settingsSet({ settings });
    expect(tauriInvoke).toHaveBeenLastCalledWith("settings_set", { args: { settings } });
  });
});
