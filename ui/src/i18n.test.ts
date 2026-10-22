import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import type { Composer } from "vue-i18n";
import { currentLocale, formatError, i18n, initLocale, resolveLocale, setLocale } from "./i18n";

const composer = i18n.global as unknown as Composer;

describe("resolveLocale", () => {
  it("matches exact tags case-insensitively", () => {
    expect(resolveLocale(["en-us"])).toBe("en-US");
  });

  it("falls back to the primary language", () => {
    expect(resolveLocale(["zh-TW", "en-US"])).toBe("zh-CN");
    expect(resolveLocale(["ja-JP", "en-GB"])).toBe("en-US");
  });

  it("uses en-US for unsupported languages", () => {
    expect(resolveLocale(["ja-JP"])).toBe("en-US");
    expect(resolveLocale([])).toBe("en-US");
  });
});

describe("initLocale", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    composer.locale.value = "en-US";
  });

  it("uses the saved locale", async () => {
    tauriInvoke.mockResolvedValueOnce({ language: "zh-CN" });
    await initLocale(["en-US"]);
    expect(currentLocale()).toBe("zh-CN");
    expect(tauriInvoke).toHaveBeenCalledTimes(1);
  });

  it("picks the system language on first launch and saves it", async () => {
    tauriInvoke.mockResolvedValueOnce({ language: null }).mockResolvedValueOnce(null);
    await initLocale(["zh-Hans-CN"]);
    expect(currentLocale()).toBe("zh-CN");
    expect(tauriInvoke).toHaveBeenLastCalledWith("settings_set_language", {
      args: { language: "zh-CN" },
    });
  });

  it("falls back to the system language without a backend", async () => {
    tauriInvoke.mockRejectedValueOnce("not in tauri");
    await initLocale(["zh-CN"]);
    expect(currentLocale()).toBe("zh-CN");
  });
});

describe("setLocale", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
  });

  it("switches immediately and persists", async () => {
    tauriInvoke.mockResolvedValue(null);
    await setLocale("en-US");
    expect(composer.t("common.language.label")).toBe("Language");
    await setLocale("zh-CN");
    expect(composer.t("common.language.label")).toBe("界面语言");
    expect(tauriInvoke).toHaveBeenLastCalledWith("settings_set_language", {
      args: { language: "zh-CN" },
    });
  });

  it("rejects with an AppError when saving fails", async () => {
    const err = { code: "COMMON_IO", params: { path: "config.json" }, detail: null };
    tauriInvoke.mockRejectedValue(err);
    await expect(setLocale("en-US")).rejects.toEqual(err);
    expect(currentLocale()).toBe("en-US");
  });
});

describe("formatError", () => {
  it("interpolates params in the current language", () => {
    composer.locale.value = "zh-CN";
    const err = { code: "COMMON_IO" as const, params: { path: "config.json" }, detail: null };
    expect(formatError(err)).toBe("读写文件失败 config.json");
    composer.locale.value = "en-US";
    expect(
      formatError({
        ...err,
        code: "COMMON_CONFIG_SCHEMA_UNSUPPORTED",
        params: { schema: "9", supported: "1" },
      }),
    ).toBe("Config file version 9 is not supported (supported: 1)");
  });

  it("drops missing optional params", () => {
    composer.locale.value = "en-US";
    expect(formatError({ code: "COMMON_IO", params: {}, detail: null })).toBe(
      "Failed to read or write file ",
    );
  });
});
