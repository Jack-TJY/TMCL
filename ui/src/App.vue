<script setup lang="ts">
// 根组件：按设置应用亮/暗主题、强调色与组件库语言，在窗口背景之上渲染应用框架。
import {
  darkTheme,
  dateEnUS,
  dateZhCN,
  enUS,
  NConfigProvider,
  NDialogProvider,
  NMessageProvider,
  zhCN,
} from "naive-ui";
import { computed, watch, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import AppBackground from "./components/app-background/AppBackground.vue";
import AppLayout from "./layouts/AppLayout.vue";
import { useSettingsStore } from "./stores/settings";
import { fluentCssVars, fluentOverrides } from "./theme/fluent";

const settings = useSettingsStore();
const { locale } = useI18n();

const theme = computed(() => (settings.isDark ? darkTheme : null));
const overrides = computed(() => fluentOverrides(settings.accent, settings.isDark));
const naiveLocale = computed(() => (locale.value === "zh-CN" ? zhCN : enUS));
const naiveDateLocale = computed(() => (locale.value === "zh-CN" ? dateZhCN : dateEnUS));

// 布局等非 Naive UI 元素使用的变量，挂在根元素上。
watchEffect(() => {
  const root = document.documentElement;
  for (const [name, value] of Object.entries(fluentCssVars(settings.accent, settings.isDark))) {
    root.style.setProperty(name, value);
  }
  if (settings.materialActive) root.style.setProperty("--tmcl-bg", "transparent");
  root.style.colorScheme = settings.isDark ? "dark" : "light";
});

// 材质的开关与明暗跟随设置变化。
watch(
  () => [settings.settings.appearance.window_material, settings.isDark, settings.materialKind],
  () => void settings.syncMaterial(),
  { immediate: true },
);
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
      <NDialogProvider>
        <AppBackground>
          <AppLayout />
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
