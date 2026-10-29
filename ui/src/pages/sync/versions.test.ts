import { describe, expect, it } from "vitest";
import { mergeOptions, SYNC_TEMPLATES, templateNoteKey } from "./templates";
import { compareVersions, parseRelease, spanText, supportedOn, usesLegacyKeys } from "./versions";

describe("versions", () => {
  it("parses and compares releases", () => {
    expect(parseRelease("1.8")).toEqual([1, 8, 0]);
    expect(parseRelease("23w31a")).toBeNull();
    expect(compareVersions("1.16.5", "1.16")).toBeGreaterThan(0);
    expect(compareVersions("1.9", "1.12.2")).toBeLessThan(0);
    expect(compareVersions("1.20", "1.20.0")).toBe(0);
    expect(compareVersions("1.20.1", "24w10a")).toBeNull();
  });

  it("knows which versions have an option", () => {
    expect(supportedOn("graphicsMode", "1.16")).toBe(true);
    expect(supportedOn("graphicsMode", "1.15.2")).toBe(false);
    expect(supportedOn("fancyGraphics", "1.15.2")).toBe(true);
    expect(supportedOn("key_key.swapHands", "1.8.9")).toBe(false);
    expect(supportedOn("key_key.swapHands", "1.12.2")).toBe(true);
    expect(supportedOn("simulationDistance", "1.17.1")).toBe(false);
    expect(supportedOn("renderDistance", "1.8.9")).toBeNull();
    expect(supportedOn("graphicsMode", "20w20a")).toBeNull();
  });

  it("detects legacy key codes and formats spans", () => {
    expect(usesLegacyKeys("1.12.2")).toBe(true);
    expect(usesLegacyKeys("1.13")).toBe(false);
    expect(usesLegacyKeys("18w01a")).toBeNull();
    expect(spanText({ since: "1.16" })).toBe("1.16+");
    expect(spanText({ until: "1.15.2" })).toBe("≤1.15.2");
    expect(spanText({ since: "1.9", until: "1.15.2" })).toBe("1.9–1.15.2");
  });
});

describe("templates", () => {
  const byVersion = (v: string) => {
    const tpl = SYNC_TEMPLATES.find((x) => x.version === v);
    if (!tpl) throw new Error(v);
    return new Map(tpl.options.map((o) => [o.key, o.value]));
  };

  it("only contains options that exist in that version", () => {
    for (const tpl of SYNC_TEMPLATES) {
      for (const o of tpl.options) expect(supportedOn(o.key, tpl.version)).not.toBe(false);
      expect(new Set(tpl.options.map((o) => o.key)).size).toBe(tpl.options.length);
    }
    const old = byVersion("1.8.9");
    expect(old.has("key_key.swapHands")).toBe(false);
    expect(old.has("attackIndicator")).toBe(false);
    expect(old.get("ao")).toBe("2");
    expect(old.get("fancyGraphics")).toBe("true");
    const modern = byVersion("1.20.1");
    expect(modern.has("fancyGraphics")).toBe(false);
    expect(modern.get("simulationDistance")).toBe("12");
    expect(modern.get("key_key.swapOffhand")).toBe("key.keyboard.f");
    expect(modern.get("key_key.hotbar.9")).toBe("key.keyboard.9");
  });

  it("builds note keys", () => {
    expect(templateNoteKey("1.12.2")).toBe("sync.template.note.1_12_2");
  });

  it("merges into existing options", () => {
    expect(
      mergeOptions(
        [
          { key: "fov", value: "0.5" },
          { key: "key_mod.x", value: "key.keyboard.r" },
        ],
        [
          { key: "fov", value: "0.0" },
          { key: "gamma", value: "0.5" },
        ],
      ),
    ).toEqual([
      { key: "fov", value: "0.0" },
      { key: "key_mod.x", value: "key.keyboard.r" },
      { key: "gamma", value: "0.5" },
    ]);
  });
});
