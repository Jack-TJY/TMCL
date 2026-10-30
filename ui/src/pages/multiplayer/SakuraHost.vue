<script setup lang="ts">
// 樱花穿透开房（PLAN.md M2-15）：选节点、填游戏端口后开始联机（后台任务，进度见任务中心），
// 拿到联机地址后显示给房主分享。联机期间每 3 秒读一次状态（只读本地），每 15 秒测一次到节点的延迟。
import { Copy20Regular, Play20Regular, Stop20Regular } from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NIcon,
  NInputNumber,
  NProgress,
  NSelect,
  NSwitch,
  NTag,
  useDialog,
  useMessage,
} from "naive-ui";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import {
  SAKURA_START_KIND,
  sakuraNodes,
  sakuraPing,
  sakuraStart,
  sakuraStatus,
  sakuraStop,
  type SakuraNodeInfo,
  type SakuraStatus,
  type SakuraUser,
} from "../../api/sakura";
import type { AppError } from "../../bindings/AppError";
import type { TaskId } from "../../bindings/TaskId";
import { progressPercent } from "../../components/tasks/format";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import SakuraAccount from "./SakuraAccount.vue";
import LatencyTag from "./LatencyTag.vue";
import { isValidPort, loadPrefs, nodeUsable, savePrefs, sortNodes } from "./multiplayer";

const STATUS_INTERVAL_MS = 3_000;
const PING_INTERVAL_MS = 15_000;

const { t } = useI18n();
const message = useMessage();
const dialog = useDialog();
const tasks = useTasksStore();

const status = ref<SakuraStatus | null>(null);
const user = ref<SakuraUser | null>(null);
const nodes = ref<SakuraNodeInfo[]>([]);
const nodesError = ref<AppError | null>(null);
const nodesLoading = ref(false);

const prefs = loadPrefs();
const node = ref<number | null>(prefs.node);
const port = ref<number | null>(prefs.port);
const isPublic = ref(false);

const taskId = ref<TaskId | null>(null);
const startError = ref<AppError | null>(null);
const stopping = ref(false);
const latency = ref<number | null | undefined>(undefined);

let statusTimer: ReturnType<typeof setInterval> | undefined;
let pingTimer: ReturnType<typeof setInterval> | undefined;

async function refreshStatus() {
  try {
    status.value = await sakuraStatus();
  } catch {
    // 浏览器中调试时没有后端。
  }
}

async function loadNodes() {
  nodesLoading.value = true;
  nodesError.value = null;
  try {
    nodes.value = await sakuraNodes();
  } catch (err) {
    nodesError.value = toAppError(err);
  } finally {
    nodesLoading.value = false;
  }
}

onMounted(async () => {
  await Promise.all([refreshStatus(), tasks.init()]);
  statusTimer = setInterval(refreshStatus, STATUS_INTERVAL_MS);
});

onUnmounted(() => {
  clearInterval(statusTimer);
  clearInterval(pingTimer);
});

function onUser(value: SakuraUser | null) {
  user.value = value;
  if (value && nodes.value.length === 0) void loadNodes();
}

const room = computed(() => status.value?.room ?? null);
/** 由本页发起、或页面打开前已在运行的开始联机任务。 */
const task = computed(() =>
  taskId.value !== null
    ? tasks.entries.find((e) => e.task_id === taskId.value)
    : tasks.running.find((e) => e.kind === SAKURA_START_KIND),
);
const starting = computed(
  () => task.value?.status === "running" || (status.value?.starting ?? false),
);
const percent = computed(() => progressPercent(task.value?.progress ?? null));
const stageText = computed(() => {
  const stage = task.value?.progress?.stage;
  return stage ? t(`task.stage.${stage}`) : t("multiplayer.sakura.host.preparing");
});

// 任务结束：成功则立刻取地址，失败则显示原因。
watch(
  () => task.value?.status,
  (state) => {
    if (!state || state === "running") return;
    if (state === "failed" && task.value?.error) startError.value = task.value.error;
    void refreshStatus();
  },
);

