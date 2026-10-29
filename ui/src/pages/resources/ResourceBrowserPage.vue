<script setup lang="ts">
// 资源下载（左侧主导航）：浏览 Mod、资源包与光影，可选一个目标实例。
// 选了实例时只显示适用于它的资源，不能安装时说明原因；不选实例时显示全部资源并标出
// 支持的游戏版本与加载器，安装前再选择实例。
import { NAlert, NButton, NEmpty, NSelect, NTab, NTabs, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
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
import InstallDialog from "./InstallDialog.vue";
import PickInstanceDialog from "./PickInstanceDialog.vue";
import { BROWSER_KINDS, loaderName } from "./targets";

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

/** 选中的实例装不了这类资源：原版实例不能装 Mod。 */
const blocked = computed(
  () => selected.value !== null && kind.value === "mod" && !selected.value.loader,
);
/** 原版实例上的光影需要先装加载器与 Iris 等 Mod，只提示不拦截。 */
const shaderHint = computed(
  () => selected.value !== null && kind.value === "shader" && !selected.value.loader,
);

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

const watched = ref<TaskId[]>([]);
watch(
  () => watched.value.map((id) => tasks.entries.find((e) => e.task_id === id)?.status ?? "running"),
  (statuses) => {
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

    <NAlert v-if="shaderHint" type="info" class="hint">
      <div class="hint-body">
        <span>{{ t("resources.global.shaderNoLoader", { name: selected?.name }) }}</span>
        <NButton size="small" @click="openLoaderDialog">
          {{ t("resources.global.installLoader") }}
        </NButton>
      </div>
    </NAlert>

    <NEmpty
      v-if="blocked"
      class="blocked"
      :description="t('resources.global.modNoLoader', { name: selected?.name })"
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

.hint {
  margin-bottom: 8px;
}

.hint-body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
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
