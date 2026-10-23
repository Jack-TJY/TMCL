<script setup lang="ts">
// 实例页（PLAN.md M1-13）：实例卡片按分组展示，可按最近游玩或名称排序；
// 详情抽屉编辑配置，菜单中复制、重命名、导出、删除，工具栏导入。
import {
  Add20Regular,
  ArrowClockwise20Regular,
  ArrowImport20Regular,
  Search20Regular,
} from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NEmpty,
  NIcon,
  NInput,
  NSelect,
  NSpin,
  useDialog,
  useMessage,
} from "naive-ui";
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { instanceDelete, instanceList, instanceOpenDir, instanceRefresh } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import type { AppError } from "../../bindings/AppError";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import type { TaskId } from "../../bindings/TaskId";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import ExportDialog from "./ExportDialog.vue";
import ImportDialog from "./ImportDialog.vue";
import InstanceCard, { type CardAction } from "./InstanceCard.vue";
import InstanceDetail from "./InstanceDetail.vue";
import {
  filterInstances,
  groupInstances,
  groupNames,
  INSTANCE_SORTS,
  sortInstances,
  type InstanceSort,
} from "./instances";
import NameDialog from "./NameDialog.vue";

const { t, locale } = useI18n();
const message = useMessage();
const dialog = useDialog();
const router = useRouter();
const tasks = useTasksStore();

const instances = ref<InstanceSummary[]>([]);
const loading = ref(false);
const error = ref<AppError | null>(null);
const query = ref("");
const sort = ref<InstanceSort>("recent");

const detail = ref<InstanceSummary | null>(null);
const naming = ref<{ mode: "rename" | "duplicate"; instance: InstanceSummary } | null>(null);
const exporting = ref<InstanceSummary | null>(null);
const importing = ref(false);

async function load(reconcile = false) {
  loading.value = true;
  error.value = null;
  try {
    instances.value = (await (reconcile ? instanceRefresh() : instanceList())).instances;
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    loading.value = false;
  }
}

onMounted(() => load());

const groups = computed(() =>
  groupInstances(
    sortInstances(filterInstances(instances.value, query.value), sort.value, locale.value),
    locale.value,
  ),
);
const existingGroups = computed(() => groupNames(instances.value, locale.value));
const existingNames = computed(() => instances.value.map((i) => i.name));
const sortOptions = computed(() =>
  INSTANCE_SORTS.map((value) => ({ label: t(`instances.sort.${value}`), value })),
);

function showError(err: unknown) {
  message.error(formatError(toAppError(err)));
}

/** 任务成功结束后重新读取列表（复制、导入会产生新实例）。 */
function reloadAfter(taskId: TaskId) {
  const status = () => tasks.entries.find((e) => e.task_id === taskId)?.status;
  const settle = (value: ReturnType<typeof status>) => {
    if (!value || value === "running") return false;
    if (value === "succeeded") void load();
    return true;
  };
  if (settle(status())) return;
  const stop = watch(status, (value) => {
    if (settle(value)) stop();
  });
}

function confirmDelete(instance: InstanceSummary) {
  dialog.warning({
    title: t("instances.delete.title", { name: instance.name }),
    content: t("instances.delete.content"),
    positiveText: t("instances.delete.confirm"),
    negativeText: t("instances.cancel"),
    onPositiveClick: async () => {
      try {
        await instanceDelete({ id: instance.id });
        if (detail.value?.id === instance.id) detail.value = null;
        message.success(t("instances.delete.done", { name: instance.name }));
        await load();
      } catch (err) {
        showError(err);
      }
    },
  });
}

function onAction(instance: InstanceSummary, action: CardAction) {
  switch (action) {
    case "details":
      detail.value = instance;
      break;
    case "open":
      instanceOpenDir({ id: instance.id }).catch(showError);
      break;
    case "rename":
    case "duplicate":
      naming.value = { mode: action, instance };
      break;
    case "export":
      exporting.value = instance;
      break;
    case "delete":
      confirmDelete(instance);
      break;
  }
}
</script>

<template>
  <div class="instances-page">
    <h1 class="page-title">{{ t("shell.nav.instances") }}</h1>

    <div class="toolbar">
      <NInput v-model:value="query" class="search" clearable :placeholder="t('instances.search')">
        <template #prefix
          ><NIcon><Search20Regular /></NIcon
        ></template>
      </NInput>
      <NSelect v-model:value="sort" class="sort" :options="sortOptions" />
      <div class="spacer" />
      <NButton quaternary :loading="loading" @click="load(true)">
        <template #icon
          ><NIcon><ArrowClockwise20Regular /></NIcon
        ></template>
        {{ t("instances.refresh") }}
      </NButton>
      <NButton secondary @click="importing = true">
        <template #icon
          ><NIcon><ArrowImport20Regular /></NIcon
        ></template>
        {{ t("instances.import.action") }}
      </NButton>
      <NButton type="primary" @click="router.push({ name: 'download' })">
        <template #icon
          ><NIcon><Add20Regular /></NIcon
        ></template>
        {{ t("instances.create") }}
      </NButton>
    </div>

    <NAlert v-if="error" type="error" class="load-error" :title="t('instances.loadFailed')">
      <div class="load-error-body">
        <span>{{ formatError(error) }}</span>
        <NButton size="small" @click="load()">{{ t("instances.retry") }}</NButton>
      </div>
    </NAlert>

    <NSpin :show="loading && instances.length === 0" class="list">
      <NEmpty
        v-if="!loading && !error && groups.length === 0"
        class="empty"
        :description="instances.length === 0 ? t('instances.empty') : t('instances.noMatch')"
      >
        <template v-if="instances.length === 0" #extra>
          <NButton size="small" @click="router.push({ name: 'download' })">
            {{ t("instances.create") }}
          </NButton>
        </template>
      </NEmpty>
      <section v-for="group in groups" :key="group.name ?? ''" class="group">
        <h2 class="group-title">
          {{ group.name ?? t("instances.ungrouped") }}
          <span class="group-count">{{ group.instances.length }}</span>
        </h2>
        <div class="cards">
          <InstanceCard
            v-for="instance in group.instances"
            :key="instance.id"
            :instance="instance"
            @action="(action: CardAction) => onAction(instance, action)"
          />
        </div>
      </section>
    </NSpin>

    <InstanceDetail
      :instance="detail"
      :groups="existingGroups"
      @close="detail = null"
      @saved="load()"
      @open-dir="(id: string) => instanceOpenDir({ id }).catch(showError)"
    />
    <NameDialog
      :request="naming"
      :existing="existingNames"
      @close="naming = null"
      @renamed="load()"
      @started="reloadAfter"
    />
    <ExportDialog :instance="exporting" @close="exporting = null" />
    <ImportDialog :show="importing" @close="importing = false" @started="reloadAfter" />
  </div>
</template>

<style scoped>
.instances-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 1200px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.search {
  flex: 1;
  max-width: 320px;
}

.sort {
  width: 140px;
}

.spacer {
  flex: 1;
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

.list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.empty {
  padding: 48px 0;
}

.group + .group {
  margin-top: 20px;
}

.group-title {
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
}

.group-count {
  margin-left: 6px;
  font-weight: 400;
  color: var(--tmcl-text-secondary);
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 8px;
}
</style>
