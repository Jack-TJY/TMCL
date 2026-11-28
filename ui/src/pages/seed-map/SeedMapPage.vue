<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { formatError } from "../../i18n";
import { blockAt, clamp, jumpPoint, SIZE, tileKey, tilesInView, zoomAt } from "./viewport";
import { availableLayers, useMapData, type World } from "./useMapData";
import StructureIcon from "./StructureIcon.vue";
import type { Pin } from "./useStructures";
import type { SeedDimension } from "../../bindings/SeedDimension";
import type { SeedStructureKind } from "../../bindings/SeedStructureKind";
const { t } = useI18n();
const versions = ["1.12.2", "1.18", "26.3"];
const seed = ref("0"),
  version = ref("26.3"),
  dimension = ref<SeedDimension>("overworld"),
  world = shallowRef<World>({
    version: version.value,
    seed: seed.value,
    dimension: dimension.value,
  });
const v = ref({ x: 0, z: 0, zoom: 1, w: 0, h: 0 });
const map = ref<HTMLElement | null>(null);
const layers = ref<SeedStructureKind[]>(["village"]);
const available = computed(() => availableLayers(world.value.version, world.value.dimension));
function switchDimension(next: SeedDimension) {
  dimension.value = next;
  // Switch the world identity first so stale query results are discarded.
  world.value = { version: version.value, seed: seed.value, dimension: next };
  const offered = availableLayers(version.value, next);
  layers.value = offered.slice(0, 1);
  cursor.value = null;
}
function onDimensionChange(event: Event) {
  switchDimension((event.target as HTMLSelectElement).value as SeedDimension);
}
const visible = computed(() => tilesInView(v.value));
const { urls, markers, partial, busy, error, retry } = useMapData(world, visible, layers, v);
const step = computed(() => 2 ** v.value.zoom);
const pos = (x: number, z: number) => ({
  left: (x - v.value.x) / step.value + v.value.w / 2 + "px",
  top: (z - v.value.z) / step.value + v.value.h / 2 + "px",
});
const cursor = ref<{ x: number; z: number } | null>(null),
  jumpX = ref("0"),
  jumpZ = ref("0"),
  invalid = ref("");
const selectedPinKey = ref(""),
  inspectingPin = ref(false);
