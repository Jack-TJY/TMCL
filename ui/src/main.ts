import { createApp } from "vue";
import App from "./App.vue";
import { i18n, initLocale } from "./i18n";

// 先确定语言再挂载，避免首屏先显示默认语言再闪烁切换。
void initLocale().finally(() => createApp(App).use(i18n).mount("#app"));
