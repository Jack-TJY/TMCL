<script setup lang="ts">
// Win11 样式的自绘标题栏（窗口 `decorations: false`）：拖拽区域 + 最小化 / 最大化 / 关闭。
// 在浏览器中调试（非 Tauri 环境）时不显示窗口按钮。关闭时若仍有任务在运行，先请用户确认。
import {
  Dismiss16Regular,
  Maximize16Regular,
  SquareMultiple16Regular,
  Subtract16Regular,
} from "@vicons/fluent";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { NIcon, useDialog } from "naive-ui";
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { taskList } from "../api/task";

defineProps<{ title: string }>();

const { t } = useI18n();
const dialog = useDialog();
const inTauri = isTauri();
const appWindow = inTauri ? getCurrentWindow() : null;
const maximized = ref(false);
let unlisten: (() => void) | null = null;

onMounted(async () => {
  if (!appWindow) return;
  maximized.value = await appWindow.isMaximized();
  unlisten = await appWindow.onResized(async () => {
    maximized.value = await appWindow.isMaximized();
  });
});

onBeforeUnmount(() => unlisten?.());

async function requestClose() {
  if (!appWindow) return;
  const running = await taskList()
    .then(({ tasks }) => tasks.filter((task) => task.status === "running").length)
    .catch(() => 0);
  if (running === 0) {
    await appWindow.close();
    return;
  }
  dialog.warning({
    title: t("shell.window.exitConfirm.title"),
    content: t("shell.window.exitConfirm.content", { count: running }),
    positiveText: t("shell.window.exitConfirm.confirm"),
    negativeText: t("shell.window.exitConfirm.cancel"),
    onPositiveClick: () => appWindow.close(),
  });
}
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <slot name="leading" />
    <span class="titlebar-title" data-tauri-drag-region>{{ title }}</span>
    <slot name="trailing" />
    <div v-if="appWindow" class="caption-buttons">
      <button
        class="caption-button"
        :title="t('shell.window.minimize')"
        :aria-label="t('shell.window.minimize')"
        @click="appWindow.minimize()"
      >
        <NIcon :size="16"><Subtract16Regular /></NIcon>
      </button>
      <button
        class="caption-button"
        :title="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
        :aria-label="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
        @click="appWindow.toggleMaximize()"
      >
        <NIcon :size="16">
          <SquareMultiple16Regular v-if="maximized" />
          <Maximize16Regular v-else />
        </NIcon>
      </button>
      <button
        class="caption-button caption-close"
        :title="t('shell.window.close')"
        :aria-label="t('shell.window.close')"
        @click="requestClose()"
      >
        <NIcon :size="16"><Dismiss16Regular /></NIcon>
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  align-items: center;
  height: 40px;
  flex-shrink: 0;
  user-select: none;
}

.titlebar-title {
  flex: 1;
  padding-left: 4px;
  font-size: 12px;
  line-height: 40px;
  color: var(--tmcl-text);
}

.caption-buttons {
  display: flex;
  align-self: stretch;
}

.caption-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  border: none;
  background: transparent;
  color: var(--tmcl-text);
  transition: background-color 83ms var(--tmcl-ease);
  cursor: default;
}

.caption-button:hover {
  background: var(--tmcl-subtle);
}

.caption-button:active {
  background: var(--tmcl-subtle-pressed);
}

/* Win11 关闭按钮：悬停红底白字。 */
.caption-close:hover {
  background: #c42b1c;
  color: #ffffff;
}

.caption-close:active {
  background: #c83c31;
  color: rgba(255, 255, 255, 0.7);
}
</style>
