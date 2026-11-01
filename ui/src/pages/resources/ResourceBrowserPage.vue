<script setup lang="ts">
// 资源下载（左侧主导航）：浏览 Mod、资源包与光影，可选一个目标实例。
// 选了实例时只显示适用于它的资源，不能安装时说明原因；不选实例时显示全部资源并标出
// 支持的游戏版本与加载器，安装前再选择实例。
import { NButton, NEmpty, NSelect, NTab, NTabs, useMessage } from "naive-ui";
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { instanceGet, instanceList } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import { resourcesInstalled, type ResourceHit, type ResourceKind } from "../../api/resources";
import type { InstanceConfig } from "../../bindings/InstanceConfig";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import type { TaskId } from "../../bindings/TaskId";
import LoaderChangeDialog from "../../components/loader-picker/LoaderChangeDialog.vue";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import BrowsePanel from "./BrowsePanel.vue";
import CreateInstanceDialog from "./CreateInstanceDialog.vue";
import InstallDialog from "./InstallDialog.vue";
import PickInstanceDialog from "./PickInstanceDialog.vue";
import { BROWSER_KINDS, loaderName } from "./targets";
import { autoInstall } from "./autoInstall";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const message = useMessage();
const tasks = useTasksStore();

const instances = ref<InstanceSummary[]>([]);
const instancesLoaded = ref(false);
const kind = ref<ResourceKind>(
  BROWSER_KINDS.find((k) => k === route.query.kind) ?? BROWSER_KINDS[0],
);
const selectedId = ref<string | null>(
  typeof route.query.instance === "string" ? route.query.instance : null,
);
const selected = computed(() => instances.value.find((i) => i.id === selectedId.value) ?? null);

async function loadInstances() {
  try {
    const list = await instanceList();
    instances.value = list.instances.filter((i) => i.error === null);
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    instancesLoaded.value = true;
  }
  if (selectedId.value && !selected.value) selectedId.value = null;
}
void loadInstances();

watch([selectedId, kind], ([instance, k]) => {
  void router.replace({ query: { ...(instance ? { instance } : {}), kind: k } });
});

/** 实例的“版本 · 加载器”描述。 */
function targetText(instance: InstanceSummary): string {
  const loader = instance.loader ? loaderName(instance.loader.kind) : t("resources.global.vanilla");
  return `${instance.game_version} · ${loader}`;
}

const instanceOptions = computed(() =>
  instances.value.map((i) => ({ label: `${i.name}（${targetText(i)}）`, value: i.id })),
);

/**
 * 选中的实例装不了这类资源时的原因文案 key：原版实例不能装 Mod，也加载不了光影
 * （光影要靠 Iris / Oculus 等 Mod）。资源包任何实例都能装。
 */
const blockedReason = computed(() => {
  if (!selected.value || selected.value.loader) return null;
  if (kind.value === "mod") return "resources.global.modNoLoader";
  if (kind.value === "shader") return "resources.global.shaderNoLoader";
  return null;
});
const blocked = computed(() => blockedReason.value !== null);

const installedIds = ref<ReadonlySet<string>>(new Set());
async function loadInstalled() {
  const instance = selected.value;
  installedIds.value = new Set();
  if (!instance || blocked.value) return;
  const list = await resourcesInstalled({ instance_id: instance.id, kind: kind.value }).catch(
    () => null,
  );
  if (selected.value !== instance || !list) return;
  installedIds.value = new Set(
    list.resources.flatMap((r) => (r.source ? [r.source.project_id] : [])),
  );
}
watch([selected, kind], () => void loadInstalled(), { immediate: true });

/** 正在选择安装目标的资源（未选实例时）。 */
const picking = ref<ResourceHit | null>(null);
/** 安装对话框的资源与目标实例。 */
const installing = ref<{ hit: ResourceHit; instance: InstanceSummary } | null>(null);

function install(hit: ResourceHit) {
  if (selected.value) installing.value = { hit, instance: selected.value };
  else picking.value = hit;
}

function picked(instance: InstanceSummary) {
  const hit = picking.value;
  picking.value = null;
  if (hit) installing.value = { hit, instance };
}

/**
 * 为资源一键新建实例；实例的下载任务成功后自动安装该资源（默认版本）。
 * 后续动作登记在任务中心，离开本页也会执行；需要用户选择时（冲突、缺少前置、
 * 没有适用版本）本页还在就打开安装对话框，否则提示到资源页手动安装。
 */
const creating = ref<ResourceHit | null>(null);
let mounted = true;
onBeforeUnmount(() => (mounted = false));

