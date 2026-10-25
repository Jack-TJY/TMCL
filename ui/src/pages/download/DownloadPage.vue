<script setup lang="ts">
// 下载页（PLAN.md M1-12）：游戏版本列表（类型筛选、搜索），选择后新建实例并下载。
// 页脚按下载源标明 BMCLAPI 来源（BMCLAPI 使用协议要求，见 docs/research/mirrors.md）。
import { ArrowClockwise20Regular, Search20Regular } from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NDataTable,
  NIcon,
  NInput,
  NRadioButton,
  NRadioGroup,
  NTag,
  type DataTableColumns,
} from "naive-ui";
import { computed, h, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { gameVersionList } from "../../api/game";
import { toAppError } from "../../api/invoke";
import type { AppError } from "../../bindings/AppError";
import type { GameVersion } from "../../bindings/GameVersion";
import type { GameVersionList } from "../../bindings/GameVersionList";
import { formatError } from "../../i18n";
import { useSettingsStore } from "../../stores/settings";
import InstallDialog from "./InstallDialog.vue";
import { filterOf, filterVersions, VERSION_FILTERS, type VersionFilter } from "./versions";

const { t, locale } = useI18n();
const settings = useSettingsStore();

const list = ref<GameVersionList | null>(null);
const loading = ref(false);
const error = ref<AppError | null>(null);
const filter = ref<VersionFilter>("release");
const query = ref("");
const selected = ref<GameVersion | null>(null);

/** 读取版本列表；`refresh` 为真（点“刷新”）时强制联网。 */
async function load(refresh = false) {
  loading.value = true;
  error.value = null;
  try {
    list.value = await gameVersionList(refresh ? { refresh } : undefined);
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    loading.value = false;
  }
}

onMounted(() => load());

const counts = computed(() => {
  const result: Record<VersionFilter, number> = { release: 0, snapshot: 0, old: 0 };
  for (const v of list.value?.versions ?? []) result[filterOf(v)] += 1;
  return result;
});

const rows = computed(() => filterVersions(list.value?.versions ?? [], filter.value, query.value));

const latest = computed(
  () => new Set([list.value?.latest_release, list.value?.latest_snapshot].filter(Boolean)),
);

function formatDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleDateString(locale.value);
}

const columns = computed<DataTableColumns<GameVersion>>(() => [
  {
    key: "id",
    title: t("download.column.version"),
    render: (row) => [
      row.id,
      latest.value.has(row.id)
        ? h(NTag, { size: "small", type: "success", bordered: false, class: "latest-tag" }, () =>
            t("download.latest"),
          )
        : null,
    ],
  },
  {
    key: "release_time",
    title: t("download.column.released"),
    width: 160,
    render: (row) => formatDate(row.release_time),
  },
  {
    key: "action",
    title: "",
    width: 110,
    align: "right",
    render: (row) =>
      h(NButton, { size: "small", secondary: true, onClick: () => (selected.value = row) }, () =>
        t("download.install.action"),
      ),
  },
]);

const sourceNote = computed(() => {
  const source = settings.settings.download.source;
  return t(`download.source.${source}`);
});
</script>

<template>
  <div class="download-page">
    <h1 class="page-title">{{ t("shell.nav.download") }}</h1>

    <div class="toolbar">
      <NRadioGroup v-model:value="filter">
        <NRadioButton v-for="f in VERSION_FILTERS" :key="f" :value="f">
          {{ t(`download.kind.${f}`) }}
          <span v-if="list" class="filter-count">{{ counts[f] }}</span>
        </NRadioButton>
      </NRadioGroup>
      <NInput v-model:value="query" class="search" clearable :placeholder="t('download.search')">
        <template #prefix
          ><NIcon><Search20Regular /></NIcon
        ></template>
      </NInput>
      <NButton
        quaternary
        :loading="loading"
        :aria-label="t('download.refresh')"
        @click="load(true)"
      >
        <template #icon
          ><NIcon><ArrowClockwise20Regular /></NIcon
        ></template>
        {{ t("download.refresh") }}
      </NButton>
    </div>

    <NAlert v-if="error" type="error" class="load-error" :title="t('download.loadFailed')">
      <div class="load-error-body">
        <span>{{ formatError(error) }}</span>
        <NButton size="small" @click="load(true)">{{ t("download.retry") }}</NButton>
      </div>
    </NAlert>

    <NAlert
      v-else-if="list?.stale"
      type="warning"
      class="load-error"
      :title="t('download.staleTitle')"
    >
      <div class="load-error-body">
        <span>{{ formatError(list.stale) }}</span>
        <NButton size="small" :loading="loading" @click="load(true)">
          {{ t("download.retry") }}
        </NButton>
      </div>
    </NAlert>

    <NDataTable
      class="version-table"
      :columns="columns"
      :data="rows"
      :loading="loading"
      :row-key="(row: GameVersion) => row.id"
      :bordered="false"
      flex-height
      virtual-scroll
      size="small"
    />

    <p class="source-note">{{ sourceNote }}</p>

    <InstallDialog :version="selected" @close="selected = null" />
  </div>
</template>

<style scoped>
.download-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 1000px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.search {
  flex: 1;
  max-width: 320px;
}

.filter-count {
  margin-left: 6px;
  font-size: 12px;
  opacity: 0.6;
}

.load-error {
  margin-bottom: 12px;
}

.load-error-body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.version-table {
  flex: 1;
  min-height: 240px;
  border-radius: 8px;
  overflow: hidden;
}

.version-table :deep(.latest-tag) {
  margin-left: 8px;
}

.source-note {
  margin: 12px 0 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--tmcl-text-secondary);
}
</style>
