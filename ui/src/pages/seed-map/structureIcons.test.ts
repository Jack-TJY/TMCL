import { describe, expect, it } from "vitest";
import { structureIcons } from "./structureIcons";
import { layerKinds } from "./useMapData";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import StructureIcon from "./StructureIcon.vue";

describe("seed-map pictograms", () => {
  it("covers every selectable structure once with a dedicated asset", () => {
    expect(Object.keys(structureIcons).sort()).toEqual([...layerKinds].sort());
    expect(new Set(Object.values(structureIcons)).size).toBe(layerKinds.length);
  });
  it.each(layerKinds)(
    "renders the registered image for %s, leaving the accessible name on its parent",
    async (kind) => {
      const html = await renderToString(createSSRApp({ render: () => h(StructureIcon, { kind }) }));
      const expected = await renderToString(h("img", { src: structureIcons[kind], alt: "" }));
      expect(html).toContain(expected.slice("<img ".length, -1));
      expect(html).toContain('aria-hidden="true"');
    },
  );
});
