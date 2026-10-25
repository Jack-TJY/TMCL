import { describe, expect, it } from "vitest";
import { Effect } from "@tauri-apps/api/window";
import {
  applyMaterial,
  detectMaterial,
  materialEffects,
  materialSelectable,
  removeWindows10Frame,
} from "./material";

const windows = (platformVersion: string) => ({
  platform: "Windows",
  getHighEntropyValues: async () => ({ platformVersion }),
});

describe("detectMaterial", () => {
  it("uses Acrylic or Mica on Windows 11 only", async () => {
    expect(await detectMaterial(windows("15.0.0"), "")).toBe("acrylic");
    expect(await detectMaterial(windows("19.0.0"), "")).toBe("acrylic");
    expect(await detectMaterial(windows("14.0.0"), "")).toBe("mica");
    expect(await detectMaterial(windows("13.0.0"), "")).toBe("mica");
    expect(await detectMaterial(windows("10.0.0"), "")).toBe("none");
    expect(await detectMaterial(windows(""), "")).toBe("none");
  });

  it("uses vibrancy on macOS", async () => {
    const ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";
    expect(await detectMaterial(undefined, ua)).toBe("vibrancy");
  });

  it("falls back to a solid background elsewhere", async () => {
    expect(await detectMaterial(undefined, "Mozilla/5.0 (X11; Linux x86_64)")).toBe("none");
    expect(
      await detectMaterial({ ...windows("15.0.0"), platform: "Linux" }, "Mozilla/5.0 (X11)"),
    ).toBe("none");
  });
});

describe("applyMaterial", () => {
  it("is inactive outside Tauri", async () => {
    expect(await applyMaterial("mica", true, false)).toBe(false);
  });
});

describe("removeWindows10Frame", () => {
  it("does nothing outside Tauri", async () => {
    await expect(removeWindows10Frame(windows("10.0.0"))).resolves.toBeUndefined();
  });
});

describe("materialEffects", () => {
  it("follows the chosen material where Acrylic is smooth", () => {
    expect(materialEffects("acrylic", "acrylic")).toEqual([Effect.Acrylic]);
    expect(materialEffects("acrylic", "mica")).toEqual([Effect.Mica]);
    expect(materialEffects("mica", "acrylic")).toEqual([Effect.Mica]);
    expect(materialEffects("vibrancy", "acrylic")).toEqual([Effect.Sidebar]);
  });

  it("offers the choice on Windows 11 22H2+ only", () => {
    expect(materialSelectable("acrylic")).toBe(true);
    expect(materialSelectable("mica")).toBe(false);
    expect(materialSelectable("vibrancy")).toBe(false);
  });
});
