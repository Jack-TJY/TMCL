import { effectScope, ref } from "vue";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SeedStructureKind } from "../../bindings/SeedStructureKind";
import { CACHE_ENTRIES, CELL, contains, MAX_REQUESTS } from "./regions";
import { runQuery } from "./request";
import { useStructures } from "./useStructures";
vi.mock("./request", () => ({ runQuery: vi.fn() }));
const run = vi.mocked(runQuery);
const camp = { chunk_x: 11, chunk_z: -11 };
let scope = effectScope();
function harness() {
  const world = ref({ seed: "0", version: "26.3" }),
    view = ref({ x: 176, z: -176, w: 800, h: 420, zoom: 2 }),
    layers = ref<SeedStructureKind[]>(["abandoned_camp"]),
    retry = ref(0);
  const data = scope.run(() => useStructures(world, view, layers, retry))!;
  return { world, view, layers, retry, ...data };
}
beforeEach(() => {
  scope = effectScope();
  vi.useFakeTimers();
  run.mockReset();
});
afterEach(() => {
  scope.stop();
  vi.useRealTimers();
});
const success: typeof runQuery = async (args, _, send) => {
  if (args.query.type !== "structures") throw Error();
  const { kind, bounds } = args.query;
  send({ type: "structures", kind, starts: contains(bounds, 11, -11) ? [camp, camp] : [] });
};
it("retains confirmed markers during the debounce, on failure and on retry", async () => {
  run.mockImplementation(success);
  const h = harness();
  await vi.runAllTimersAsync();
  expect(h.pins.value).toHaveLength(1);
  run.mockRejectedValue(new Error("offline"));
  h.view.value = { ...h.view.value, zoom: 4 };
  expect(h.pins.value).toHaveLength(1);
  await vi.runAllTimersAsync();
  expect(h.error.value?.code).toBe("COMMON_INTERNAL");
  expect(h.pins.value).toHaveLength(1);
  run.mockImplementation(success);
  h.retry.value++;
  await vi.runAllTimersAsync();
  expect(h.error.value).toBeNull();
  expect(h.pins.value).toHaveLength(1);
  const calls = run.mock.calls.length;
  for (const zoom of [3, 2]) {
    h.view.value = { ...h.view.value, zoom };
    expect(h.pins.value).toHaveLength(1);
    await vi.runAllTimersAsync();
  }
  expect(run.mock.calls.length).toBe(calls);
  h.layers.value = [];
  expect(h.pins.value).toHaveLength(0);
  h.layers.value = ["abandoned_camp"];
  expect(h.pins.value).toHaveLength(1);
  h.world.value = { seed: "-1", version: "26.3" };
  expect(h.pins.value).toHaveLength(0);
});
it("keeps region work sequential and discards late replies from cancelled work", async () => {
  let release!: () => void;
  run.mockImplementation(async (...args) => {
    await new Promise<void>((r) => {
      release = r;
    });
    await success(...args);
  });
  const h = harness();
  await vi.advanceTimersByTimeAsync(120);
  expect(run).toHaveBeenCalledTimes(1);
  const signal = run.mock.calls[0][1];
  h.view.value = { ...h.view.value, zoom: 5 };
  expect(signal.aborted).toBe(true);
  release();
  await vi.advanceTimersByTimeAsync(10);
  expect(h.pins.value).toEqual([]);
  expect(run).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(120);
  expect(run).toHaveBeenCalledTimes(2);
  h.layers.value = [];
  release();
  await vi.runAllTimersAsync();
  expect(h.pins.value).toEqual([]);
});
it("limits both retained cells and far-view work, aggregating confirmed starts", async () => {
  run.mockImplementation(async (a, _, send) => {
    if (a.query.type === "structures") {
      const { kind, bounds: b } = a.query;
      send({
        type: "structures",
        kind,
        starts: [
          { chunk_x: b.min_x, chunk_z: b.min_z },
          { chunk_x: b.min_x + 1, chunk_z: b.min_z },
        ],
      });
    }
  });
  const h = harness();
  for (let i = 0; i <= CACHE_ENTRIES; i++) {
    h.view.value = { x: (i * CELL + 1) * 16, z: 16, w: 32, h: 32, zoom: 0 };
    await vi.runAllTimersAsync();
  }
  expect(run).toHaveBeenCalledTimes(CACHE_ENTRIES + 1);
  h.view.value = { ...h.view.value, x: 16 };
  await vi.runAllTimersAsync();
  expect(run).toHaveBeenCalledTimes(CACHE_ENTRIES + 2);
  const before = run.mock.calls.length;
  h.view.value = { ...h.view.value, w: 1800, h: 800, zoom: 12 };
  await vi.runAllTimersAsync();
  expect(run.mock.calls.length - before).toBeLessThanOrEqual(MAX_REQUESTS);
  expect(h.partial.value).toBe(true);
  expect(h.pins.value.some((p) => p.count >= 2)).toBe(true);
});
