<script setup lang="ts">
// 设置页“背景与透明度”（PLAN.md M1-19）：背景来源、窗口材质类型与底色、图片填充、透明度、
// 模糊、遮罩与卡片透明度。
// 滑块拖动时立即预览，停止 300 ms 后保存。
import {
  ArrowReset24Regular,
  Blur24Regular,
  BrightnessHigh24Regular,
  CircleHalfFill24Regular,
  ColorBackground24Regular,
  Image24Regular,
  Layer24Regular,
  PaintBucket24Regular,
  Wallpaper24Regular,
  WindowNew24Regular,
} from "@vicons/fluent";
import { NButton, NColorPicker, NSelect, NSlider, useMessage } from "naive-ui";
import { computed, onBeforeUnmount } from "vue";
import { useI18n } from "vue-i18n";
import { appearanceImportBackground, BACKGROUND_EXTENSIONS } from "../../../api/appearance";
import { pickFile } from "../../../api/dialog";
import { toAppError } from "../../../api/invoke";
import type { BackgroundConfig } from "../../../bindings/BackgroundConfig";
import type { BackgroundFit } from "../../../bindings/BackgroundFit";
import type { WindowMaterial } from "../../../bindings/WindowMaterial";
import { formatError } from "../../../i18n";
import { useSettingsStore, type Settings } from "../../../stores/settings";
import {
  backgroundSource,
  DEFAULT_BACKGROUND,
  MAX_BLUR,
  type BackgroundSource,
} from "../../../theme/background";
import { isHexColor } from "../../../theme/fluent";
import { materialSelectable } from "../../../theme/material";
import SettingRow from "../SettingRow.vue";

const SOURCES: BackgroundSource[] = ["default", "color", "image", "material"];
const FITS: BackgroundFit[] = ["fill", "fit", "center", "tile"];
const MATERIALS: WindowMaterial[] = ["acrylic", "mica"];
const SAVE_DELAY_MS = 300;

const { t } = useI18n();
const message = useMessage();
const store = useSettingsStore();

const background = computed(() => store.settings.appearance.background);
const materialSupported = computed(() => store.materialKind !== "none");
const source = computed(() =>
  backgroundSource(
    store.settings.appearance.window_material,
    materialSupported.value,
    background.value.kind,
  ),
);
const usesMaterial = computed(() => source.value === "material");

const sourceOptions = computed(() =>
  SOURCES.map((value) => ({
    label: t(`appearance.source.${value}`),
    value,
    disabled: value === "material" && !materialSupported.value,
  })),
);
const materialOptions = computed(() =>
  MATERIALS.map((value) => ({ label: t(`appearance.material.${value}`), value })),
);
const showMaterialKind = computed(
  () => usesMaterial.value && materialSelectable(store.materialKind),
);
const fitOptions = computed(() =>
  FITS.map((value) => ({ label: t(`appearance.fit.${value}`), value })),
);

function showError(err: unknown) {
  message.error(formatError(toAppError(err)));
}

function save(change: (draft: Settings) => void) {
  store.update(change).catch(showError);
}

let timer: ReturnType<typeof setTimeout> | undefined;

type SliderKey = "material_opacity" | "opacity" | "blur" | "mask" | "card_opacity";

/** 滑块：立即预览，停止拖动后保存。 */
function preview(key: SliderKey, value: number) {
  if (key === "material_opacity") store.settings.appearance.material_opacity = value;
  else store.settings.appearance.background[key] = value;
  clearTimeout(timer);
  timer = setTimeout(() => save(() => undefined), SAVE_DELAY_MS);
}

onBeforeUnmount(() => {
  if (timer === undefined) return;
  clearTimeout(timer);
  save(() => undefined);
});

/** 选择并导入图片；成功后切换为图片背景。 */
async function chooseImage() {
  try {
    const path = await pickFile([
      { name: t("appearance.image.filter"), extensions: BACKGROUND_EXTENSIONS },
    ]);
    if (!path) return;
    const { image } = await appearanceImportBackground(path);
    await store.update((s) => {
      s.appearance.window_material = false;
      s.appearance.background.kind = "image";
      s.appearance.background.image = image;
    });
  } catch (err) {
    showError(err);
  }
}

function setSource(value: BackgroundSource) {
  if (value === "material") {
    save((s) => (s.appearance.window_material = true));
  } else if (value === "image" && !background.value.image) {
    void chooseImage();
  } else {
    save((s) => {
      s.appearance.window_material = false;
      s.appearance.background.kind = value;
    });
  }
}

function setColor(value: string) {
  if (isHexColor(value)) save((s) => (s.appearance.background.color = value.toUpperCase()));
}

