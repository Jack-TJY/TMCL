import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import { expect, it, vi } from "vitest";
import { i18n } from "../../i18n";
import SeedMapPage from "./SeedMapPage.vue";
import StructureIcon from "./StructureIcon.vue";

vi.mock("./useMapData", async (importOriginal) => {
  const original = await importOriginal<typeof import("./useMapData")>();
  const { ref } = await import("vue");
  return {
    ...original,
    useMapData: () => ({
      urls: new Map(),
      markers: ref([
        { key: "a", kind: "village", chunk_x: 38, chunk_z: 45, count: 1 },
        { key: "b", kind: "stronghold", chunk_x: -13, chunk_z: -106, count: 3 },
      ]),
      busy: ref(false),
      error: ref(null),
      partial: ref(false),
      retry: vi.fn(),
    }),
  };
});

it("uses the legend artwork on single and grouped pins without replacing accessible labels", async () => {
  const html = await renderToString(createSSRApp(SeedMapPage).use(i18n));
  const [map, legend] = html.split("<fieldset");
  for (const kind of ["village", "stronghold"] as const) {
    const img = await renderToString(h(StructureIcon, { kind }));
    const src = img.match(/src="[^"]+"/)![0];
    expect(map).toContain(src);
    expect(legend).toContain(src);
  }
  expect(map).toContain('aria-label="Village · start chunk (38, 45)"');
  expect(map).toContain(
    'aria-label="Stronghold × 3 confirmed · representative start chunk (-13, -106) · activate to zoom in"',
  );
  expect(map).toMatch(/<span class="cluster-count" aria-hidden="true"[^>]*>×3<\/span>/);
  const pins = map.match(/<(?:span|button)[^>]* class="pin(?: cluster)?"[^>]*>/g) ?? [];
  expect(pins).toHaveLength(2);
  expect(pins.filter((tag) => tag.includes('tabindex="0"'))).toHaveLength(1);
});
