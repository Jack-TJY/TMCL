<script setup lang="ts">
// 组件库：列出全部已登记的组件，带按默认尺寸缩小的实时预览，点击“添加”放到第一个空位。
import { NButton, NDrawer, NDrawerContent } from "naive-ui";
import { useI18n } from "vue-i18n";
import { WIDGETS } from "../../widgets/registry";
import { normalizeSettings, type WidgetDefinition } from "../../widgets/types";
import WidgetFrame from "../../widgets/WidgetFrame.vue";
import { widgetComponent } from "./widgetComponent";

const show = defineModel<boolean>("show", { required: true });
const emit = defineEmits<{ add: [type: string] }>();
const { t } = useI18n();

/** 预览按 64 px 一格绘制，再缩放到抽屉宽度。 */
const CELL = 64;
const GAP = 12;
const PREVIEW_WIDTH = 312;

function previewStyle(def: WidgetDefinition) {
  const width = def.defaultSize.w * CELL + (def.defaultSize.w - 1) * GAP;
  const height = def.defaultSize.h * CELL + (def.defaultSize.h - 1) * GAP;
  const scale = Math.min(1, PREVIEW_WIDTH / width);
  return {
    box: { height: `${height * scale}px` },
    inner: { width: `${width}px`, height: `${height}px`, transform: `scale(${scale})` },
  };
}
</script>

<template>
  <NDrawer v-model:show="show" :width="360" placement="right">
    <NDrawerContent :title="t('home.library.title')" closable :native-scrollbar="false">
      <ul class="library">
        <li v-for="def in WIDGETS" :key="def.id" class="library-item">
          <div class="preview" :style="previewStyle(def).box" aria-hidden="true">
            <div class="preview-inner" :style="previewStyle(def).inner">
              <WidgetFrame :def="def" :editing="false">
                <component
                  :is="widgetComponent(def)"
                  uid="preview"
                  :settings="normalizeSettings(def.settingsSchema, {})"
                  :size="def.defaultSize"
                  :editing="true"
                />
              </WidgetFrame>
            </div>
          </div>
          <div class="library-info">
            <div class="library-text">
              <div class="library-name">{{ t(def.titleKey) }}</div>
              <div class="library-desc">{{ t(def.descriptionKey) }}</div>
            </div>
            <NButton size="small" @click="emit('add', def.id)">{{ t("home.library.add") }}</NButton>
          </div>
        </li>
      </ul>
    </NDrawerContent>
  </NDrawer>
</template>

<style scoped>
.library {
  display: flex;
  flex-direction: column;
  gap: 20px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.preview {
  position: relative;
  overflow: hidden;
  border-radius: 8px;
  pointer-events: none;
}

.preview-inner {
  transform-origin: top left;
}

.library-info {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 8px;
}

.library-text {
  flex: 1;
  min-width: 0;
}

.library-name {
  font-weight: 600;
}

.library-desc {
  color: var(--tmcl-text-secondary);
  font-size: 12px;
}
</style>
