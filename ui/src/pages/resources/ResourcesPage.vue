<script setup lang="ts">
// 实例的资源管理（PLAN.md M2-06）：按种类（Mod、资源包、光影、数据包）分页签，
// 每类可查看已安装（启用/禁用/删除、检查更新与更新、从本地导入）或浏览 Modrinth 并安装。
// 数据包按存档管理。
import {
  ArrowDownload20Regular,
  ArrowImport20Regular,
  ArrowLeft20Regular,
  ArrowSync20Regular,
} from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NEmpty,
  NIcon,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSpin,
  NTab,
  NTabs,
} from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { instanceGet } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import {
  resourcesInstalled,
  resourcesWorlds,
  type InstalledList,
  type InstalledResource,
  type ResourceHit,
  type ResourceKind,
} from "../../api/resources";
import type { AppError } from "../../bindings/AppError";
import type { InstanceConfig } from "../../bindings/InstanceConfig";
import type { TaskId } from "../../bindings/TaskId";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import BrowsePanel from "./BrowsePanel.vue";
import InstallDialog from "./InstallDialog.vue";
import ChannelTag from "./ChannelTag.vue";
import InstalledPanel from "./InstalledPanel.vue";
import { useModImport } from "../instances/modImport";
import { kindsFor } from "./resources";
import { useResourceUpdates } from "./updates";

type View = "installed" | "browse";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const tasks = useTasksStore();

const instanceId = computed(() => String(route.params.id ?? ""));
const instance = ref<InstanceConfig | null>(null);
const loadError = ref<AppError | null>(null);

const kind = ref<ResourceKind>("mod");
const view = ref<View>("installed");
const worlds = ref<string[]>([]);
const world = ref<string | null>(null);

const installed = ref<InstalledList | null>(null);
const installedLoading = ref(false);
const installedError = ref<AppError | null>(null);
const installing = ref<ResourceHit | null>(null);

const kinds = computed(() => kindsFor(instance.value?.loader ?? null));
const needsWorld = computed(() => kind.value === "datapack");
const ready = computed(() => !needsWorld.value || world.value !== null);

watch(
  instanceId,
  async (id) => {
    instance.value = null;
    loadError.value = null;
    installed.value = null;
    if (!id) return;
    try {
      const loaded = await instanceGet({ id });
      if (instanceId.value !== id) return;
      instance.value = loaded;
      const requested = route.query.kind;
      const available = kindsFor(loaded.loader);
      kind.value = available.find((k) => k === requested) ?? available[0];
      worlds.value = await resourcesWorlds({ instance_id: id }).catch(() => []);
      world.value = worlds.value[0] ?? null;
      await loadInstalled();
    } catch (err) {
      loadError.value = toAppError(err);
    }
  },
  { immediate: true },
);

/** 只保留最后一次读取的结果。 */
let seq = 0;
async function loadInstalled() {
  const current = ++seq;
  installedError.value = null;
  if (!instance.value || !ready.value) {
    installed.value = null;
    return;
  }
  installedLoading.value = true;
  try {
    const list = await resourcesInstalled({
      instance_id: instanceId.value,
      kind: kind.value,
      world: world.value ?? undefined,
    });
    if (current === seq) installed.value = list;
  } catch (err) {
    if (current === seq) installedError.value = toAppError(err);
  } finally {
    if (current === seq) installedLoading.value = false;
  }
}

const updateTarget = computed(() =>
  instance.value && ready.value
    ? {
        instanceId: instanceId.value,
        instanceName: instance.value.name,
        kind: kind.value,
        world: world.value,
      }
    : null,
);
const {
  updates,
  checking,
  check: checkUpdates,
  update,
  reset: resetUpdates,
} = useResourceUpdates(updateTarget);
const { importing, importMods } = useModImport();

watch([kind, world], () => {
  installed.value = null;
  resetUpdates();
  void loadInstalled();
});

const installedIds = computed(
  () =>
    new Set(
      (installed.value?.resources ?? []).flatMap((r) => (r.source ? [r.source.project_id] : [])),
    ),
);

/** 本页发起的安装与更新任务；成功后重新读取已安装列表，更新结果需重新检查。 */
const started = ref<TaskId[]>([]);
watch(
  () => started.value.map((id) => tasks.entries.find((e) => e.task_id === id)?.status ?? "running"),
  (statuses) => {
    if (statuses.every((s) => s === "running")) return;
    if (statuses.some((s) => s === "succeeded")) {
      updates.value = null;
      void loadInstalled();
    }
    started.value = started.value.filter((_, i) => statuses[i] === "running");
  },
);

async function startUpdate(fileNames?: string[]) {
  const id = await update(fileNames);
  if (id) started.value.push(id);
}

async function importLocal() {
  if (await importMods(instanceId.value, kind.value, world.value ?? undefined)) {
    void loadInstalled();
  }
}

