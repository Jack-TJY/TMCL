// 窗口主题：按设置把亮/暗主题、强调色与窗口材质应用到当前窗口，主窗口与日志窗口共用。
import { darkTheme, dateEnUS, dateZhCN, enUS, zhCN } from "naive-ui";
import { computed, watch, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../stores/settings";
import { scaleAlpha } from "./background";
import { fluentCssVars, fluentOverrides } from "./fluent";

/** 应用主题并返回 `NConfigProvider` 所需的属性。须在组件 setup 中调用。 */
export function useWindowTheme() {
  const settings = useSettingsStore();
  const { locale } = useI18n();

  const theme = computed(() => (settings.isDark ? darkTheme : null));
  const overrides = computed(() => fluentOverrides(settings.accent, settings.isDark));
  const naiveLocale = computed(() => (locale.value === "zh-CN" ? zhCN : enUS));
  const naiveDateLocale = computed(() => (locale.value === "zh-CN" ? dateZhCN : dateEnUS));

  // 布局等非 Naive UI 元素使用的变量，挂在根元素上。
  watchEffect(() => {
    const root = document.documentElement;
    const vars = fluentCssVars(settings.accent, settings.isDark);
    for (const [name, value] of Object.entries(vars)) {
      root.style.setProperty(name, value);
    }
    // 材质生效时主题底色按设置半透明叠在材质上：0% 为纯材质，100% 为纯色。
    if (settings.materialActive) {
      const opacity = settings.settings.appearance.material_opacity / 100;
      root.style.setProperty("--tmcl-bg", scaleAlpha(vars["--tmcl-bg"], opacity));
    }
    root.style.colorScheme = settings.isDark ? "dark" : "light";
  });

  // 材质的开关与明暗跟随设置变化。
  watch(
    () => [
      settings.settings.appearance.window_material,
      settings.settings.appearance.material,
      settings.isDark,
      settings.materialKind,
    ],
    () => void settings.syncMaterial(),
    { immediate: true },
  );

  return { theme, overrides, naiveLocale, naiveDateLocale };
}
