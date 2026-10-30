<script setup lang="ts">
// 实例页（PLAN.md M1-13）：实例卡片按分组展示，可按最近游玩或名称排序；
// 详情抽屉编辑配置，菜单中复制、重命名、导出、删除，工具栏导入；卡片上启动游戏并查看日志，
// 崩溃时弹出提示。拖入整合包或实例包时按内容识别类型，打开对应的安装 / 导入对话框。
import {
  Add20Regular,
  ArrowClockwise20Regular,
  ArrowImport20Regular,
  BoxMultiple20Regular,
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
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { instanceDelete, instanceList, instanceOpenDir, instanceRefresh } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import { resourcesPackDetect } from "../../api/resources";
import type { AppError } from "../../bindings/AppError";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import type { TaskId } from "../../bindings/TaskId";
import GameLogWindow from "../../components/game-log/GameLogWindow.vue";
import { useGameSessions } from "../../components/game-log/sessions";
import { formatError } from "../../i18n";
import ModpackDialog from "../resources/ModpackDialog.vue";
import { useTasksStore } from "../../stores/tasks";
import ExportDialog from "./ExportDialog.vue";
import ImportDialog from "./ImportDialog.vue";
import InstanceCard, { type CardAction } from "./InstanceCard.vue";
import InstanceDetail from "./InstanceDetail.vue";
import LaunchButton from "./LaunchButton.vue";
import { useModImport } from "./modImport";
import {
  filterInstances,
  groupInstances,
  groupNames,
  INSTANCE_SORTS,
  mostRecent,
  sortInstances,
  type InstanceSort,
} from "./instances";
import NameDialog from "./NameDialog.vue";

const { t, locale } = useI18n();
const message = useMessage();
const dialog = useDialog();
const router = useRouter();
const route = useRoute();
const games = useGameSessions();
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
const installingModpack = ref(false);
/** 拖入的文件：整合包交给整合包对话框，实例包交给导入对话框。 */
const droppedModpack = ref<string | null>(null);
const droppedPack = ref<string | null>(null);
const dragging = ref(false);
const { importMods } = useModImport();
/** 打开日志窗口的实例 ID；也可经 `?log=<id>` 打开（崩溃提示在其他页面时跳转过来）。 */
const logFor = ref<string | null>(null);

watch(
  () => route.query.log,
  (id) => {
    if (typeof id === "string" && games.sessions[id]) logFor.value = id;
  },
  { immediate: true },
);

function closeLog() {
  logFor.value = null;
  if (route.query.log) void router.replace({ query: {} });
}

// 对话框挂在应用级的 provider 上，离开本页后仍能弹出；“查看日志”经路由回到本页。
games.setCrashHandler((session) => {
  dialog.error({
    title: t("instances.crash.title", { name: session.name }),
    content: t("instances.crash.content", { code: session.exitCode ?? "-" }),
    positiveText: t("instances.crash.viewLog"),
    negativeText: t("instances.crash.close"),
    onPositiveClick: () => {
      logFor.value = session.instanceId;
      void router.push({ name: "instances", query: { log: session.instanceId } });
    },
  });
});

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

/** 识别拖入的文件并打开对应对话框；识别失败时提示原因。 */
async function onDrop(paths: string[]) {
  const [path] = paths;
  if (!path) return;
  if (paths.length > 1) message.info(t("instances.drop.multiple"));
  const file = path.split(/[\\/]/).pop() ?? path;
  try {
    switch (await resourcesPackDetect({ path })) {
      case "mrpack":
      case "curseforge":
        droppedModpack.value = path;
        installingModpack.value = true;
        break;
      case "tmclpack":
        droppedPack.value = path;
        importing.value = true;
        break;
      case "multimc":
        message.warning(t("instances.drop.multimc", { file }), { duration: 6000 });
        break;
      case "unknown":
        message.error(t("instances.drop.unknown", { file }), { duration: 6000 });
        break;
    }
  } catch (err) {
    showError(err);
  }
}

let unlistenDrop: (() => void) | null = null;
let unmounted = false;

onMounted(async () => {
  void load();
  try {
    const unlisten = await getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter" || payload.type === "over") dragging.value = true;
      else if (payload.type === "leave") dragging.value = false;
      else {
        dragging.value = false;
        void onDrop(payload.paths);
      }
    });
    if (unmounted) unlisten();
    else unlistenDrop = unlisten;
  } catch {
    // 不在 Tauri 中（例如浏览器调试）时没有拖放事件。
  }
});

onBeforeUnmount(() => {
  unmounted = true;
  unlistenDrop?.();
});

const groups = computed(() =>
  groupInstances(
    sortInstances(filterInstances(instances.value, query.value), sort.value, locale.value),
    locale.value,
  ),
);
const existingGroups = computed(() => groupNames(instances.value, locale.value));
const existingNames = computed(() => instances.value.map((i) => i.name));
const recent = computed(() => mostRecent(instances.value));
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
    case "resources":
      void router.push({ name: "instance-resources", params: { id: instance.id } });
      break;
    case "open":
      instanceOpenDir({ id: instance.id }).catch(showError);
      break;
    case "importMods":
      void importMods(instance.id);
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
      <NButton secondary @click="((droppedPack = null), (importing = true))">
        <template #icon
          ><NIcon><ArrowImport20Regular /></NIcon
        ></template>
        {{ t("instances.import.action") }}
      </NButton>
      <NButton secondary @click="((droppedModpack = null), (installingModpack = true))">
        <template #icon
          ><NIcon><BoxMultiple20Regular /></NIcon
        ></template>
        {{ t("resources.modpack.action") }}
      </NButton>
      <LaunchButton
        v-if="recent"
        :instance="recent"
        primary
        @launched="load()"
        @log="(id: string) => (logFor = id)"
      />
      <NButton :type="recent ? 'default' : 'primary'" @click="router.push({ name: 'download' })">
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
          >
            <template v-if="!instance.error" #actions>
              <LaunchButton
                :instance="instance"
                @launched="load()"
                @log="(id: string) => (logFor = id)"
              />
            </template>
          </InstanceCard>
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
    <GameLogWindow :instance-id="logFor" @close="closeLog" />
    <ImportDialog
      :show="importing"
      :path="droppedPack"
      @close="importing = false"
      @started="reloadAfter"
    />
    <ModpackDialog
      :show="installingModpack"
      :file="droppedModpack"
      @close="installingModpack = false"
      @started="reloadAfter"
    />

    <div v-if="dragging" class="drop-overlay">
      <NIcon size="40"><BoxMultiple20Regular /></NIcon>
      <span class="drop-title">{{ t("instances.drop.title") }}</span>
      <span class="drop-hint">{{ t("instances.drop.hint") }}</span>
    </div>
  </div>
</template>

<style scoped>
.instances-page {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 1200px;
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
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

.drop-overlay {
  position: absolute;
  inset: 0;
  z-index: 10;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border: 2px dashed var(--tmcl-accent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--tmcl-card) 85%, transparent);
  color: var(--tmcl-accent);
  pointer-events: none;
}

.drop-title {
  font-size: 16px;
  font-weight: 600;
}

.drop-hint {
  color: var(--tmcl-text-secondary);
}
</style>
