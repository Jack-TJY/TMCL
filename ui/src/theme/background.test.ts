import { describe, expect, it } from "vitest";
import {
  backgroundSource,
  cardVars,
  fitStyle,
  maskColor,
  needsTextShadow,
  scaleAlpha,
} from "./background";
import { fluentCssVars } from "./fluent";

describe("background", () => {
  it("maps fit modes to CSS", () => {
    expect(fitStyle("fill").backgroundSize).toBe("cover");
    expect(fitStyle("fit").backgroundSize).toBe("contain");
    expect(fitStyle("center")).toMatchObject({
      backgroundSize: "auto",
      backgroundRepeat: "no-repeat",
    });
    expect(fitStyle("tile").backgroundRepeat).toBe("repeat");
  });

  it("darkens with negative mask and brightens with positive mask", () => {
    expect(maskColor(0)).toBeNull();
    expect(maskColor(-40)).toBe("rgba(0, 0, 0, 0.4)");
    expect(maskColor(25)).toBe("rgba(255, 255, 255, 0.25)");
  });

  it("scales existing alpha", () => {
    expect(scaleAlpha("#FFFFFF", 0.5)).toBe("rgba(255, 255, 255, 0.5)");
    expect(scaleAlpha("rgba(58, 58, 58, 0.3)", 0.5)).toBe("rgba(58, 58, 58, 0.15)");
  });

  it("makes cards translucent and frosted below 100%", () => {
    const light = fluentCssVars("#0067C0", false);
    expect(cardVars(light, 100)).toEqual({
      "--tmcl-card": "rgba(255, 255, 255, 1)",
      "--tmcl-layer": "rgba(255, 255, 255, 0.5)",
      "--tmcl-card-blur": "0px",
    });
    const dark = cardVars(fluentCssVars("#0067C0", true), 0);
    expect(dark["--tmcl-card"]).toBe("rgba(43, 43, 43, 0)");
    expect(dark["--tmcl-card-blur"]).toBe("20px");
  });

  it("adds text shadow only for very transparent cards", () => {
    expect(needsTextShadow(85)).toBe(false);
    expect(needsTextShadow(30)).toBe(true);
  });

  it("falls back from material when the platform lacks it", () => {
    expect(backgroundSource(true, true, "image")).toBe("material");
    expect(backgroundSource(true, false, "image")).toBe("image");
    expect(backgroundSource(false, true, "default")).toBe("default");
  });
});
