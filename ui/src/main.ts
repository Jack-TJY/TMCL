import { createPinia } from "pinia";
import { createApp } from "vue";
import App from "./App.vue";
import { i18n, initLocale } from "./i18n";
import { router } from "./router";
import { useSettingsStore } from "./stores/settings";

const pinia = createPinia();

// 先确定语言和主题再挂载，避免首屏先显示默认值再闪烁切换。
void Promise.allSettled([initLocale(), useSettingsStore(pinia).load()]).finally(() =>
  createApp(App).use(pinia).use(i18n).use(router).mount("#app"),
);
