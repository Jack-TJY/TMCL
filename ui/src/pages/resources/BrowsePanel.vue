<script setup lang="ts">
// 浏览 Modrinth 与 CurseForge（构建带 CurseForge Key 时）：按实例的游戏版本与加载器合并搜索
// 某类资源，每项标出来源平台，点“安装”打开安装对话框。
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
  resourcesSources,
  type ResourceHit,
  type ResourceKind,
  type ResourcePlatform,
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
  /** 已安装项目的 Modrinth ID，用于标出已安装（合并条目也比较其 Modrinth 条目）。 */
  installedIds: ReadonlySet<string>;
}>();
const emit = defineEmits<{ install: [hit: ResourceHit] }>();

const { t, locale } = useI18n();

const query = ref("");
const sort = ref<ResourceSort>("relevance");
const page = ref(1);
/** 平台过滤：`all` 为合并显示。 */
const platform = ref<ResourcePlatform | "all">("all");
const curseforge = ref(false);
const failed = ref<ResourcePlatform[]>([]);
resourcesSources()
  .then((sources) => (curseforge.value = sources.curseforge))
  .catch(() => (curseforge.value = false));
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
      platforms: platform.value === "all" ? undefined : [platform.value],
    };
    const result = props.instanceId
      ? await resourcesSearch({ instance_id: props.instanceId, ...args })
      : await resourcesSearchAny(args);
    if (current !== seq) return;
    hits.value = result.hits;
    total.value = result.total_hits;
    failed.value = result.failed;
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
watch([sort, platform], () => {
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

/** 本条及并入它的其他平台条目所在的平台。 */
function platformsOf(hit: ResourceHit): ResourcePlatform[] {
  return [hit.platform, ...hit.also_on.map((r) => r.platform)];
}

function isInstalled(hit: ResourceHit): boolean {
  return (
    props.installedIds.has(hit.project_id) ||
    hit.also_on.some((r) => props.installedIds.has(r.project_id))
  );
}

const failedText = computed(() =>
  failed.value
    .map((p) =>
      t("resources.browse.platformFailed", { platform: t(`resources.browse.platform.${p}`) }),
    )
    .join(" "),
);
const platformOptions = computed(() =>
  (["all", "modrinth", "curseforge"] as const).map((value) => ({
    label: t(`resources.browse.platform.${value}`),
    value,
  })),
);

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
        :placeholder="t(curseforge ? 'resources.browse.searchAll' : 'resources.browse.search')"
      >
        <template #prefix
          ><NIcon><Search20Regular /></NIcon
        ></template>
      </NInput>
      <NSelect v-if="curseforge" v-model:value="platform" class="sort" :options="platformOptions" />
      <NSelect v-model:value="sort" class="sort" :options="sortOptions" />
    </div>

    <NAlert v-if="failedText && !error" type="warning" :show-icon="false" class="error">
      {{ failedText }}
    </NAlert>

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
      <div v-for="hit in hits" :key="`${hit.platform}:${hit.project_id}`" class="hit">
        <ResourceIcon :url="hit.icon_url" :size="44" />
        <div class="hit-body">
          <div class="hit-head">
            <span class="hit-title">{{ hit.title }}</span>
            <span class="hit-meta">{{ t("resources.browse.by", { author: hit.author }) }}</span>
            <template v-if="curseforge">
              <NTag
                v-for="p in platformsOf(hit)"
                :key="p"
                size="tiny"
                round
                :bordered="false"
                :type="p === 'curseforge' ? 'warning' : 'success'"
              >
                {{ t(`resources.browse.platform.${p}`) }}
              </NTag>
            </template>
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
        <NTag v-if="isInstalled(hit)" size="small" round :bordered="false">
          {{ t("resources.browse.installed") }}
        </NTag>
        <NButton size="small" type="primary" secondary @click="emit('install', hit)">
          {{ t("resources.browse.install") }}
        </NButton>
      </div>
    </NSpin>

    <div class="footer">
      <span class="credit">{{ t(curseforge ? "resources.creditBoth" : "resources.credit") }}</span>
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
  flex-wrap: wrap;
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
