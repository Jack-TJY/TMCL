<script setup lang="ts">
// 安装一个 Modrinth 项目：选择版本（默认最新正式版），展示安装计划（前置、替换的旧文件、
// 冲突与缺少的前置），确认后以 resources_install 任务安装。
import { NAlert, NButton, NModal, NSelect, NSpin, NTag, useMessage } from "naive-ui";
import { computed, h, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import {
  resourcesInstall,
  resourcesInstallPlan,
  resourcesVersions,
  RESOURCES_INSTALL_KIND,
  type Conflict,
  type InstallPlan,
  type ResourceHit,
  type ResourceKind,
  type ResourceVersion,
  type UnavailableDependency,
} from "../../api/resources";
import type { AppError } from "../../bindings/AppError";
import type { TaskId } from "../../bindings/TaskId";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import ChannelTag from "./ChannelTag.vue";
import { defaultVersion, formatSize, planBlocked, planSize } from "./resources";

const props = defineProps<{
  hit: ResourceHit | null;
  instanceId: string;
  instanceName: string;
  kind: ResourceKind;
  world: string | null;
}>();
const emit = defineEmits<{ close: []; started: [taskId: TaskId] }>();

const { t } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const versions = ref<ResourceVersion[]>([]);
const versionId = ref<string | null>(null);
const loadingVersions = ref(false);
const versionError = ref<AppError | null>(null);
const plan = ref<InstallPlan | null>(null);
const planning = ref(false);
const planError = ref<AppError | null>(null);
const busy = ref(false);

function args(id: string) {
  return {
    instance_id: props.instanceId,
    kind: props.kind,
    version_id: id,
    world: props.world ?? undefined,
  };
}

watch(
  () => props.hit,
  async (hit) => {
    versions.value = [];
    versionId.value = null;
    versionError.value = null;
    plan.value = null;
    if (!hit) return;
    loadingVersions.value = true;
    try {
      const list = await resourcesVersions({
        instance_id: props.instanceId,
        kind: props.kind,
        project_id: hit.project_id,
      });
      if (props.hit !== hit) return;
      versions.value = list;
      versionId.value = defaultVersion(list)?.id ?? null;
    } catch (err) {
      if (props.hit === hit) versionError.value = toAppError(err);
    } finally {
      if (props.hit === hit) loadingVersions.value = false;
    }
  },
  { immediate: true },
);

watch(versionId, async (id) => {
  plan.value = null;
  planError.value = null;
  if (!id) return;
  planning.value = true;
  try {
    const result = await resourcesInstallPlan(args(id));
    if (versionId.value === id) plan.value = result;
  } catch (err) {
    if (versionId.value === id) planError.value = toAppError(err);
  } finally {
    if (versionId.value === id) planning.value = false;
  }
});

const versionOptions = computed(() =>
  versions.value.map((v) => ({ label: v.version_number, value: v.id, version: v })),
);

function renderVersionLabel(option: { label: string; version: ResourceVersion }) {
  return [option.label, " ", h(ChannelTag, { channel: option.version.channel })];
}

const blocked = computed(() => !plan.value || planBlocked(plan.value));

function conflictText(c: Conflict): string {
  return t(
    c.with_installed ? "resources.install.conflictInstalled" : "resources.install.conflict",
    {
      title: c.title,
      with: c.with_title,
    },
  );
}

/** `required_by` 是项目 ID，尽量换成计划里的标题。 */
function titleOf(projectId: string): string {
  if (projectId === props.hit?.project_id) return props.hit.title;
  return plan.value?.files.find((f) => f.project_id === projectId)?.title ?? projectId;
}

function unavailableText(u: UnavailableDependency): string {
  return t("resources.install.unavailableItem", {
    title: u.title ?? u.project_id ?? t("resources.install.unknownProject"),
    required: titleOf(u.required_by),
  });
}

async function confirm() {
  const hit = props.hit;
  const id = versionId.value;
  if (!hit || !id || blocked.value || busy.value) return;
  busy.value = true;
  try {
    const taskId = await tasks.start({
      kind: RESOURCES_INSTALL_KIND,
      label: `${hit.title} → ${props.instanceName}`,
      start: () => resourcesInstall(args(id)),
      instanceId: props.instanceId,
    });
    message.success(t("resources.install.started", { name: hit.title }));
    emit("started", taskId);
    emit("close");
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <NModal
    :show="hit !== null"
    preset="card"
    style="width: 600px"
    :title="t('resources.install.title', { name: hit?.title ?? '' })"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <NSpin :show="loadingVersions">
      <NAlert v-if="versionError" type="error" :title="t('resources.install.loadFailed')">
        {{ formatError(versionError) }}
      </NAlert>
      <NAlert
        v-else-if="!loadingVersions && versions.length === 0"
        type="warning"
        :title="t('resources.install.noVersions')"
      />
      <template v-else>
        <div class="field">
          <span class="field-label">{{ t("resources.install.version") }}</span>
          <NSelect
            v-model:value="versionId"
            class="version"
            :options="versionOptions"
            :render-label="renderVersionLabel"
            :disabled="busy"
            filterable
          />
        </div>
        <div v-if="planning" class="hint">{{ t("resources.install.planning") }}</div>
        <NAlert v-if="planError" type="error" :title="t('resources.install.planFailed')">
          {{ formatError(planError) }}
        </NAlert>
        <template v-if="plan">
          <NAlert
            v-if="plan.conflicts.length"
            type="error"
            class="block"
            :title="t('resources.install.conflicts')"
          >
            <div v-for="c in plan.conflicts" :key="`${c.project_id}-${c.with_project_id}`">
              {{ conflictText(c) }}
            </div>
          </NAlert>
          <NAlert
            v-if="plan.unavailable.length"
            type="error"
            class="block"
            :title="t('resources.install.unavailable')"
          >
            <div v-for="(u, i) in plan.unavailable" :key="i">{{ unavailableText(u) }}</div>
          </NAlert>
          <div class="hint">
            {{
              t("resources.install.files", {
                count: plan.files.length,
                size: formatSize(planSize(plan)),
              })
            }}
          </div>
          <ul class="files">
            <li v-for="file in plan.files" :key="file.version_id">
              <div class="file-main">
                <span class="file-title">{{ file.title }}</span>
                <span class="file-version">{{ file.version_number }}</span>
                <NTag v-if="file.required_by" size="small" round :bordered="false">
                  {{ t("resources.install.dependency") }}
                </NTag>
                <span class="file-size">{{ formatSize(file.size) }}</span>
              </div>
              <div v-if="file.required_by || file.replaces" class="file-sub">
                <span v-if="file.required_by">
                  {{ t("resources.install.requiredBy", { name: titleOf(file.required_by) }) }}
                </span>
                <span v-if="file.replaces">
                  {{ t("resources.install.replaces", { file: file.replaces }) }}
                </span>
              </div>
            </li>
          </ul>
        </template>
      </template>
    </NSpin>
    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{
          t("resources.install.cancel")
        }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="blocked" @click="confirm">
          {{ t("resources.install.confirm") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.field {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.field-label {
  flex-shrink: 0;
  font-size: 13px;
}

.version {
  flex: 1;
}

.hint {
  margin: 8px 0;
  font-size: 13px;
  color: var(--tmcl-text-secondary);
}

.block {
  margin-bottom: 8px;
}

.files {
  max-height: 260px;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}

.files li {
  padding: 6px 0;
  border-bottom: 1px solid var(--tmcl-stroke);
}

.file-main {
  display: flex;
  align-items: center;
  gap: 8px;
}

.file-title {
  font-weight: 600;
}

.file-version {
  color: var(--tmcl-text-secondary);
}

.file-size {
  margin-left: auto;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.file-sub {
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