function created(taskId: TaskId, instance: InstanceConfig) {
  const hit = creating.value;
  const resourceKind = kind.value;
  void loadInstances();
  if (!hit) return;
  tasks.afterSuccess(taskId, () => {
    void (async () => {
      const result = await autoInstall(
        { hit, kind: resourceKind, instanceId: instance.id, instanceName: instance.name },
        tasks.start,
      );
      if (result.state === "started") {
        message.info(t("resources.create.ready", { name: instance.name, title: hit.title }));
        if (mounted) watched.value.push(result.taskId);
        return;
      }
      if (result.state === "failed") message.error(formatError(result.error));
      if (mounted) {
        await loadInstances();
        const summary = instances.value.find((i) => i.id === instance.id);
        if (summary) installing.value = { hit, instance: summary };
      } else {
        message.warning(t("resources.create.manual", { name: instance.name, title: hit.title }));
      }
    })();
  });
}

function startCreate() {
  creating.value = picking.value;
  picking.value = null;
}

/** 后装加载器：成功后重新读取实例列表。 */
const loaderTarget = ref<InstanceConfig | null>(null);
async function openLoaderDialog() {
  if (!selected.value) return;
  try {
    loaderTarget.value = await instanceGet({ id: selected.value.id });
  } catch (err) {
    message.error(formatError(toAppError(err)));
  }
}

/**
 * 本页发起的安装、加载器任务；成功后重新读取实例与已安装列表。
 * 只在有任务结束时才改写 `watched`：getter 每次返回新数组，回调里无条件赋值会让
 * 侦听器反复触发自己，界面卡死（beta.1 冒烟 R2）。
 */
const watched = ref<TaskId[]>([]);
watch(
  () => watched.value.map((id) => tasks.entries.find((e) => e.task_id === id)?.status ?? "running"),
  (statuses) => {
    if (statuses.every((s) => s === "running")) return;
    if (statuses.some((s) => s === "succeeded")) {
      void loadInstances().then(loadInstalled);
    }
    watched.value = watched.value.filter((_, i) => statuses[i] === "running");
  },
);
</script>

<template>
  <div class="browser-page">
    <h1 class="page-title">{{ t("resources.global.title") }}</h1>

    <div class="target">
      <span class="target-label">{{ t("resources.global.target") }}</span>
      <NSelect
        v-model:value="selectedId"
        class="target-select"
        clearable
        filterable
        :loading="!instancesLoaded"
        :options="instanceOptions"
        :placeholder="t('resources.global.anyInstance')"
      />
    </div>
    <div class="target-hint">
      {{
        selected
          ? t("resources.global.filtered", { target: targetText(selected) })
          : t("resources.global.unfiltered")
      }}
    </div>

    <NTabs v-model:value="kind" type="line" class="kinds">
      <NTab v-for="k in BROWSER_KINDS" :key="k" :name="k">
        {{ t(`resources.kind.${k}`) }}
      </NTab>
    </NTabs>

    <NEmpty
      v-if="blocked"
      class="blocked"
      :description="blockedReason ? t(blockedReason, { name: selected?.name }) : ''"
    >
      <template #extra>
        <div class="actions">
          <NButton type="primary" @click="openLoaderDialog">
            {{ t("resources.global.installLoader") }}
          </NButton>
          <NButton @click="selectedId = null">{{ t("resources.global.showAll") }}</NButton>
        </div>
      </template>
    </NEmpty>
    <BrowsePanel
      v-else
      class="panel"
      :instance-id="selectedId"
      :kind="kind"
      :installed-ids="installedIds"
      @install="install"
    >
      <template v-if="selected" #empty>
        <NEmpty
          class="blocked"
          :description="
            t('resources.global.noneFor', {
              kind: t(`resources.kind.${kind}`),
              target: targetText(selected),
            })
          "
        >
          <template #extra>
            <NButton @click="selectedId = null">{{ t("resources.global.showAll") }}</NButton>
          </template>
        </NEmpty>
      </template>
    </BrowsePanel>

    <PickInstanceDialog
      :hit="picking"
      :kind="kind"
      :instances="instances"
      @close="picking = null"
      @pick="picked"
      @create="startCreate"
    />
    <CreateInstanceDialog
      :hit="creating"
      :kind="kind"
      @close="creating = null"
      @started="created"
    />
    <InstallDialog
      :hit="installing?.hit ?? null"
      :instance-id="installing?.instance.id ?? ''"
      :instance-name="installing?.instance.name ?? ''"
      :kind="kind"
      :world="null"
      @close="installing = null"
      @started="(id: TaskId) => watched.push(id)"
    />
    <LoaderChangeDialog
      :instance="loaderTarget"
      @close="loaderTarget = null"
      @saved="() => void loadInstances()"
      @started="(id: TaskId) => watched.push(id)"
    />
  </div>
</template>

<style scoped>
.browser-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 1200px;
}

.target {
  display: flex;
  align-items: center;
  gap: 12px;
}

.target-label {
  font-size: 13px;
}

.target-select {
  width: 360px;
}

.target-hint {
  margin-top: 4px;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.kinds {
  margin: 8px 0 12px;
}

.blocked {
  padding: 48px 0;
}

.actions {
  display: flex;
  gap: 8px;
  justify-content: center;
}

.panel {
  flex: 1;
  min-height: 0;
}
</style>