const selectedPin = computed(() => markers.value.find((pin) => pin.key === selectedPinKey.value));
const tabPin = computed(() => selectedPin.value?.key ?? markers.value[0]?.key);
function pinLabel(pin: Pin) {
  return t(pin.count > 1 ? "seedMap.cluster" : "seedMap.marker", {
    kind: t(`seedMap.layer.${pin.kind}`),
    n: pin.count,
    x: pin.chunk_x,
    z: pin.chunk_z,
  });
}
function focusPin(pin: Pin) {
  selectedPinKey.value = pin.key;
  inspectingPin.value = true;
}
// One tab stop for the whole overlay, even in a densely marked viewport.
function pinKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    map.value?.focus({ preventScroll: true });
  } else if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) {
    const pins = Array.from(map.value?.querySelectorAll<HTMLElement>(".pin") ?? []);
    if (!pins.length) return;
    const i = pins.indexOf(e.currentTarget as HTMLElement);
    const next =
      e.key === "Home"
        ? 0
        : e.key === "End"
          ? pins.length - 1
          : (i + (e.key === "ArrowLeft" ? -1 : 1) + pins.length) % pins.length;
    pins[next].focus({ preventScroll: true });
  } else return;
  e.preventDefault();
  e.stopPropagation();
}
function expandPin(pin: Pin) {
  if (pin.count <= 1) return;
  v.value = { ...v.value, x: pin.chunk_x * 16, z: pin.chunk_z * 16, zoom: v.value.zoom - 1 };
  map.value?.focus({ preventScroll: true });
}
function apply() {
  if (!seed.value.trim() || new TextEncoder().encode(seed.value).length > 1024) {
    invalid.value = "seedMap.badSeed";
    return;
  }
  invalid.value = "";
  world.value = { version: version.value, seed: seed.value, dimension: dimension.value };
  layers.value = layers.value.filter((k) => available.value.includes(k));
  cursor.value = null;
}
function jump() {
  const point = jumpPoint(jumpX.value, jumpZ.value);
  if (!point) {
    invalid.value = "seedMap.badPoint";
    return;
  }
  invalid.value = "";
  v.value = { ...v.value, ...point };
  cursor.value = null;
}
function origin() {
  jumpX.value = jumpZ.value = "0";
  jump();
}
function zoom(delta: number, x = v.value.w / 2, y = v.value.h / 2) {
  v.value = zoomAt(v.value, delta, x, y);
  cursor.value = null;
}
function local(e: MouseEvent) {
  const rect = map.value!.getBoundingClientRect();
  return { x: e.clientX - rect.left, z: e.clientY - rect.top };
}
function wheel(e: WheelEvent) {
  if (!e.deltaY) return;
  const p = local(e);
  zoom(Math.sign(e.deltaY), p.x, p.z);
}
let drag: { id: number; x: number; z: number } | null = null;
function start(e: PointerEvent) {
  if (e.button !== 0 || drag) return;
  map.value!.focus({ preventScroll: true });
  map.value!.setPointerCapture(e.pointerId);
  drag = { id: e.pointerId, x: e.clientX, z: e.clientY };
}
function move(e: PointerEvent) {
  if (drag?.id === e.pointerId) {
    v.value = {
      ...v.value,
      x: clamp(v.value.x + (drag.x - e.clientX) * step.value),
      z: clamp(v.value.z + (drag.z - e.clientY) * step.value),
    };
    drag.x = e.clientX;
    drag.z = e.clientY;
  }
  const p = local(e);
  cursor.value = blockAt(v.value, p.x, p.z);
}
function end(e: PointerEvent) {
  if (drag?.id === e.pointerId) drag = null;
}
function key(e: KeyboardEvent) {
  const dirs: Record<string, [number, number]> = {
    ArrowLeft: [-1, 0],
    ArrowRight: [1, 0],
    ArrowUp: [0, -1],
    ArrowDown: [0, 1],
  };
  if (e.key === "+" || e.key === "=") zoom(-1);
  else if (e.key === "-") zoom(1);
  else if (dirs[e.key]) {
    const [dx, dz] = dirs[e.key];
    v.value = {
      ...v.value,
      x: clamp(v.value.x + dx * 64 * step.value),
      z: clamp(v.value.z + dz * 64 * step.value),
    };
    cursor.value = null;
  } else return;
  e.preventDefault();
}
let observer: ResizeObserver | undefined;
onMounted(() => {
  const el = map.value!;
  const measure = () => {
    v.value = { ...v.value, w: el.clientWidth, h: el.clientHeight };
  };
  observer = new ResizeObserver(measure);
  observer.observe(el);
  measure();
});
onBeforeUnmount(() => observer?.disconnect());
</script>
<template>
  <section class="seed-page">
    <header>
      <h1 class="page-title">{{ t("seedMap.title") }}</h1>
      <p>{{ t("seedMap.subtitle") }} · {{ t("seedMap.predicted") }}</p>
    </header>
    <form class="bar" @submit.prevent="apply">
      <label class="seed-input"
        >{{ t("seedMap.seed")
        }}<input v-model="seed" maxlength="1024" spellcheck="false" autocomplete="off" /></label
      ><label
        >{{ t("seedMap.version")
        }}<select v-model="version">
          <option v-for="ver in versions" :key="ver">{{ ver }}</option>
        </select></label
      ><label
        >{{ t("seedMap.dimension")
        }}<select :value="dimension" @change="onDimensionChange">
          <option value="overworld">{{ t("seedMap.dimensionOverworld") }}</option>
          <option value="the_nether">{{ t("seedMap.dimensionNether") }}</option>
          <option value="the_end">{{ t("seedMap.dimensionEnd") }}</option>
        </select></label
      ><button class="primary" type="submit">{{ t("seedMap.apply") }}</button>
    </form>
    <div class="map-shell">
      <div
        ref="map"
        class="map"
        tabindex="0"
        role="region"
        :aria-label="t('seedMap.title')"
        @pointerdown="start"
        @pointermove="move"
        @pointerup="end"
        @pointercancel="end"
        @lostpointercapture="end"
        @pointerleave="cursor = null"
        @wheel.prevent="wheel"
        @keydown="key"
      >
        <template v-for="tile in visible" :key="tileKey(tile)"
          ><img
            v-if="urls.has(tileKey(tile))"
            :src="urls.get(tileKey(tile))"
            alt=""
            draggable="false"
            class="tile"
            :style="pos(tile.x * SIZE * step, tile.z * SIZE * step)"
        /></template>
        <component
          v-for="m in markers"
          :key="m.key"
          :is="m.count > 1 ? 'button' : 'span'"
          :type="m.count > 1 ? 'button' : undefined"
          :role="m.count === 1 ? 'img' : undefined"
          class="pin"
          :class="{ cluster: m.count > 1 }"
          :tabindex="m.key === tabPin ? 0 : -1"
          :data-kind="m.kind"
          :style="pos(m.chunk_x * 16, m.chunk_z * 16)"
          :title="pinLabel(m)"
          :aria-label="pinLabel(m)"
          @focus="focusPin(m)"
          @blur="inspectingPin = false"
          @keydown="pinKey"
          @pointerdown="m.count > 1 && $event.stopPropagation()"
          @click="expandPin(m)"
        >
          <StructureIcon :kind="m.kind" /><span
            v-if="m.count > 1"
            class="cluster-count"
            aria-hidden="true"
            >×{{ m.count }}</span
          >
        </component>
        <span class="cross" aria-hidden="true">+</span>
      </div>
      <div class="zoom">
        <button
          type="button"
          :disabled="v.zoom === 0"
          :aria-label="t('seedMap.zoomIn')"
          :title="t('seedMap.zoomIn')"
          @click="zoom(-1)"
        >
          +</button
        ><button
          type="button"
          :disabled="v.zoom === 12"
          :aria-label="t('seedMap.zoomOut')"
          :title="t('seedMap.zoomOut')"
          @click="zoom(1)"
        >
          −
        </button>
      </div>
      <span v-if="busy" class="badge" role="status">{{ t("seedMap.working") }}</span>
    </div>
    <div class="readout">
      <b>{{ t("seedMap.center") }}: X {{ Math.floor(v.x) }} · Z {{ Math.floor(v.z) }}</b
      ><span v-if="cursor"
        >{{ t("seedMap.pointer") }}: X {{ Math.floor(cursor.x) }} · Z
        {{ Math.floor(cursor.z) }}</span
      ><span>{{ t("seedMap.scale", { n: step }) }}</span>
    </div>
    <p v-if="inspectingPin && selectedPin" class="pin-info">{{ pinLabel(selectedPin) }}</p>
    <form class="bar jump" @submit.prevent="jump">
      <label>X<input v-model="jumpX" inputmode="text" aria-label="X" /></label
      ><label>Z<input v-model="jumpZ" inputmode="text" aria-label="Z" /></label
      ><button type="submit">{{ t("seedMap.jump") }}</button
      ><button type="button" @click="origin">{{ t("seedMap.origin") }}</button>
    </form>
    <fieldset v-if="available.length">
      <legend>{{ t("seedMap.layers") }}</legend>
      <label v-for="k in available" :key="k"
        ><input v-model="layers" type="checkbox" :value="k" /><StructureIcon :kind="k" />
        {{ t(`seedMap.layer.${k}`) }}</label
      >
    </fieldset>
    <p v-if="dimension !== 'overworld'" class="tip">{{ t("seedMap.structureOnly") }}</p>
    <p v-if="!available.length" class="tip">{{ t("seedMap.endEmpty") }}</p>
    <p v-if="v.w && !visible.length && dimension === 'overworld'">{{ t("seedMap.edge") }}</p>
    <p v-if="partial" class="tip">{{ t("seedMap.partial") }}</p>
    <p class="tip">{{ t("seedMap.hint") }}</p>
    <p v-if="invalid" role="alert">{{ t(invalid) }}</p>
    <p v-if="error" role="alert">
      {{ formatError(error) }} <button @click="retry">{{ t("seedMap.retry") }}</button>
    </p>
  </section>
