<script setup lang="ts">
// 独立日志窗口的根组件：与主窗口相同的主题、材质与背景，自绘标题栏下显示一个实例的游戏日志。
// 设置在主窗口中修改，日志窗口获得焦点时重新读取，使主题与语言保持一致。
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { NConfigProvider, NDialogProvider, NMessageProvider } from "naive-ui";
import { computed, onBeforeUnmount, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { initLocale } from "../../i18n";
import TitleBar from "../../layouts/TitleBar.vue";
import { useSettingsStore } from "../../stores/settings";
import { useWindowTheme } from "../../theme/useWindowTheme";
import AppBackground from "../app-background/AppBackground.vue";
import { useMirroredSession } from "./bridge";
import GameLogView from "./GameLogView.vue";
import { rememberGeometry } from "./window";

const props = defineProps<{ instanceId: string }>();

const { t } = useI18n();
const settings = useSettingsStore();
const { theme, overrides, naiveLocale, naiveDateLocale } = useWindowTheme();
const session = useMirroredSession(props.instanceId);

const title = computed(() => t("gameLog.title", { name: session.value?.name ?? "" }));
watchEffect(() => {
  document.title = title.value;
  if (isTauri())
    void getCurrentWindow()
      .setTitle(title.value)
      .catch(() => undefined);
});

let unlisten: (() => void) | null = null;
if (isTauri()) {
  void rememberGeometry().catch(() => undefined);
  void getCurrentWindow()
    .onFocusChanged(({ payload: focused }) => {
      if (focused) void Promise.allSettled([initLocale(), settings.load()]);
    })
    .then((stop) => (unlisten = stop));
}
onBeforeUnmount(() => unlisten?.());
</script>

<template>
  <NConfigProvider
    :theme="theme"
    :theme-overrides="overrides"
    :locale="naiveLocale"
    :date-locale="naiveDateLocale"
    class="config-root"
  >
    <NMessageProvider>
      <!-- TitleBar 使用 useDialog，缺少 provider 时整个窗口挂载失败、显示为黑屏。 -->
      <NDialogProvider>
        <AppBackground>
          <div class="log-window">
            <TitleBar :title="title" plain-close />
            <main class="log-content">
              <GameLogView :session="session" />
            </main>
          </div>
        </AppBackground>
      </NDialogProvider>
    </NMessageProvider>
  </NConfigProvider>
</template>

<style>
html,
body,
#app,
.config-root {
  height: 100%;
  margin: 0;
}

body {
  font-family: var(--tmcl-font);
  background: var(--tmcl-bg);
  color: var(--tmcl-text);
  overflow: hidden;
  -webkit-font-smoothing: antialiased;
}
</style>

<style scoped>
.log-window {
  display: flex;
  flex-direction: column;
  height: 100%;
  color: var(--tmcl-text);
}

.log-content {
  flex: 1;
  min-height: 0;
  padding: 16px;
  border-top: 1px solid var(--tmcl-stroke);
  background: var(--tmcl-layer);
  -webkit-backdrop-filter: blur(var(--tmcl-card-blur, 0px));
  backdrop-filter: blur(var(--tmcl-card-blur, 0px));
}
</style>
