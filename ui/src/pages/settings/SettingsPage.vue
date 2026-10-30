<script setup lang="ts">
// 设置页（PLAN.md M1-11）：外观（背景与透明度见 appearance/，M1-19）、下载、游戏、存储与关于。
// 下载源与 Java 列表暂由 api/__mocks__ 提供，见该文件说明。
import {
  ArrowDownload24Regular,
  Bug24Regular,
  Color24Regular,
  DarkTheme24Regular,
  DeveloperBoard24Regular,
  DrinkCoffee24Regular,
  Folder24Regular,
  FolderOpen24Regular,
  Info24Regular,
  LocalLanguage24Regular,
  Server24Regular,
  TopSpeed24Regular,
} from "@vicons/fluent";
import { NButton, NColorPicker, NInputNumber, NSelect, NSlider, NTag, useMessage } from "naive-ui";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { mockJavaList, type DownloadSource, type JavaInstall } from "../../api/__mocks__/settings";
import { appInfo } from "../../api/app";
import { pickSavePath } from "../../api/dialog";
import { diagnosticsExport } from "../../api/diagnostics";
import { toAppError } from "../../api/invoke";
import { settingsOpenDataDir } from "../../api/settings";
import type { AppInfo } from "../../bindings/AppInfo";
import type { ThemeMode } from "../../bindings/ThemeMode";
import { formatError, isLocale, LOCALE_NAMES, setLocale, SUPPORTED_LOCALES } from "../../i18n";
import { useSettingsStore, type Settings } from "../../stores/settings";
import { DEFAULT_ACCENT, isHexColor } from "../../theme/fluent";
import BackgroundSettings from "./appearance/BackgroundSettings.vue";
import SettingRow from "./SettingRow.vue";

/** Win11 个性化设置中的部分强调色，作为取色器的预设。 */
const ACCENT_SWATCHES = [
  "#0067C0",
  "#0078D4",
  "#8764B8",
  "#C239B3",
  "#E3008C",
  "#D13438",
  "#CA5010",
  "#498205",
  "#038387",
  "#567C73",
];
const THEMES: ThemeMode[] = ["system", "light", "dark"];
const SOURCES: DownloadSource[] = ["auto", "official", "bmclapi"];
const MEMORY_MIN_MB = 512;
const MEMORY_MAX_MB = 32768;

const { t, locale } = useI18n();
const message = useMessage();
const store = useSettingsStore();
const router = useRouter();

const info = ref<AppInfo | null>(null);
const javas = ref<JavaInstall[]>([]);

onMounted(async () => {
  javas.value = await mockJavaList();
  // 浏览器中调试时没有后端，版本号显示为占位符。
  info.value = await appInfo().catch(() => null);
});

const languageOptions = SUPPORTED_LOCALES.map((l) => ({ label: LOCALE_NAMES[l], value: l }));
const themeOptions = computed(() =>
  THEMES.map((value) => ({ label: t(`settings.theme.${value}`), value })),
);
const sourceOptions = computed(() =>
  SOURCES.map((value) => ({ label: t(`settings.source.${value}`), value })),
);

function showError(err: unknown) {
  message.error(formatError(toAppError(err)));
}

async function changeLanguage(value: string) {
  if (!isLocale(value)) return;
  await setLocale(value).catch(showError);
}

function save(change: (draft: Settings) => void) {
  store.update(change).catch(showError);
}

const accentMode = computed(() =>
  store.settings.appearance.accent === null ? "system" : "custom",
);
const accentModeOptions = computed(() =>
  (["system", "custom"] as const).map((value) => ({ label: t(`settings.accent.${value}`), value })),
);

function setAccentMode(mode: "system" | "custom") {
  save((s) => (s.appearance.accent = mode === "system" ? null : store.accent));
}

function setAccent(value: string) {
  if (isHexColor(value)) save((s) => (s.appearance.accent = value.toUpperCase()));
}

function setMemory(value: number | null) {
  if (value === null) return;
  save((s) => {
    s.memory.max_mb = value;
    s.memory.min_mb = Math.min(s.memory.min_mb, value);
  });
}

function setMemoryMin(value: number | null) {
  if (value !== null) save((s) => (s.memory.min_mb = Math.min(value, s.memory.max_mb)));
}

function openDataDir() {
  settingsOpenDataDir().catch(showError);
}

const exporting = ref(false);