</template>
<style scoped>
.seed-page {
  display: grid;
  gap: 12px;
  max-width: 1440px;
  color: var(--tmcl-text);
}
h1,
p {
  margin: 0;
}
header p,
.tip,
.readout {
  color: var(--tmcl-text-secondary);
}
.bar,
.readout {
  display: flex;
  align-items: flex-end;
  flex-wrap: wrap;
  gap: 8px 14px;
}
.bar label {
  display: grid;
  gap: 3px;
}
.seed-input {
  flex: 1;
  min-width: 150px;
}
.jump input {
  width: 100px;
}
.bar input,
select,
button {
  box-sizing: border-box;
  font: inherit;
  color: inherit;
  border: 1px solid var(--tmcl-stroke);
  background: var(--tmcl-card, Canvas);
  border-radius: 5px;
  padding: 4px 9px;
  min-height: 30px;
}
select option {
  background: Canvas;
  color: CanvasText;
}
button {
  cursor: pointer;
}
button:hover {
  background: var(--tmcl-subtle);
}
button:disabled {
  opacity: 0.45;
  cursor: default;
}
.primary {
  font-weight: 600;
}
.map-shell {
  position: relative;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 9px;
  overflow: hidden;
}
.map {
  isolation: isolate;
  height: clamp(260px, 47vh, 600px);
  position: relative;
  overflow: hidden;
  touch-action: none;
  user-select: none;
  cursor: grab;
  background: repeating-conic-gradient(#293139 0 25%, #303942 0 50%) 0 0/24px 24px;
}
.map:active {
  cursor: grabbing;
}
.map:focus-visible {
  outline: 2px solid Highlight;
  outline-offset: -3px;
}
.tile,
.pin {
  position: absolute;
}
.tile {
  width: 256px;
  height: 256px;
  pointer-events: none;
  image-rendering: pixelated;
}
/* The icon center is the chunk anchor. The badge is out of flow so its
   digit count never shifts the glyph off that coordinate. */
.pin,
.pin.cluster {
  box-sizing: border-box;
  width: 28px;
  height: 28px;
  min-height: 0;
  padding: 0;
  display: grid;
  place-items: center;
  z-index: 1;
  transform: translate(-50%, -50%);
  background: #25333b;
  border: 1px solid #fff4d8;
  border-radius: 4px;
  box-shadow:
    0 0 0 1px #222d35,
    0 1px 3px #0009;
}
.pin.cluster {
  cursor: zoom-in;
}
.cluster-count {
  position: absolute;
  left: calc(100% - 3px);
  bottom: -2px;
  padding: 1px 3px;
  border-radius: 3px;
  background: #15212b;
  border: 1px solid #fff4d8;
  color: white;
  font: 700 10px/13px sans-serif;
  white-space: nowrap;
}
.pin:hover,
.pin:focus-visible {
  z-index: 2;
}
.pin:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 2px;
  box-shadow: 0 0 0 5px #15212b;
}
.pin-info {
  font-size: 13px;
  color: var(--tmcl-text);
}
.cross {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  color: white;
  text-shadow:
    0 0 2px black,
    0 0 3px black;
  pointer-events: none;
}
.zoom,
.badge {
  position: absolute;
  top: 9px;
}
.zoom {
  right: 9px;
  display: flex;
  gap: 3px;
}
.zoom button,
.badge {
  background: #19232de6;
  color: white;
}
.badge {
  left: 9px;
  padding: 3px 8px;
  border-radius: 4px;
  pointer-events: none;
}
.readout {
  font-variant-numeric: tabular-nums;
  align-items: center;
}
.readout span:last-child {
  margin-left: auto;
}
fieldset {
  margin: 0;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 6px;
}
fieldset label {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  margin: 3px 12px 3px 0;
}
fieldset .structure-icon {
  box-sizing: content-box;
  padding: 1px;
  border-radius: 3px;
  background: #25333b;
  border: 1px solid #fff4d8;
  box-shadow: 0 0 0 1px #222d35;
}
</style>
