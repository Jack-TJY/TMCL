import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke, isTauri: () => false }));

import { DEFAULT_ACCENT } from "../theme/fluent";
import { DEFAULT_SETTINGS, useSettingsStore } from "./settings";

const { download, ...backendDefaults } = DEFAULT_SETTINGS;
/** 后端返回的设置；`download.source` 无效时界面沿用前端暂存的下载源。 */
const BACKEND = {
  ...backendDefaults,
  download: { max_concurrency: download.max_concurrency, source: "" },
};

/** 测试运行在 Node 环境，没有 localStorage。 */
function memoryStorage() {
  const items = new Map<string, string>();
  return {
    getItem: (key: string) => items.get(key) ?? null,
    setItem: (key: string, value: string) => void items.set(key, value),
  };
}

describe("settings store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    tauriInvoke.mockReset();
    vi.stubGlobal("localStorage", memoryStorage());
  });

  it("follows the system theme by default", () => {
    const store = useSettingsStore();
    expect(store.settings.theme).toBe("system");
    // 测试环境没有 matchMedia，视为亮色系统。
    expect(store.isDark).toBe(false);
    expect(store.accent).toBe(DEFAULT_ACCENT);
  });

  it("applies saved settings and the data directory", async () => {
    tauriInvoke.mockResolvedValue({
      settings: {
        ...BACKEND,
        theme: "dark",
        appearance: { accent: "#FF0000", window_material: false },
      },
      data_dir: "D:/TMCL/tmcl-data",
      config_backup: null,
    });
    const store = useSettingsStore();
    await store.load();
    expect(tauriInvoke).toHaveBeenCalledWith("settings_get", undefined);
    expect(store.loaded).toBe(true);
    expect(store.isDark).toBe(true);
    expect(store.accent).toBe("#FF0000");
    expect(store.dataDir).toBe("D:/TMCL/tmcl-data");
    expect(store.settings.download.source).toBe("auto");
  });

  it("prefers the download source saved by the backend", async () => {
    tauriInvoke.mockResolvedValue({
      settings: { ...BACKEND, download: { max_concurrency: 16, source: "bmclapi" } },
      data_dir: "",
      config_backup: null,
    });
    const store = useSettingsStore();
    await store.load();
    expect(store.settings.download.source).toBe("bmclapi");
  });

  it("keeps defaults without a backend", async () => {
    tauriInvoke.mockRejectedValue("no backend");
    const store = useSettingsStore();
    await store.load();
    expect(store.loaded).toBe(true);
    expect(store.settings).toEqual(DEFAULT_SETTINGS);
  });

  it("saves changes through settings_set and keeps the source locally", async () => {
    tauriInvoke.mockImplementation(async (_cmd, args) => {
      const settings = args.args.settings;
      return { ...settings, download: { max_concurrency: settings.download.max_concurrency } };
    });
    const store = useSettingsStore();
    await store.update((s) => {
      s.download.max_concurrency = 8;
      s.download.source = "official";
    });
    expect(tauriInvoke).toHaveBeenCalledWith("settings_set", {
      args: {
        settings: expect.objectContaining({ download: { source: "official", max_concurrency: 8 } }),
      },
    });
    expect(store.settings.download).toEqual({ source: "official", max_concurrency: 8 });

    setActivePinia(createPinia());
    tauriInvoke.mockResolvedValue({ settings: BACKEND, data_dir: "", config_backup: null });
    const reloaded = useSettingsStore();
    await reloaded.load();
    expect(reloaded.settings.download.source).toBe("official");
  });

  it("rolls back and rejects with an AppError when saving fails", async () => {
    tauriInvoke.mockRejectedValue(new Error("disk full"));
    const store = useSettingsStore();
    await expect(store.update((s) => (s.theme = "light"))).rejects.toMatchObject({
      code: "COMMON_INTERNAL",
      detail: "disk full",
    });
    expect(store.settings.theme).toBe("system");
  });
});
