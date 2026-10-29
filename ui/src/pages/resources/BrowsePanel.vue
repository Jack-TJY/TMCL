<script setup lang="ts">
// 浏览 Modrinth：按实例的游戏版本与加载器搜索某类资源，点“安装”打开安装对话框。
// 不指定实例时搜索全部资源，并在每一项上标出支持的游戏版本与所需的加载器。
import { Search20Regular } from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NEmpty,
  NIcon,
  NInput,
  NPagination,
  NSelect,
  NSpin,
  NTag,
} from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import {
  resourcesSearch,
  resourcesSearchAny,
  type ResourceHit,
  type ResourceKind,
  type ResourceSort,
} from "../../api/resources";
import type { AppError } from "../../bindings/AppError";
import { formatError } from "../../i18n";
import ResourceIcon from "./ResourceIcon.vue";
import { formatCount, PAGE_SIZE, RESOURCE_SORTS } from "./resources";
import { hitLoaders, loaderName, versionSummary } from "./targets";

const props = defineProps<{
  /** 按该实例过滤；`null` 表示不限实例。 */
  instanceId: string | null;
  kind: ResourceKind;
  /** 已安装项目的 Modrinth ID，用于标出已安装。 */
  installedIds: ReadonlySet<string>;
}>();
const emit = defineEmits<{ install: [hit: ResourceHit] }>();

const { t, locale } = useI18n();

const query = ref("");
const sort = ref<ResourceSort>("relevance");
const page = ref(1);
const hits = ref<ResourceHit[]>([]);
const total = ref(0);
const loading = ref(false);
const error = ref<AppError | null>(null);
/** 只保留最后一次搜索的结果。 */
let seq = 0;

async function search() {
  const current = ++seq;
  loading.value = true;
  error.value = null;
  try {
    const args = {
      kind: props.kind,
      query: query.value.trim(),
      sort: sort.value,
      offset: (page.value - 1) * PAGE_SIZE,
      limit: PAGE_SIZE,
    };
    const result = props.instanceId
      ? await resourcesSearch({ instance_id: props.instanceId, ...args })
      : await resourcesSearchAny(args);
    if (current !== seq) return;
    hits.value = result.hits;
    total.value = result.total_hits;
  } catch (err) {
    if (current === seq) error.value = toAppError(err);
  } finally {
    if (current === seq) loading.value = false;
  }
}

let timer: ReturnType<typeof setTimeout> | undefined;
watch(query, () => {
  clearTimeout(timer);
  timer = setTimeout(() => {
    if (page.value === 1) void search();
    else page.value = 1;
  }, 350);
});
watch(sort, () => {
  if (page.value === 1) void search();
  else page.value = 1;
});
watch(page, () => search());
watch(
  () => [props.instanceId, props.kind],
  () => {
    hits.value = [];
    if (page.value === 1) void search();
    else page.value = 1;
  },
  { immediate: true },
);

/** 不限实例时显示的“支持的游戏版本”。 */
function supportText(hit: ResourceHit): string {
  const { latest, total } = versionSummary(hit.game_versions);
  if (total === 0) return "";
  const versions = latest.join(t("resources.browse.separator"));
  return total > latest.length
    ? t("resources.browse.supportsMore", { versions, count: total })
    : t("resources.browse.supports", { versions });
}

const pageCount = computed(() => Math.max(1, Math.ceil(total.value / PAGE_SIZE)));
const sortOptions = computed(() =>
  RESOURCE_SORTS.map((value) => ({ label: t(`resources.browse.sort.${value}`), value })),
);
</script>

<template>
  <div class="browse">
    <div class="toolbar">
      <NInput
        v-model:value="query"
        class="search"
        clearable
        :placeholder="t('resources.browse.search')"
      >
        <template #prefix
          ><NIcon><Search20Regular /></NIcon
        ></template>
      </NInput>
      <NSelect v-model:value="sort" class="sort" :options="sortOptions" />
    </div>

    <NAlert v-if="error" type="error" class="error" :title="t('resources.browse.failed')">
      <div class="error-body">
        <span>{{ formatError(error) }}</span>
        <NButton size="small" @click="search">{{ t("resources.browse.retry") }}</NButton>
      </div>
    </NAlert>

    <NSpin :show="loading" class="list">
      <template v-if="!loading && !error && hits.length === 0">
        <slot name="empty">
          <NEmpty class="empty" :description="t('resources.browse.empty')" />
        </slot>
      </template>
      <div v-for="hit in hits" :key="hit.project_id" class="hit">
        <ResourceIcon :url="hit.icon_url" :size="44" />
        <div class="hit-body">
          <div class="hit-head">
            <span class="hit-title">{{ hit.title }}</span>
            <span class="hit-meta">{{ t("resources.browse.by", { author: hit.author }) }}</span>
          </div>
          <div class="hit-desc" :title="hit.description">{{ hit.description }}</div>
          <div class="hit-meta">
            {{ t("resources.browse.downloads", { count: formatCount(hit.downloads, locale) }) }}
            <template v-if="instanceId === null && supportText(hit)">
              · {{ supportText(hit) }}
            </template>
          </div>
          <div v-if="instanceId === null && hitLoaders(hit, kind).length" class="hit-tags">
            <NTag
              v-for="loader in hitLoaders(hit, kind)"
              :key="loader"
              size="tiny"
              round
              :bordered="false"
            >
              {{ t("resources.browse.requires", { loader: loaderName(loader) }) }}
            </NTag>
          </div>
        </div>
        <NTag v-if="installedIds.has(hit.project_id)" size="small" round :bordered="false">
          {{ t("resources.browse.installed") }}
        </NTag>
        <NButton size="small" type="primary" secondary @click="emit('install', hit)">
          {{ t("resources.browse.install") }}
        </NButton>
      </div>
    </NSpin>

    <div class="footer">
      <span class="credit">{{ t("resources.credit") }}</span>
      <NPagination v-if="pageCount > 1" v-model:page="page" :page-count="pageCount" simple />
    </div>
  </div>
</template>

<style scoped>
.browse {
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 100%;
  min-height: 0;
}

.toolbar {
  display: flex;
  gap: 8px;
}

.search {
  flex: 1;
  max-width: 360px;
}

.sort {
  width: 140px;
}

.error-body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.empty {
  padding: 48px 0;
}

.hit {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 8px;
}

.hit:hover {
  background: var(--tmcl-subtle);
}

.hit-body {
  flex: 1;
  min-width: 0;
}

.hit-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.hit-title {
  font-weight: 600;
}

.hit-desc {
  overflow: hidden;
  font-size: 13px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.hit-meta {
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.hit-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 4px;
}

.footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.credit {
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}
</style>