// 联机中定时测延迟。
watch(
  () => room.value?.address ?? null,
  (address) => {
    clearInterval(pingTimer);
    latency.value = undefined;
    if (!address) return;
    const ping = async () => {
      latency.value = await sakuraPing(address).catch(() => null);
    };
    void ping();
    pingTimer = setInterval(ping, PING_INTERVAL_MS);
  },
);

const nodeOptions = computed(() =>
  sortNodes(nodes.value, isPublic.value).map((n) => ({
    value: n.id,
    label: n.vip > 0 ? `#${n.id} ${n.name} · VIP` : `#${n.id} ${n.name}`,
    disabled: !nodeUsable(n, isPublic.value),
  })),
);
const selectedNode = computed(() => nodes.value.find((n) => n.id === node.value) ?? null);
const nodeHint = computed(() => {
  const n = selectedNode.value;
  if (!n) return null;
  if (!n.available) return t("multiplayer.sakura.host.nodeOffline");
  if (isPublic.value && !n.public_game) return t("multiplayer.sakura.host.nodeNotPublic");
  return n.description || null;
});
const canStart = computed(
  () =>
    user.value !== null &&
    user.value.verified &&
    selectedNode.value !== null &&
    nodeUsable(selectedNode.value, isPublic.value) &&
    isValidPort(port.value) &&
    !starting.value,
);
const roomNodeName = computed(() => {
  const id = room.value?.node;
  const found = nodes.value.find((n) => n.id === id);
  return found ? `#${found.id} ${found.name}` : `#${id}`;
});

async function start() {
  if (!canStart.value || node.value === null || port.value === null) return;
  const args = { local_port: port.value, node: node.value, public: isPublic.value };
  savePrefs({ node: args.node, port: args.local_port });
  startError.value = null;
  try {
    taskId.value = await tasks.start({
      kind: SAKURA_START_KIND,
      label: t("multiplayer.sakura.host.taskLabel"),
      start: () => sakuraStart(args),
    });
  } catch (err) {
    startError.value = toAppError(err);
  }
}

async function stop() {
  stopping.value = true;
  try {
    await sakuraStop();
    taskId.value = null;
    await refreshStatus();
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    stopping.value = false;
  }
}

function confirmStop() {
  dialog.warning({
    title: t("multiplayer.sakura.host.stopTitle"),
    content: t("multiplayer.sakura.host.stopContent"),
    positiveText: t("multiplayer.sakura.host.stop"),
    negativeText: t("multiplayer.cancel"),
    onPositiveClick: () => void stop(),
  });
}

async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    message.success(t("multiplayer.copied"));
  } catch {
    message.error(t("multiplayer.copyFailed"));
  }
}
</script>

