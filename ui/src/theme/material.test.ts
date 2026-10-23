import { describe, expect, it } from "vitest";
import { applyMaterial, detectMaterial } from "./material";

const windows = (platformVersion: string) => ({
  platform: "Windows",
  getHighEntropyValues: async () => ({ platformVersion }),
});

describe("detectMaterial", () => {
  it("uses Mica on Windows 11 only", async () => {
    expect(await detectMaterial(windows("15.0.0"), "")).toBe("mica");
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
