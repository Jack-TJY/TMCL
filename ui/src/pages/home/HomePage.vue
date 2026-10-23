<script setup lang="ts">
// 首页（PLAN.md M1-18）：组件组成的 12 列栅格。右上角“编辑”进入编辑模式，
// 可添加、删除、拖动、拉伸组件与修改组件设置；“完成”保存，“取消”或 Esc 放弃本次修改。
import { Add20Regular, ArrowReset20Regular, Edit20Regular } from "@vicons/fluent";
import { GridItem, GridLayout } from "grid-layout-plus";
import { NButton, NEmpty, NIcon, NSpin, useMessage } from "naive-ui";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { onBeforeRouteLeave } from "vue-router";
import type { AppError } from "../../bindings/AppError";
import { formatError } from "../../i18n";
import { useLayoutStore, type HomeItem } from "../../stores/layout";
import { findWidget } from "../../widgets/registry";
import WidgetFrame from "../../widgets/WidgetFrame.vue";
import { widgetComponent } from "./widgetComponent";
import WidgetLibrary from "./WidgetLibrary.vue";
import WidgetSettingsDialog from "./WidgetSettingsDialog.vue";

/** 行高与间距（px）：窗口最小宽度下一列约 60 px，与行高接近，组件大致按正方形格子排布。 */
const ROW_HEIGHT = 64;
const MARGIN = 12;

const { t } = useI18n();
const message = useMessage();
const layout = useLayoutStore();

const libraryOpen = ref(false);
const configuring = ref<HomeItem | null>(null);

const items = computed(() =>
  layout.items.flatMap((item) => {
    const def = findWidget(item.type);
    return def ? [{ item, def }] : [];
  }),
);

onMounted(async () => {
  if (layout.loaded) return;
  const notice = await layout.load();
  if (notice.error) message.error(t("home.loadFailed", { error: formatError(notice.error) }));
  else if (notice.backup) message.warning(t("home.backup", { path: notice.backup }));
  else if (notice.skipped > 0) message.warning(t("home.skipped", { count: notice.skipped }));
});

async function finish() {
  try {
    await layout.finishEdit();
  } catch (err) {
    message.error(t("home.saveFailed", { error: formatError(err as AppError) }));
  }
}

function add(type: string) {
  if (layout.add(type)) libraryOpen.value = false;
}

function onKeydown(e: KeyboardEvent) {
  // 组件库或设置对话框打开时，Esc 只关闭它们。
  if (e.key !== "Escape" || !layout.editing || libraryOpen.value || configuring.value) return;
  e.preventDefault();
  layout.cancelEdit();
}
window.addEventListener("keydown", onKeydown);
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));

// 离开首页时放弃未保存的修改，回来时看到的总是已保存的布局。
onBeforeRouteLeave(() => layout.cancelEdit());
</script>

<template>
  <div class="home-page">
    <header class="home-header">
      <h1 class="page-title">{{ t("shell.nav.home") }}</h1>
      <div v-if="layout.editing" class="home-toolbar">
        <NButton :disabled="!layout.canAdd" @click="libraryOpen = true">
          <template #icon>
            <NIcon><Add20Regular /></NIcon>
          </template>
          {{ t("home.addWidget") }}
        </NButton>
        <NButton @click="layout.resetToDefault()">
          <template #icon>
            <NIcon><ArrowReset20Regular /></NIcon>
          </template>
          {{ t("home.reset") }}
        </NButton>
        <NButton @click="layout.cancelEdit()">{{ t("home.cancel") }}</NButton>
        <NButton type="primary" :loading="layout.saving" @click="finish">
          {{ t("home.done") }}
        </NButton>
      </div>
      <NButton v-else quaternary :disabled="!layout.loaded" @click="layout.startEdit()">
        <template #icon>
          <NIcon><Edit20Regular /></NIcon>
        </template>
        {{ t("home.edit") }}
      </NButton>
    </header>
    <p v-if="layout.editing" class="home-hint">{{ t("home.editHint") }}</p>

    <NSpin v-if="!layout.loaded" class="home-loading" />
    <NEmpty v-else-if="items.length === 0" class="home-empty" :description="t('home.empty')">
      <template #extra>
        <NButton v-if="!layout.editing" @click="layout.startEdit()">{{ t("home.edit") }}</NButton>
      </template>
    </NEmpty>
    <GridLayout
      v-else
      v-model:layout="layout.items"
      class="home-grid"
      :class="{ editing: layout.editing }"
      :col-num="12"
      :row-height="ROW_HEIGHT"
      :margin="[MARGIN, MARGIN]"
      :is-draggable="layout.editing"
      :is-resizable="layout.editing"
      vertical-compact
      use-css-transforms
    >
      <GridItem
        v-for="{ item, def } in items"
        :key="item.i"
        :i="item.i"
        :x="item.x"
        :y="item.y"
        :w="item.w"
        :h="item.h"
        :min-w="item.minW"
        :min-h="item.minH"
        :max-w="item.maxW"
        :max-h="item.maxH"
      >
        <WidgetFrame
          :def="def"
          :editing="layout.editing"
          @configure="configuring = item"
          @remove="layout.remove(item.i)"
        >
          <component
            :is="widgetComponent(def)"
            :uid="item.i"
            :settings="item.settings"
            :size="{ w: item.w, h: item.h }"
            :editing="layout.editing"
          />
        </WidgetFrame>
      </GridItem>
    </GridLayout>

    <WidgetLibrary v-model:show="libraryOpen" @add="add" />
    <WidgetSettingsDialog
      :item="configuring"
      @close="configuring = null"
      @save="(settings) => configuring && layout.setSettings(configuring.i, settings)"
    />
  </div>
</template>

<style scoped>
.home-page {
  min-width: 0;
}

.home-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.home-toolbar {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.home-hint {
  margin: -12px 0 12px;
  color: var(--tmcl-text-secondary);
  font-size: 12px;
}

.home-loading,
.home-empty {
  margin-top: 96px;
}

/* 栅格四周的间距由 margin 产生，向外抵消，让组件与标题左对齐。 */
.home-grid {
  margin: -12px;
  --vgl-placeholder-bg: var(--tmcl-accent);
  --vgl-placeholder-opacity: 16%;
  --vgl-resizer-border-color: var(--tmcl-text-secondary);
}

.home-grid :deep(.vgl-item__resizer) {
  z-index: 2;
  right: 4px;
  bottom: 4px;
}

.home-grid :deep(.vgl-item--placeholder) {
  border-radius: 8px;
}
</style>
