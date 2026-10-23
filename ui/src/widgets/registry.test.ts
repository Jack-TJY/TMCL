import { describe, expect, it } from "vitest";
import type { Composer } from "vue-i18n";
import { i18n, SUPPORTED_LOCALES } from "../i18n";
import { DEFAULT_LAYOUT } from "./defaults";
import { findWidget, WIDGETS } from "./registry";

const composer = i18n.global as unknown as Composer;

describe("widget registry", () => {
  it("is sorted by id without duplicates", () => {
    const ids = WIDGETS.map((w) => w.id);
    expect(ids).toEqual([...new Set(ids)].sort());
    for (const id of ids) expect(findWidget(id)?.id).toBe(id);
  });

  it("has texts for every widget in every language", () => {
    for (const locale of SUPPORTED_LOCALES) {
      for (const w of WIDGETS) {
        const keys = [
          w.titleKey,
          w.descriptionKey,
          ...(w.settingsSchema ?? []).map((f) => f.labelKey),
        ];
        for (const key of keys) {
          expect(key.startsWith(`widget-${w.id}.`)).toBe(true);
          expect(composer.te(key, locale), `${locale} ${key}`).toBe(true);
        }
      }
    }
  });

  it("default layout fits the grid without overlaps", () => {
    for (const a of DEFAULT_LAYOUT) {
      expect(a.x + a.w).toBeLessThanOrEqual(12);
      for (const b of DEFAULT_LAYOUT) {
        if (a === b) continue;
        const overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
        expect(overlap, `${a.uid} / ${b.uid}`).toBe(false);
      }
    }
  });
});