function setMaterial(value: WindowMaterial) {
  save((s) => (s.appearance.material = value));
}

function setFit(value: BackgroundFit) {
  save((s) => (s.appearance.background.fit = value));
}

/** 恢复默认：保留已导入的图片，方便再次选用。 */
function reset() {
  save((s) => {
    s.appearance.window_material = true;
    s.appearance.material = "acrylic";
    s.appearance.material_opacity = 0;
    s.appearance.background = { ...DEFAULT_BACKGROUND, image: s.appearance.background.image };
  });
}

const percent = (value: number) => `${value}%`;
const pixels = (value: number) => `${value} px`;
const signed = (value: number) => (value > 0 ? `+${value}` : String(value));

const sliders = computed(() => {
  const bg: BackgroundConfig = background.value;
  const material = {
    key: "material_opacity" as const,
    icon: ColorBackground24Regular,
    min: 0,
    max: 100,
    value: store.settings.appearance.material_opacity,
    format: percent,
    disabled: false,
  };
  return [
    ...(usesMaterial.value ? [material] : []),
    {
      key: "opacity" as const,
      icon: CircleHalfFill24Regular,
      min: 0,
      max: 100,
      value: bg.opacity,
      format: percent,
      disabled: usesMaterial.value,
    },
    {
      key: "blur" as const,
      icon: Blur24Regular,
      min: 0,
      max: MAX_BLUR,
      value: bg.blur,
      format: pixels,
      disabled: usesMaterial.value,
    },
    {
      key: "mask" as const,
      icon: BrightnessHigh24Regular,
      min: -100,
      max: 100,
      value: bg.mask,
      format: signed,
      disabled: usesMaterial.value,
    },
    {
      key: "card_opacity" as const,
      icon: Layer24Regular,
      min: 0,
      max: 100,
      value: bg.card_opacity,
      format: percent,
      disabled: false,
    },
  ];
});
</script>

<template>
  <SettingRow
    :icon="Wallpaper24Regular"
    :title="t('appearance.source.label')"
    :description="
      materialSupported ? t('appearance.source.description') : t('appearance.source.noMaterial')
    "
  >
    <NSelect
      class="control-select"
      :value="source"
      :options="sourceOptions"
      @update:value="setSource"
    />
  </SettingRow>
  <SettingRow
    v-if="showMaterialKind"
    :icon="WindowNew24Regular"
    :title="t('appearance.material.label')"
    :description="t('appearance.material.description')"
  >
    <NSelect
      class="control-select"
      :value="store.settings.appearance.material"
      :options="materialOptions"
      @update:value="setMaterial"
    />
  </SettingRow>
  <SettingRow
    v-if="source === 'color'"
    :icon="PaintBucket24Regular"
    :title="t('appearance.color.label')"
  >
    <NColorPicker
      class="control-color"
      :value="background.color"
      :modes="['hex']"
      :show-alpha="false"
      @update:value="setColor"
    />
  </SettingRow>
  <SettingRow
    v-if="source === 'image'"
    :icon="Image24Regular"
    :title="t('appearance.image.label')"
    :description="t('appearance.image.description')"
  >
    <NSelect
      class="control-fit"
      :value="background.fit"
      :options="fitOptions"
      :aria-label="t('appearance.fit.label')"
      @update:value="setFit"
    />
    <NButton @click="chooseImage">{{ t("appearance.image.choose") }}</NButton>
  </SettingRow>
  <SettingRow
    v-for="slider in sliders"
    :key="slider.key"
    :icon="slider.icon"
    :title="t(`appearance.${slider.key}.label`)"
    :description="
      slider.disabled ? t('appearance.materialHint') : t(`appearance.${slider.key}.description`)
    "
  >
    <NSlider
      class="control-slider"
      :value="slider.value"
      :min="slider.min"
      :max="slider.max"
      :disabled="slider.disabled"
      :format-tooltip="slider.format"
      @update:value="(v: number) => preview(slider.key, v)"
    />
    <span class="slider-value">{{ slider.format(slider.value) }}</span>
  </SettingRow>
  <SettingRow
    :icon="ArrowReset24Regular"
    :title="t('appearance.reset.label')"
    :description="t('appearance.reset.description')"
  >
    <NButton @click="reset">{{ t("appearance.reset.action") }}</NButton>
  </SettingRow>
</template>

<style scoped>
.control-select {
  width: 200px;
}

.control-fit {
  width: 120px;
}

.control-color {
  width: 120px;
}

.control-slider {
  width: 180px;
}

.slider-value {
  min-width: 44px;
  text-align: right;
  font-variant-numeric: tabular-nums;
  color: var(--tmcl-text-secondary);
}
</style>
