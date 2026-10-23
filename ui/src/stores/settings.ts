// 启动器设置与外观状态（PLAN.md M1-11）。界面语言由 `i18n.ts` 管理，这里不重复保存。
// 设置经 `settings_get` / `settings_set` 保存在 `config.json`；下载源暂由 api/__mocks__ 提供（M1-02）。
import { defineStore } from "pinia";
import { computed, ref, toRaw } from "vue";
import {
  DEFAULT_SOURCE,
  isSource,
  mockSourceGet,
  mockSourceSave,
  type DownloadSource,
} from "../api/__mocks__/settings";
import { toAppError } from "../api/invoke";
import { settingsGet, settingsSet } from "../api/settings";
import type { LauncherSettings } from "../bindings/LauncherSettings";
import { DEFAULT_ACCENT, systemAccent } from "../theme/fluent";
import { applyMaterial, detectMaterial, type MaterialKind } from "../theme/material";

/** 设置页编辑的设置：后端的 `LauncherSettings` 加上暂存于前端的下载源。 */
export type Settings = LauncherSettings & {
  download: LauncherSettings["download"] & { source: DownloadSource };
};

/** 与后端 `Config::default()` 一致的默认设置。 */
export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  appearance: { accent: null, window_material: true },
  memory: { max_mb: 4096 },
  download: { max_concurrency: 16, source: DEFAULT_SOURCE },
};

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

/** 合并后端设置与前端暂存的下载源；后端提供 `source` 时以后端为准。 */
function withSource(saved: LauncherSettings, source: DownloadSource): Settings {
  const backend = saved.download.source;
  return {
    ...saved,
    download: { ...saved.download, source: isSource(backend) ? backend : source },
  };
}

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<Settings>(structuredClone(DEFAULT_SETTINGS));
  const loaded = ref(false);
  /** 数据目录的绝对路径；读取前（或没有后端时）为空串。 */
  const dataDir = ref("");
  const systemDark = useSystemDark();
  // 系统强调色在启动时读取一次；用户修改系统设置后重启生效。
  const detectedAccent = systemAccent();

  /** 当前是否使用暗色主题。 */
  const isDark = computed(() =>
    settings.value.theme === "system" ? systemDark.value : settings.value.theme === "dark",
  );

  /** 实际使用的强调色。 */
  const accent = computed(
    () => settings.value.appearance.accent ?? detectedAccent ?? DEFAULT_ACCENT,
  );

  /** 系统强调色；取不到时为 `null`。 */
  const systemAccentColor = detectedAccent;

  /** 平台支持的窗口材质，启动时检测。 */
  const materialKind = ref<MaterialKind>("none");
  /** 窗口材质当前是否生效；生效时页面底色透明。 */
  const materialActive = ref(false);

  /** 按当前设置应用窗口材质。 */
  async function syncMaterial() {
    materialActive.value = await applyMaterial(
      materialKind.value,
      settings.value.appearance.window_material,
      isDark.value,
    );
  }

  /** 读取已保存的设置；读取失败（例如在浏览器中调试、没有后端）时保持默认值。 */
  async function load() {
    try {
      const [snapshot, source, material] = await Promise.all([
        settingsGet().catch(() => null),
        mockSourceGet(),
        detectMaterial().catch(() => "none" as const),
      ]);
      materialKind.value = material;
      if (snapshot) {
        settings.value = withSource(snapshot.settings, source);
        dataDir.value = snapshot.data_dir;
      } else {
        settings.value.download.source = source;
      }
    } finally {
      loaded.value = true;
    }
  }

  /** 修改设置：界面立即生效，保存失败时回滚并抛出 AppError。 */
  async function update(change: (draft: Settings) => void) {
    const previous = structuredClone(toRaw(settings.value));
    const next = structuredClone(toRaw(settings.value));
    change(next);
    settings.value = next;
    try {
      const [saved] = await Promise.all([
        settingsSet({ settings: next }),
        mockSourceSave(next.download.source),
      ]);
      settings.value = withSource(saved, next.download.source);
    } catch (err) {
      settings.value = previous;
      throw toAppError(err);
    }
  }

  return {
    settings,
    loaded,
    dataDir,
    isDark,
    accent,
    systemAccentColor,
    materialKind,
    materialActive,
    load,
    update,
    syncMaterial,
  };
});