/** 默认文件名，例如 `tmcl-diagnostics-20261030-1903.zip`。 */
function diagnosticsFileName(now = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  const date = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`;
  return `tmcl-diagnostics-${date}-${pad(now.getHours())}${pad(now.getMinutes())}.zip`;
}

async function exportDiagnostics() {
  const dest = await pickSavePath(
    [{ name: t("settings.diagnostics.filter"), extensions: ["zip"] }],
    diagnosticsFileName(),
  ).catch((err: unknown) => {
    showError(err);
    return null;
  });
  if (!dest) return;
  exporting.value = true;
  try {
    await diagnosticsExport({ dest });
    message.success(t("settings.diagnostics.done"));
  } catch (err) {
    showError(err);
  } finally {
    exporting.value = false;
  }
}

function setConcurrency(value: number | null) {
  if (value !== null) save((s) => (s.download.max_concurrency = value));
}
</script>

<template>
  <div class="settings-page">
    <h1 class="page-title">{{ t("shell.nav.settings") }}</h1>

    <section>
      <h2 class="section-title">{{ t("settings.section.appearance") }}</h2>
      <SettingRow :icon="LocalLanguage24Regular" :title="t('common.language.label')">
        <NSelect
          class="control-select"
          :value="locale"
          :options="languageOptions"
          @update:value="changeLanguage"
        />
      </SettingRow>
      <SettingRow
        :icon="DarkTheme24Regular"
        :title="t('settings.theme.label')"
        :description="t('settings.theme.description')"
      >
        <NSelect
          class="control-select"
          :value="store.settings.theme"
          :options="themeOptions"
          @update:value="(v: ThemeMode) => save((s) => (s.theme = v))"
        />
      </SettingRow>
      <SettingRow
        :icon="Color24Regular"
        :title="t('settings.accent.label')"
        :description="
          store.systemAccentColor
            ? t('settings.accent.description')
            : t('settings.accent.unavailable', { color: DEFAULT_ACCENT })
        "
      >
        <NColorPicker
          v-if="accentMode === 'custom'"
          class="control-color"
          :value="store.accent"
          :modes="['hex']"
          :show-alpha="false"
          :swatches="ACCENT_SWATCHES"
          @update:value="setAccent"
        />
        <NSelect
          class="control-select"
          :value="accentMode"
          :options="accentModeOptions"
          @update:value="setAccentMode"
        />
      </SettingRow>
      <BackgroundSettings />
    </section>

    <section>
      <h2 class="section-title">{{ t("settings.section.download") }}</h2>
      <SettingRow
        :icon="Server24Regular"
        :title="t('settings.source.label')"
        :description="t('settings.source.description')"
      >
        <NSelect
          class="control-select"
          :value="store.settings.download.source"
          :options="sourceOptions"
          @update:value="(v: DownloadSource) => save((s) => (s.download.source = v))"
        />
      </SettingRow>
      <SettingRow
        :icon="TopSpeed24Regular"
        :title="t('settings.concurrency.label')"
        :description="t('settings.concurrency.description')"
      >
        <NInputNumber
          class="control-number"
          :value="store.settings.download.max_concurrency"
          :min="1"
          :max="64"
          @update:value="setConcurrency"
        />
      </SettingRow>
    </section>

    <section>
      <h2 class="section-title">{{ t("settings.section.game") }}</h2>
      <SettingRow
        :icon="DeveloperBoard24Regular"
        :title="t('settings.memory.label')"
        :description="t('settings.memory.description')"
      >
        <NSlider
          class="control-slider"
          :value="store.settings.memory.max_mb"
          :min="MEMORY_MIN_MB"
          :max="MEMORY_MAX_MB"
          :step="512"
          :tooltip="false"
          @update:value="setMemory"
        />
        <NInputNumber
          class="control-number"
          :value="store.settings.memory.max_mb"
          :min="MEMORY_MIN_MB"
          :max="MEMORY_MAX_MB"
          :step="512"
          @update:value="setMemory"
        >
          <template #suffix>MB</template>
        </NInputNumber>
      </SettingRow>
      <SettingRow
        :icon="DeveloperBoard24Regular"
        :title="t('settings.memoryMin.label')"
        :description="t('settings.memoryMin.description')"
      >
        <NInputNumber
          class="control-number"
          :value="store.settings.memory.min_mb"
          :min="MEMORY_MIN_MB"
          :max="store.settings.memory.max_mb"
          :step="256"
          @update:value="setMemoryMin"
        >
          <template #suffix>MB</template>
        </NInputNumber>
      </SettingRow>
      <SettingRow
        :icon="DrinkCoffee24Regular"
        :title="t('settings.java.label')"
        :description="
          javas.length ? t('settings.java.found', { count: javas.length }) : t('settings.java.none')
        "
      />
      <SettingRow
        v-for="java in javas"
        :key="java.path"
        :icon="DrinkCoffee24Regular"
        :title="`Java ${java.version} · ${java.vendor} · ${java.arch}`"
        :description="java.path"
      >
        <NTag v-if="java.managed" size="small" :bordered="false">
          {{ t("settings.java.managed") }}
        </NTag>
      </SettingRow>
    </section>

    <section>
      <h2 class="section-title">{{ t("settings.section.storage") }}</h2>
      <SettingRow
        :icon="Folder24Regular"
        :title="t('settings.dataDir.label')"
        :description="store.dataDir"
      >
        <NButton :disabled="!store.dataDir" @click="openDataDir">
          <template #icon><FolderOpen24Regular /></template>
          {{ t("settings.dataDir.open") }}
        </NButton>
      </SettingRow>
      <SettingRow
        :icon="Bug24Regular"
        :title="t('settings.diagnostics.label')"
        :description="t('settings.diagnostics.description')"
      >
        <NButton :loading="exporting" @click="exportDiagnostics">
          <template #icon><ArrowDownload24Regular /></template>
          {{ t("settings.diagnostics.export") }}
        </NButton>
      </SettingRow>
    </section>

    <section>
      <h2 class="section-title">{{ t("settings.section.about") }}</h2>
      <SettingRow
        :icon="Info24Regular"
        title="TMCL"
        :description="
          info
            ? t('settings.about.version', { version: info.version, os: info.os, arch: info.arch })
            : t('settings.about.versionUnknown')
        "
      >
        <NButton @click="router.push({ name: 'about' })">{{ t("about.open") }}</NButton>
      </SettingRow>
      <div class="about-text">
        <p>{{ t("settings.about.disclaimer") }}</p>
        <p>{{ t("settings.about.license") }}</p>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-page {
  max-width: 1000px;
}

section + section {
  margin-top: 28px;
}

.section-title {
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
}

.control-select {
  width: 200px;
}

.control-number {
  width: 140px;
}

.control-slider {
  width: 180px;
}

.control-color {
  width: 120px;
}

.about-text {
  padding: 4px 16px;
  font-size: 12px;
  line-height: 18px;
  color: var(--tmcl-text-secondary);
}

.about-text p {
  margin: 8px 0;
}
</style>
