import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn() }));
vi.mock("../api/__mocks__/settings", async (importOriginal) => ({
  ...(await importOriginal<object>()),
  mockSettingsGet: mocks.get,
  mockSettingsSave: mocks.save,
}));

import { DEFAULT_SETTINGS } from "../api/__mocks__/settings";
import { DEFAULT_ACCENT } from "../theme/fluent";
import { useSettingsStore } from "./settings";

describe("settings store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mocks.get.mockReset();
    mocks.save.mockReset();
  });

  it("follows the system theme by default", () => {
    const store = useSettingsStore();
    expect(store.settings.theme).toBe("system");
    // 测试环境没有 matchMedia，视为亮色系统。
    expect(store.isDark).toBe(false);
    expect(store.accent).toBe(DEFAULT_ACCENT);
  });

  it("applies saved settings", async () => {
    mocks.get.mockResolvedValue({ ...DEFAULT_SETTINGS, theme: "dark", accent: "#FF0000" });
    const store = useSettingsStore();
    await store.load();
    expect(store.loaded).toBe(true);
    expect(store.isDark).toBe(true);
    expect(store.accent).toBe("#FF0000");
  });

  it("saves changes", async () => {
    mocks.save.mockResolvedValue(undefined);
    const store = useSettingsStore();
    await store.update((s) => (s.download.max_concurrency = 8));
    expect(store.settings.download.max_concurrency).toBe(8);
    expect(mocks.save).toHaveBeenCalledWith(
      expect.objectContaining({ download: { source: "auto", max_concurrency: 8 } }),
    );
  });

  it("rolls back and rejects with an AppError when saving fails", async () => {
    mocks.save.mockRejectedValue(new Error("disk full"));
    const store = useSettingsStore();
    await expect(store.update((s) => (s.theme = "light"))).rejects.toMatchObject({
      code: "COMMON_INTERNAL",
      detail: "disk full",
    });
    expect(store.settings.theme).toBe("system");
  });
});
