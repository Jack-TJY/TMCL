import { computed, ref, shallowReactive, watch, type Ref } from "vue";
import { toAppError } from "../../api/invoke";
import type { AppError } from "../../bindings/AppError";
import type { SeedStructureKind } from "../../bindings/SeedStructureKind";
import type { SeedStructureStart } from "../../bindings/SeedStructureStart";
import type { World } from "./useMapData";
import type { View } from "./viewport";
import { CACHE_ENTRIES, contains, intersects, planRegions, type Region } from "./regions";
import { runQuery } from "./request";
interface Cached extends Region {
  starts: SeedStructureStart[];
}
export interface Pin extends SeedStructureStart {
  kind: SeedStructureKind;
  count: number;
  key: string;
}
/** Cache by world, layer and aligned cell (not zoom); only publish complete, successful cells. */
export function useStructures(
  world: Ref<World>,
  view: Ref<View>,
  layers: Ref<SeedStructureKind[]>,
  retry: Ref<number>,
) {
  const cache = shallowReactive(new Map<string, Cached>()),
    error = ref<AppError | null>(null),
    loading = ref(false),
    full = ref(false);
  const plan = computed(() => planRegions(view.value, layers.value));
  function evictable() {
    for (const entry of cache.values()) {
      if (!layers.value.includes(entry.kind) || !intersects(plan.value.visible, entry.box))
        return entry.key;
    }
    return undefined;
  }
  const pins = computed(() => {
    const groups = new Map<string, Pin>();
    const visible = plan.value.visible;
    // At distant scales group confirmed points of the same kind in stable 32px bins.
    const bin = view.value.zoom >= 5 ? 2 ** view.value.zoom * 2 : 1;
    for (const { kind, box, starts } of cache.values()) {
      if (!layers.value.includes(kind) || !intersects(visible, box)) continue;
      for (const s of starts) {
        if (!contains(visible, s.chunk_x, s.chunk_z)) continue;
        const key = kind + "/" + Math.floor(s.chunk_x / bin) + "/" + Math.floor(s.chunk_z / bin);
        const pin = groups.get(key);
        if (pin) pin.count++;
        else groups.set(key, { ...s, kind, key, count: 1 });
      }
    }
    return [...groups.values()];
  });
  const identity = computed(() =>
    JSON.stringify([world.value.version, world.value.seed, world.value.dimension ?? "overworld"]),
  );
  watch(
    [identity, () => plan.value.jobs.map((j) => j.key).join(";"), retry],
    ([id], [prev], cleanup) => {
      if (id !== prev) cache.clear();
      error.value = null;
      full.value = false;
      const w = world.value;
      const jobs = plan.value.jobs.filter((j) => !cache.has(j.key));
      loading.value = !!jobs.length;
      if (!jobs.length) return;
      const ctrl = new AbortController();
      const timer = setTimeout(() => {
        void (async () => {
          for (const job of jobs) {
            if (ctrl.signal.aborted) return;
            // Never evict a still-visible active cell merely to admit a far-view query.
            if (cache.size >= CACHE_ENTRIES && evictable() === undefined) {
              full.value = true;
              break;
            }
            let received: SeedStructureStart[] | undefined;
            await runQuery(
              { ...w, query: { type: "structures", kind: job.kind, bounds: job.box } },
              ctrl.signal,
              (msg) => {
                if (ctrl.signal.aborted) return;
                if (msg.type !== "structures" || msg.kind !== job.kind)
                  throw new Error("Unexpected structure response");
                // Adjacent cells are disjoint; enforce ownership even on their shared seam.
                const unique = new Map<string, SeedStructureStart>();
                for (const s of msg.starts) {
                  if (contains(job.box, s.chunk_x, s.chunk_z))
                    unique.set(s.chunk_x + "," + s.chunk_z, s);
                }
                received = [...unique.values()];
              },
            );
            if (ctrl.signal.aborted) return;
            if (!received) throw new Error("Missing structure response");
            if (cache.size >= CACHE_ENTRIES) {
              // A sub-cell pan can change visibility without restarting this same queue.
              const victim = evictable();
              if (victim === undefined) {
                full.value = true;
                break;
              }
              cache.delete(victim);
            }
            cache.set(job.key, { ...job, starts: received });
          }
        })()
          .catch((e) => {
            if (!ctrl.signal.aborted) error.value = toAppError(e);
          })
          .finally(() => {
            if (!ctrl.signal.aborted) loading.value = false;
          });
      }, 100);
      cleanup(() => {
        clearTimeout(timer);
        ctrl.abort();
      });
    },
    { immediate: true, flush: "sync" },
  );
  return {
    pins,
    error,
    loading,
    partial: computed(() => full.value || plan.value.jobs.length < plan.value.total),
  };
}
