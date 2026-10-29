<script setup lang="ts">
// 皮肤 3D 预览（PLAN.md M2-08）：skinview3d，拖拽旋转、滚轮缩放、动作、Steve/Alex 模型与披风。
// 图片地址由调用方经后端取得（object URL），本组件不访问网络和文件系统。
import { SkinViewer } from "skinview3d";
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { SkinVariant } from "../../api/skin";
import { createAnimation, viewerModel, type SkinAnimation } from "./animations";

const props = withDefaults(
  defineProps<{
    /** 皮肤图片地址；`null` 时不显示人物。 */
    skin: string | null;
    variant?: SkinVariant;
    /** 披风图片地址；`null` 时不显示披风。 */
    cape?: string | null;
    animation?: SkinAnimation;
    autoRotate?: boolean;
  }>(),
  { variant: "classic", cape: null, animation: "walk", autoRotate: false },
);

const emit = defineEmits<{
  /** 无法创建 WebGL 上下文，或图片无法加载。 */
  error: [kind: "webgl" | "load"];
}>();

const host = ref<HTMLDivElement | null>(null);
const canvas = ref<HTMLCanvasElement | null>(null);
let viewer: SkinViewer | null = null;
let resize: ResizeObserver | null = null;

async function loadSkin() {
  if (!viewer) return;
  if (props.skin === null) {
    viewer.loadSkin(null);
    return;
  }
  try {
    await viewer.loadSkin(props.skin, { model: viewerModel(props.variant) });
  } catch {
    emit("error", "load");
  }
}

async function loadCape() {
  if (!viewer) return;
  if (props.cape === null) {
    viewer.loadCape(null);
    return;
  }
  try {
    await viewer.loadCape(props.cape);
  } catch {
    emit("error", "load");
  }
}

onMounted(() => {
  if (!host.value || !canvas.value) return;
  try {
    viewer = new SkinViewer({
      canvas: canvas.value,
      width: host.value.clientWidth || 300,
      height: host.value.clientHeight || 400,
      zoom: 0.85,
    });
  } catch {
    emit("error", "webgl");
    return;
  }
  viewer.animation = createAnimation(props.animation);
  viewer.autoRotate = props.autoRotate;
  void loadSkin();
  void loadCape();
  resize = new ResizeObserver(([entry]) => {
    const { width, height } = entry.contentRect;
    if (viewer && width > 0 && height > 0) viewer.setSize(width, height);
  });
  resize.observe(host.value);
});

onBeforeUnmount(() => {
  resize?.disconnect();
  viewer?.dispose();
  viewer = null;
});

watch(() => [props.skin, props.variant], loadSkin);
watch(() => props.cape, loadCape);
watch(
  () => props.animation,
  (kind) => {
    if (viewer) viewer.animation = createAnimation(kind);
  },
);
watch(
  () => props.autoRotate,
  (value) => {
    if (viewer) viewer.autoRotate = value;
  },
);
</script>

<template>
  <div ref="host" class="skin-viewer">
    <canvas ref="canvas" />
  </div>
</template>

<style scoped>
.skin-viewer {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

canvas {
  position: absolute;
  inset: 0;
  cursor: grab;
}

canvas:active {
  cursor: grabbing;
}
</style>
