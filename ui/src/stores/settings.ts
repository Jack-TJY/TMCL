// 启动器设置与外观状态（PLAN.md M1-11）。界面语言由 `i18n.ts` 管理，这里不重复保存。
import { defineStore } from "pinia";
import { computed, ref, toRaw } from "vue";
import {
  DEFAULT_SETTINGS,
  mockSettingsGet,
  mockSettingsSave,
  type LauncherSettings,
} from "../api/__mocks__/settings";
import { toAppError } from "../api/invoke";
import { DEFAULT_ACCENT, systemAccent } from "../theme/fluent";

/** 跟踪系统的深浅色偏好。 */
function useSystemDark() {
  const query =
    typeof window !== "undefined" && window.matchMedia
      ? window.matchMedia("(prefers-color-scheme: dark)")
      : null;
  const dark = ref(query?.matches ?? false);
  query?.addEventListener("change", (e) => (dark.value = e.matches));
  return dark;
}

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<LauncherSettings>(structuredClone(DEFAULT_SETTINGS));
  const loaded = ref(false);
  const systemDark = useSystemDark();
  // 系统强调色在启动时读取一次；用户修改系统设置后重启生效。
  const detectedAccent = systemAccent();

  /** 当前是否使用暗色主题。 */
  const isDark = computed(() =>
    settings.value.theme === "system" ? systemDark.value : settings.value.theme === "dark",
  );

  /** 实际使用的强调色。 */
  const accent = computed(() => settings.value.accent ?? detectedAccent ?? DEFAULT_ACCENT);

  /** 系统强调色；取不到时为 `null`。 */
  const systemAccentColor = detectedAccent;

  /** 读取已保存的设置；失败时保持默认值。 */
  async function load() {
    try {
      settings.value = await mockSettingsGet();
    } finally {
      loaded.value = true;
    }
  }

  /** 修改设置：界面立即生效，保存失败时回滚并抛出 AppError。 */
  async function update(change: (draft: LauncherSettings) => void) {
    const previous = structuredClone(toRaw(settings.value));
    const next = structuredClone(toRaw(settings.value));
    change(next);
    settings.value = next;
    try {
      await mockSettingsSave(next);
    } catch (err) {
      settings.value = previous;
      throw toAppError(err);
    }
  }

  return { settings, loaded, isDark, accent, systemAccentColor, load, update };
});