function updateFor(resource: InstalledResource) {
  return updates.value?.get(resource.file_name) ?? null;
}

const worldOptions = computed(() => worlds.value.map((w) => ({ label: w, value: w })));
const title = computed(() =>
  instance.value ? `${t("resources.title")} · ${instance.value.name}` : t("resources.title"),
);
</script>

<template>
  <div class="resources-page">
    <div class="head">
      <NButton
        quaternary
        circle
        :aria-label="t('resources.back')"
        @click="router.push({ name: 'instances' })"
      >
        <template #icon
          ><NIcon><ArrowLeft20Regular /></NIcon
        ></template>
      </NButton>
      <h1 class="page-title">{{ title }}</h1>
    </div>

    <NAlert v-if="loadError" type="error" :title="t('resources.loadFailed')">
      {{ formatError(loadError) }}
    </NAlert>

    <NSpin v-else :show="!instance" class="body">
      <template v-if="instance">
        <div class="bar">
          <NTabs v-model:value="kind" type="line" class="kinds">
            <NTab v-for="k in kinds" :key="k" :name="k">
              {{ t(`resources.kind.${k}`) }}
            </NTab>
          </NTabs>
          <NRadioGroup v-model:value="view" size="small">
            <NRadioButton value="installed">{{ t("resources.view.installed") }}</NRadioButton>
            <NRadioButton value="browse">{{ t("resources.view.browse") }}</NRadioButton>
          </NRadioGroup>
        </div>

        <div v-if="needsWorld" class="world">
          <span class="world-label">{{ t("resources.world.label") }}</span>
          <NSelect
            v-model:value="world"
            class="world-select"
            :options="worldOptions"
            :placeholder="t('resources.world.placeholder')"
          />
        </div>

        <NEmpty v-if="!ready" class="empty" :description="t('resources.world.empty')" />
        <InstalledPanel
          v-else-if="view === 'installed'"
          class="panel"
          :instance-id="instanceId"
          :kind="kind"
          :world="world"
          :list="installed"
          :loading="installedLoading"
          :error="installedError"
          @reload="loadInstalled"
        >
          <template #actions>
            <NButton :loading="importing === instanceId" @click="importLocal">
              <template #icon
                ><NIcon><ArrowImport20Regular /></NIcon
              ></template>
              {{ t("resources.import") }}
            </NButton>
            <NButton
              :loading="checking"
              :disabled="!installed?.resources.length"
              @click="checkUpdates"
            >
              <template #icon
                ><NIcon><ArrowSync20Regular /></NIcon
              ></template>
              {{ t("resources.updates.check") }}
            </NButton>
            <NButton v-if="updates?.size" type="primary" @click="startUpdate()">
              <template #icon
                ><NIcon><ArrowDownload20Regular /></NIcon
              ></template>
              {{ t("resources.updates.all", { count: updates.size }) }}
            </NButton>
          </template>
          <template v-if="updates && updates.size === 0" #notice>
            <div class="up-to-date">{{ t("resources.updates.none") }}</div>
          </template>
          <template #row="{ resource }">
            <div v-if="updateFor(resource)" class="update">
              <span class="update-version">
                {{
                  t("resources.updates.to", { version: updateFor(resource)?.latest.version_number })
                }}
              </span>
              <ChannelTag :channel="updateFor(resource)!.latest.channel" />
              <NButton
                size="small"
                type="primary"
                secondary
                @click="startUpdate([resource.file_name])"
              >
                {{ t("resources.updates.update") }}
              </NButton>
            </div>
          </template>
        </InstalledPanel>
        <BrowsePanel
          v-else
          class="panel"
          :instance-id="instanceId"
          :kind="kind"
          :installed-ids="installedIds"
          @install="(hit: ResourceHit) => (installing = hit)"
        />
      </template>
    </NSpin>

    <InstallDialog
      v-if="instance"
      :hit="installing"
      :instance-id="instanceId"
      :instance-name="instance.name"
      :kind="kind"
      :world="world"
      @close="installing = null"
      @started="(id: TaskId) => started.push(id)"
    />
  </div>
</template>

<style scoped>
.resources-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 1200px;
}

.head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.head .page-title {
  margin: 0;
}

.body {
  flex: 1;
  min-height: 0;
}

.body :deep(.n-spin-content) {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.bar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 8px 0 12px;
}

.kinds {
  flex: 1;
}

.world {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.world-label {
  font-size: 13px;
}

.world-select {
  width: 280px;
}

.panel {
  flex: 1;
  min-height: 0;
}

.empty {
  padding: 48px 0;
}

.up-to-date {
  font-size: 13px;
  color: var(--tmcl-text-secondary);
}

.update {
  display: flex;
  align-items: center;
  gap: 8px;
}

.update-version {
  font-size: 12px;
  color: var(--tmcl-accent);
}
</style>
