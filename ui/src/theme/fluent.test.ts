import { describe, expect, it } from "vitest";
import { DEFAULT_ACCENT, fluentOverrides, isHexColor, mix, parseColor } from "./fluent";

describe("parseColor", () => {
  it("parses hex and rgb()", () => {
    expect(parseColor("#0067C0")).toEqual([0, 103, 192]);
    expect(parseColor("rgb(0, 103, 192)")).toEqual([0, 103, 192]);
    expect(parseColor("rgba(1 2 3 / 50%)")).toEqual([1, 2, 3]);
  });

  it("returns null for anything else", () => {
    expect(parseColor("AccentColor")).toBeNull();
    expect(parseColor("#fff")).toBeNull();
  });
});

describe("mix", () => {
  it("interpolates towards the target", () => {
    expect(mix("#000000", "#FFFFFF", 0)).toBe("#000000");
    expect(mix("#000000", "#FFFFFF", 1)).toBe("#FFFFFF");
    expect(mix("#000000", "#FFFFFF", 0.5)).toBe("#808080");
  });

  it("falls back to the default accent for invalid input", () => {
    expect(mix("nope", "#FFFFFF", 0)).toBe(DEFAULT_ACCENT);
  });
});

describe("isHexColor", () => {
  it("accepts only #RRGGBB", () => {
    expect(isHexColor("#0067c0")).toBe(true);
    expect(isHexColor("0067C0")).toBe(false);
    expect(isHexColor("#0067C0FF")).toBe(false);
  });
});

describe("fluentOverrides", () => {
  it("uses Win11 radii and the accent colour", () => {
    const light = fluentOverrides("#0067C0", false);
    expect(light.common?.borderRadius).toBe("4px");
    expect(light.Card?.borderRadius).toBe("8px");
    expect(light.Dialog?.borderRadius).toBe("8px");
    expect(light.common?.primaryColor).toBe("#0067C0");
  });

  it("brightens the accent on dark backgrounds", () => {
    const dark = fluentOverrides("#0067C0", true);
    expect(dark.common?.primaryColor).not.toBe("#0067C0");
    expect(dark.common?.bodyColor).toBe("#202020");
    expect(dark.Button?.textColorPrimary).toBe("#000000");
  });
});
