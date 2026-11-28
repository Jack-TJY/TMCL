import { describe, expect, it } from "vitest";
import { structureIcons } from "./structureIcons";
import { layerKinds } from "./useMapData";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import StructureIcon from "./StructureIcon.vue";

describe("seed-map pictograms", () => {
  it("has a dedicated asset for every selectable layer and for nether-only kinds", () => {
    for (const kind of layerKinds) expect(structureIcons[kind]).toBeTypeOf("string");
    expect(structureIcons.fortress).toBeTypeOf("string");
    expect(structureIcons.bastion).toBeTypeOf("string");
    expect(new Set(Object.values(structureIcons)).size).toBe(Object.keys(structureIcons).length);
  });

  it.each([...layerKinds, "fortress", "bastion"] as const)(
    "renders the registered image for %s, leaving the accessible name on its parent",
    async (kind) => {
      const html = await renderToString(createSSRApp({ render: () => h(StructureIcon, { kind }) }));
      const expected = await renderToString(h("img", { src: structureIcons[kind], alt: "" }));
      expect(html).toContain(expected.slice("<img ".length, -1));

      expect(html).toMatch(/\salt(?:\s|=)/);
      expect(html).toContain('aria-hidden="true"');
    },
  );
});
