import { createPinia } from "pinia";
import { createApp } from "vue";
import App from "./App.vue";
import GameLogApp from "./components/game-log/GameLogApp.vue";
import { logWindowInstance } from "./components/game-log/window";
import { i18n, initLocale } from "./i18n";
import { router } from "./router";
import { useSettingsStore } from "./stores/settings";

const pinia = createPinia();

// 独立日志窗口加载同一个前端，按地址挂载日志界面，不使用路由。
const logInstance = logWindowInstance(location.hash);

// 先确定语言和主题再挂载，避免首屏先显示默认值再闪烁切换。
void Promise.allSettled([initLocale(), useSettingsStore(pinia).load()]).finally(() => {
  if (logInstance) {
    createApp(GameLogApp, { instanceId: logInstance }).use(pinia).use(i18n).mount("#app");
  } else {
    createApp(App).use(pinia).use(i18n).use(router).mount("#app");
  }
});
