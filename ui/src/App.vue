<script setup lang="ts">
// 根组件：按设置应用亮/暗主题、强调色与组件库语言，在窗口背景之上渲染应用框架。
import { NConfigProvider, NDialogProvider, NMessageProvider } from "naive-ui";
import AppBackground from "./components/app-background/AppBackground.vue";
import { serveLogWindows } from "./components/game-log/bridge";
import AppLayout from "./layouts/AppLayout.vue";
import { useWindowTheme } from "./theme/useWindowTheme";

const { theme, overrides, naiveLocale, naiveDateLocale } = useWindowTheme();
serveLogWindows();
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