<template>
  <SakuraAccount
    v-if="status"
    :has-key="status.has_key"
    @user="onUser"
    @key-changed="refreshStatus"
  />

  <div v-if="room" class="card room">
    <div class="room-head">
      <span class="title">{{ t("multiplayer.sakura.host.online") }}</span>
      <NTag v-if="status?.running" type="success" size="small" :bordered="false">
        {{ t("multiplayer.status.running") }}
      </NTag>
      <NTag v-else type="error" size="small" :bordered="false">
        {{ t("multiplayer.status.exited") }}
      </NTag>
      <LatencyTag v-if="status?.running" :latency="latency" />
    </div>
    <p class="muted">{{ t("multiplayer.sakura.host.share") }}</p>
    <div class="address">
      <code>{{ room.address }}</code>
      <NButton secondary @click="copy(room.address)">
        <template #icon
          ><NIcon><Copy20Regular /></NIcon
        ></template>
        {{ t("multiplayer.copy") }}
      </NButton>
    </div>
    <div v-if="room.ip_address" class="address secondary">
      <span class="muted">{{ t("multiplayer.sakura.host.ipAddress") }}</span>
      <code>{{ room.ip_address }}</code>
      <NButton size="small" quaternary @click="copy(room.ip_address)">
        <template #icon
          ><NIcon><Copy20Regular /></NIcon
        ></template>
      </NButton>
    </div>
    <p class="muted">
      {{ t("multiplayer.sakura.host.details", { node: roomNodeName, port: room.local_port }) }}
    </p>
    <NAlert v-if="!status?.running" type="error" class="gap">
      {{ t("multiplayer.sakura.host.exitedHint") }}
    </NAlert>
    <NAlert v-else-if="latency === null" type="warning" class="gap">
      {{ t("multiplayer.sakura.host.unreachableHint") }}
    </NAlert>
    <div class="actions">
      <NButton type="error" secondary :loading="stopping" @click="confirmStop">
        <template #icon
          ><NIcon><Stop20Regular /></NIcon
        ></template>
        {{ t("multiplayer.sakura.host.stop") }}
      </NButton>
    </div>
  </div>

  <div v-else-if="user" class="card">
    <ol class="steps">
      <li>{{ t("multiplayer.sakura.host.step1") }}</li>
      <li>{{ t("multiplayer.sakura.host.step2") }}</li>
      <li>{{ t("multiplayer.sakura.host.step3") }}</li>
    </ol>
    <div class="form">
      <label class="field">
        <span class="label">{{ t("multiplayer.sakura.host.port") }}</span>
        <NInputNumber
          v-model:value="port"
          class="port"
          :min="1"
          :max="65535"
          :show-button="false"
          :placeholder="t('multiplayer.sakura.host.portPlaceholder')"
        />
      </label>
      <label class="field">
        <span class="label">{{ t("multiplayer.sakura.host.node") }}</span>
        <NSelect
          v-model:value="node"
          class="node"
          filterable
          :loading="nodesLoading"
          :options="nodeOptions"
          :placeholder="t('multiplayer.sakura.host.nodePlaceholder')"
        />
      </label>
      <label class="field">
        <span class="label">{{ t("multiplayer.sakura.host.public") }}</span>
        <NSwitch v-model:value="isPublic" />
      </label>
    </div>
    <p class="muted small">
      {{
        isPublic ? t("multiplayer.sakura.host.publicOn") : t("multiplayer.sakura.host.publicOff")
      }}
    </p>
    <p v-if="nodeHint" class="muted small">{{ nodeHint }}</p>
    <NAlert v-if="nodesError" type="error" class="gap">
      {{ formatError(nodesError) }}
      <NButton size="small" text type="primary" @click="loadNodes">{{
        t("multiplayer.retry")
      }}</NButton>
    </NAlert>
    <NAlert v-if="startError" type="error" class="gap" :title="t('multiplayer.sakura.host.failed')">
      {{ formatError(startError) }}
    </NAlert>
    <div v-if="starting" class="progress">
      <span class="muted">{{ stageText }}</span>
      <NProgress
        type="line"
        :percentage="percent ?? 0"
        :processing="percent === null"
        :show-indicator="false"
      />
    </div>
    <div class="actions">
      <NButton type="primary" :disabled="!canStart" :loading="starting" @click="start">
        <template #icon
          ><NIcon><Play20Regular /></NIcon
        ></template>
        {{ t("multiplayer.sakura.host.start") }}
      </NButton>
    </div>
  </div>
</template>

<style scoped>
.card {
  margin-top: 12px;
  padding: 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
}

.title {
  font-weight: 600;
}

.muted {
  margin: 4px 0;
  color: var(--tmcl-text-secondary);
}

.small {
  font-size: 12px;
}

.room-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.address {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin: 8px 0;
}

.address code {
  font-size: 20px;
  font-weight: 600;
  user-select: all;
}

.address.secondary code {
  font-size: 14px;
  font-weight: 400;
}

.steps {
  margin: 0 0 12px;
  padding-left: 20px;
  line-height: 22px;
}

.form {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 16px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.label {
  font-size: 12px;
}

.port {
  width: 140px;
}

.node {
  width: 320px;
}

.gap {
  margin-top: 12px;
}

.progress {
  margin-top: 12px;
}

.actions {
  display: flex;
  gap: 8px;
  margin-top: 16px;
}
</style>
