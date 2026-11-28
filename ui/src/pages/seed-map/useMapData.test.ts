import { computed, effectScope, ref } from "vue";
import { expect, it, vi } from "vitest";
import type { SeedMapArgs } from "../../bindings/SeedMapArgs";
import type { SeedMapResult } from "../../bindings/SeedMapResult";
import type { SeedStructureKind } from "../../bindings/SeedStructureKind";
const run = vi.hoisted(() => vi.fn());
vi.mock("./request", () => ({ runQuery: run }));
import { availableLayers, useMapData } from "./useMapData";
it("offers only verified layers for each dimension", () => {
  expect(availableLayers("26.3", "overworld")).toContain("village");
  expect(availableLayers("26.3", "overworld")).not.toContain("fortress");
  expect(availableLayers("26.3", "the_nether")).toEqual([
    "fortress",
    "bastion",
    "ruined_portal_nether",
  ]);
  expect(availableLayers("1.12.2", "the_nether")).toEqual(["fortress"]);
  expect(availableLayers("26.3", "the_end")).toEqual(["end_city"]);
});

it("batches only missing tiles and releases obsolete image URLs on pan, seed change and unmount", async () => {
  vi.useFakeTimers();
  const scope = effectScope();
  let n = 0;
  const create = vi.spyOn(URL, "createObjectURL").mockImplementation(() => "blob:test-" + ++n);
  const revoke = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  try {
    run.mockImplementation(
      async (a: SeedMapArgs, _signal: AbortSignal, apply: (m: SeedMapResult) => void) => {
        if (a.query.type === "tiles")
          for (const tile of a.query.tiles) apply({ type: "tile", tile, png: [0] });
      },
    );
    const world = ref({ version: "26.3", seed: "0" });
    const x = ref(0);
    const tiles = computed(() =>
      Array.from({ length: 17 }, (_, i) => ({ x: i + x.value, z: 0, zoom: 0 })),
    );
    const data = scope.run(() =>
      useMapData(
        world,
        tiles,
        ref<SeedStructureKind[]>([]),
        ref({ x: 0, z: 0, w: 800, h: 420, zoom: 0 }),
      ),
    )!;
    await vi.runAllTimersAsync();
    expect(data.urls.size).toBe(17);
    expect(run.mock.calls.map((c) => c[0].query.tiles.length)).toEqual([16, 1]);
    x.value = 1;
    await vi.runAllTimersAsync();
    expect(create).toHaveBeenCalledTimes(18);
    expect(revoke).toHaveBeenCalledTimes(1);
    world.value = { version: "26.3", seed: "1" };
    expect(data.urls.size).toBe(0);
    await vi.runAllTimersAsync();
    scope.stop();
    expect(revoke).toHaveBeenCalledTimes(35);
  } finally {
    scope.stop();
    vi.restoreAllMocks();
    vi.useRealTimers();
  }
});
