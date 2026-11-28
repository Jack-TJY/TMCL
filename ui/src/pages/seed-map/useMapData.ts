import {
  computed,
  onScopeDispose,
  ref,
  shallowReactive,
  watch,
  type Ref,
  type ComputedRef,
} from "vue";
import { toAppError } from "../../api/invoke";
import type { AppError } from "../../bindings/AppError";
import type { SeedDimension } from "../../bindings/SeedDimension";
import type { SeedStructureKind } from "../../bindings/SeedStructureKind";
import type { SeedTile } from "../../bindings/SeedTile";
import { useStructures } from "./useStructures";
import { runQuery } from "./request";
import { tileKey, type View } from "./viewport";
export const layerKinds: SeedStructureKind[] = [
  "village",
  "stronghold",
  "desert_pyramid",
  "jungle_pyramid",
  "ocean_monument",
  "mansion",
  "igloo",
  "swamp_hut",
  "pillager_outpost",
  "shipwreck",
  "ancient_city",
  "trail_ruins",
  "trial_chambers",
  "buried_treasure",
  "abandoned_camp",
  "fortress",
  "bastion",
];
export type World = { version: string; seed: string; dimension?: SeedDimension };

/** Layers offered for a version and dimension; only verified combinations are listed. */
export function availableLayers(version: string, dimension: SeedDimension = "overworld") {
  if (dimension === "the_nether") {
    return layerKinds.filter(
      (kind) => kind === "fortress" || (kind === "bastion" && version !== "1.12.2"),
    );
  }
  if (dimension === "the_end") return [];
  return layerKinds.filter(
    (kind) =>
      (kind !== "abandoned_camp" || version === "26.3") &&
      (!["pillager_outpost", "shipwreck"].includes(kind) || version !== "1.12.2") &&
      (!["ancient_city", "trail_ruins", "trial_chambers"].includes(kind) || version === "26.3") &&
      !["fortress", "bastion"].includes(kind),
  );
}
/** Keep decoded image URLs only for visible tiles. Revisits use the backend disk cache. */
export function useMapData(
  world: Ref<World>,
  tiles: ComputedRef<SeedTile[]>,
  layers: Ref<SeedStructureKind[]>,
  view: Ref<View>,
) {
  const urls = shallowReactive(new Map<string, string>());
  const tileError = ref<AppError | null>(null),
    retry = ref(0),
    loadingTiles = ref(false);
  const structure = useStructures(world, view, layers, retry);
  const tileSet = computed(() => tiles.value.map(tileKey).join(";"));
  const clear = (keep = new Set<string>()) => {
    for (const [key, url] of urls)
      if (!keep.has(key)) {
        urls.delete(key);
        URL.revokeObjectURL(url);
      }
  };
  const overworld = computed(() => (world.value.dimension ?? "overworld") === "overworld");
  watch(
    [world, tileSet, retry],
    ([w], [previous], cleanup) => {
      if (!overworld.value) {
        clear();
        loadingTiles.value = false;
        tileError.value = null;
        return;
      }
      const list = tiles.value;
      clear(w === previous ? new Set(list.map(tileKey)) : undefined);
      const missing = list.filter((t) => !urls.has(tileKey(t)));
      tileError.value = null;
      loadingTiles.value = missing.length > 0;
      const ctrl = new AbortController();
      const timer = setTimeout(() => {
        void (async () => {
          for (let i = 0; i < missing.length && !ctrl.signal.aborted; i += 16) {
            await runQuery(
              { ...w, query: { type: "tiles", tiles: missing.slice(i, i + 16) } },
              ctrl.signal,
              (msg) => {
                if (msg.type !== "tile") throw new Error("Unexpected tile response");
                const key = tileKey(msg.tile);
                if (!urls.has(key))
                  urls.set(
                    key,
                    URL.createObjectURL(
                      new Blob([Uint8Array.from(msg.png)], { type: "image/png" }),
                    ),
                  );
              },
            );
          }
        })()
          .catch((e) => {
            if (!ctrl.signal.aborted) tileError.value = toAppError(e);
          })
          .finally(() => {
            if (!ctrl.signal.aborted) loadingTiles.value = false;
          });
      }, 90);
      cleanup(() => {
        clearTimeout(timer);
        ctrl.abort();
      });
    },
    { immediate: true, flush: "sync" },
  );
  onScopeDispose(() => clear());
  return {
    urls,
    markers: structure.pins,
    partial: structure.partial,
    error: computed(() => tileError.value ?? structure.error.value),
    busy: computed(() => loadingTiles.value || structure.loading.value),
    retry: () => retry.value++,
  };
}
