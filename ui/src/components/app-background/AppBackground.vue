<script setup lang="ts">
// 窗口背景（PLAN.md M1-19）：在应用框架下方绘制背景图层，并按卡片透明度覆盖卡片底色变量。
// 窗口材质生效时不画背景；自定义图片读取失败时回退内置背景并提示。
import { useMessage } from "naive-ui";
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { appearanceReadBackground } from "../../api/appearance";
import { toAppError } from "../../api/invoke";
import { formatError } from "../../i18n";
import { useSettingsStore } from "../../stores/settings";
import {
  cardVars,
  defaultBackgroundUrl,
  fitStyle,
  maskColor,
  needsTextShadow,
} from "../../theme/background";
import { fluentCssVars } from "../../theme/fluent";

const store = useSettingsStore();
const message = useMessage();
const background = computed(() => store.settings.appearance.background);

/** 当前自定义图片的 Blob URL。 */
const imageUrl = ref<string | null>(null);

function setImageUrl(url: string | null) {
  if (imageUrl.value) URL.revokeObjectURL(imageUrl.value);
  imageUrl.value = url;
}

const MIME: Record<string, string> = { png: "image/png", jpg: "image/jpeg", webp: "image/webp" };

async function loadImage(image: string) {
  try {
    const bytes = await appearanceReadBackground(image);
    if (store.settings.appearance.background.image !== image) return;
    const type = MIME[image.split(".").pop() ?? ""] ?? "";
    setImageUrl(URL.createObjectURL(new Blob([bytes], { type })));
  } catch (err) {
    setImageUrl(null);
    message.warning(formatError(toAppError(err)));
    // 图片已被删除：改回内置背景，避免每次启动都提示。
    store
      .update((s) => {
        s.appearance.background.kind = "default";
        s.appearance.background.image = null;
      })
      .catch(() => undefined);
  }
}

watch(
  () => [store.loaded, background.value.kind, background.value.image] as const,
  ([loaded, kind, image]) => {
    if (loaded && kind === "image" && image) void loadImage(image);
    else setImageUrl(null);
  },
  { immediate: true },
);
onBeforeUnmount(() => setImageUrl(null));

/** 背景图层；窗口材质生效时为 `null`。 */
const layerStyle = computed<Record<string, string> | null>(() => {
  if (store.materialActive) return null;
  const bg = background.value;
  const blur = bg.blur;
  const style: Record<string, string> = {
    opacity: String(bg.opacity / 100),
    // 模糊会让边缘透出底色，向外扩出模糊半径的两倍。
    inset: `${-blur * 2}px`,
    filter: blur > 0 ? `blur(${blur}px)` : "none",
  };
  if (bg.kind === "color") return { ...style, backgroundColor: bg.color };
  const url = bg.kind === "image" && imageUrl.value ? imageUrl.value : null;
  if (url) return { ...style, ...fitStyle(bg.fit), backgroundImage: `url("${url}")` };
  return {
    ...style,
    ...fitStyle("fill"),
    backgroundImage: `url("${defaultBackgroundUrl(store.isDark)}")`,
  };
});

const mask = computed(() => (store.materialActive ? null : maskColor(background.value.mask)));

const rootStyle = computed(() =>
  cardVars(fluentCssVars(store.accent, store.isDark), background.value.card_opacity),
);
const readable = computed(() => needsTextShadow(background.value.card_opacity));
</script>

<template>
  <div class="app-background" :class="{ readable, dark: store.isDark }" :style="rootStyle">
    <div v-if="layerStyle" class="background-layer" :style="layerStyle" aria-hidden="true" />
    <div v-if="mask" class="background-mask" :style="{ background: mask }" aria-hidden="true" />
    <div class="background-content">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.app-background {
  position: relative;
  height: 100%;
  overflow: hidden;
}

.background-layer,
.background-mask {
  position: absolute;
  pointer-events: none;
}

.background-layer {
  will-change: opacity, filter;
}

.background-mask {
  inset: 0;
}

.background-content {
  position: relative;
  height: 100%;
}

/* 卡片半透明时文字加一圈与底色相反的柔和阴影。 */
.readable {
  text-shadow: 0 0 3px rgba(255, 255, 255, 0.9);
}

.readable.dark {
  text-shadow: 0 0 3px rgba(0, 0, 0, 0.9);
}
</style>
