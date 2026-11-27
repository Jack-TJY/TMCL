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
import type { SeedStructureKind } from "../../bindings/SeedStructureKind";
import type { SeedTile } from "../../bindings/SeedTile";
import type { SeedStructureStart } from "../../bindings/SeedStructureStart";
import { runQuery } from "./request";
import { tileKey, structureBounds } from "./viewport";
export const layerKinds: SeedStructureKind[] = [
  "village",
  "stronghold",
  "desert_pyramid",
  "jungle_pyramid",
  "ocean_monument",
  "abandoned_camp",
];
export type World = { version: string; seed: string };
export type Marker = SeedStructureStart & { kind: SeedStructureKind };
/** Keep decoded image URLs only for visible tiles. Revisits use the backend disk cache. */
export function useMapData(
  world: Ref<World>,
  tiles: ComputedRef<SeedTile[]>,
  layers: Ref<SeedStructureKind[]>,
) {
  const urls = shallowReactive(new Map<string, string>()),
    markers = ref<Marker[]>([]);
  const tileError = ref<AppError | null>(null),
    layerError = ref<AppError | null>(null),
    retry = ref(0);
  const loadingTiles = ref(false),
    loadingLayers = ref(false);
  const bounds = computed(() => structureBounds(tiles.value));
  const tileSet = computed(() => tiles.value.map(tileKey).join(";"));
  const clear = (keep = new Set<string>()) => {
    for (const [key, url] of urls)
      if (!keep.has(key)) {
        urls.delete(key);
        URL.revokeObjectURL(url);
      }
  };
  watch(
    [world, tileSet, retry],
    ([w], [previous], cleanup) => {
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
  watch(
    [world, () => JSON.stringify(bounds.value), () => layers.value.join(","), retry],
    ([w], _, cleanup) => {
      markers.value = [];
      layerError.value = null;
      const box = bounds.value;
      const selected = [...layers.value];
      loadingLayers.value = !!box && selected.length > 0;
      if (!box || !selected.length) return;
      const ctrl = new AbortController();
      const timer = setTimeout(() => {
        void (async () => {
          for (const kind of selected) {
            if (ctrl.signal.aborted) break;
            await runQuery(
              { ...w, query: { type: "structures", kind, bounds: box } },
              ctrl.signal,
              (msg) => {
                if (msg.type !== "structures" || msg.kind !== kind)
                  throw new Error("Unexpected structure response");
                markers.value.push(...msg.starts.map((s) => ({ ...s, kind })));
              },
            );
          }
        })()
          .catch((e) => {
            if (!ctrl.signal.aborted) layerError.value = toAppError(e);
          })
          .finally(() => {
            if (!ctrl.signal.aborted) loadingLayers.value = false;
          });
      }, 120);
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
    markers,
    bounds,
    error: computed(() => tileError.value ?? layerError.value),
    busy: computed(() => loadingTiles.value || loadingLayers.value),
    retry: () => retry.value++,
  };
}
