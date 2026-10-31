<script setup lang="ts">
// 直连（PLAN.md M2-12、M2-15）：EasyTier + Scaffolding 联机，房间码与 HMCL、PCL CE、陶瓦联机互通。
// 房主选一个已对局域网开放的世界开房，把房间码发给朋友；成员输入房间码加入，游戏里在“局域网游戏”中看到房间。
// 开房 / 加入是后台任务（进度见任务中心），之后每 3 秒读一次 `p2p_status`（只访问本机）。
import {
  ArrowClockwise20Regular,
  Copy20Regular,
  DoorArrowRight20Regular,
  Play20Regular,
  Stop20Regular,
} from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NIcon,
  NInput,
  NInputNumber,
  NProgress,
  NSelect,
  NTag,
  useDialog,
  useMessage,
} from "naive-ui";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import { p2pHost, p2pJoin, p2pLanWorlds, p2pStatus, p2pStop } from "../../api/p2p";
import type { AppError } from "../../bindings/AppError";
import type { LanWorld } from "../../bindings/LanWorld";
import type { P2pStatus } from "../../bindings/P2pStatus";
import type { TaskId } from "../../bindings/TaskId";
import { progressPercent } from "../../components/tasks/format";
import { formatError } from "../../i18n";
import { useAccountsStore } from "../../stores/accounts";
import { useTasksStore } from "../../stores/tasks";
import LatencyTag from "./LatencyTag.vue";
import {
  isValidPlayerName,
  isValidPort,
  loadPrefs,
  normalizeRoomCode,
  savePrefs,
  type MultiplayerRole,
} from "./multiplayer";

const props = defineProps<{ role: MultiplayerRole }>();
const emit = defineEmits<{ useSakura: [] }>();

const STATUS_INTERVAL_MS = 3_000;
const TASK_KINDS = ["p2p_host", "p2p_join"];
/** 这些失败说明直连走不通，提示改用樱花穿透。 */
const SAKURA_HINT_CODES = ["P2P_DIRECT_FAILED", "P2P_CENTER_NOT_FOUND", "P2P_TUNNEL_EXITED"];

const { t } = useI18n();
const message = useMessage();
const dialog = useDialog();
const tasks = useTasksStore();
const accounts = useAccountsStore();

const status = ref<P2pStatus | null>(null);
const playerName = ref("");
const port = ref<number | null>(loadPrefs().port);
const worlds = ref<LanWorld[]>([]);
const scanning = ref(false);
const scanned = ref(false);
const code = ref("");
const taskId = ref<TaskId | null>(null);
const startError = ref<AppError | null>(null);
const stopping = ref(false);

let timer: ReturnType<typeof setInterval> | undefined;

async function refresh() {
  try {
    status.value = await p2pStatus();
  } catch {
    // 浏览器中调试时没有后端。
  }
}

onMounted(async () => {
  await Promise.all([refresh(), tasks.init(), accounts.ensureLoaded().catch(() => {})]);
  playerName.value ||= accounts.current?.name ?? "";
  timer = setInterval(refresh, STATUS_INTERVAL_MS);
  if (props.role === "host") void scan();
});

onUnmounted(() => clearInterval(timer));

watch(
  () => props.role,
  (role) => {
    startError.value = null;
    if (role === "host" && !scanned.value) void scan();
  },
);

async function scan() {
  scanning.value = true;
  try {
    worlds.value = (await p2pLanWorlds()).worlds;
    if (worlds.value.length && !worlds.value.some((w) => w.port === port.value))
      port.value = worlds.value[0].port;
  } catch {
    worlds.value = [];
  } finally {
    scanning.value = false;
    scanned.value = true;
  }
}

const room = computed(() => status.value?.room ?? null);
const task = computed(() =>
  taskId.value !== null
    ? tasks.entries.find((e) => e.task_id === taskId.value)
    : tasks.running.find((e) => TASK_KINDS.includes(e.kind)),
);
const starting = computed(
  () => task.value?.status === "running" || (status.value?.starting ?? false),
);
const percent = computed(() => progressPercent(task.value?.progress ?? null));
const stageText = computed(() => {
  const stage = task.value?.progress?.stage;
  return stage ? t(`task.stage.${stage}`) : t("multiplayer.direct.preparing");
});

watch(
  () => task.value?.status,
  (state) => {
    if (!state || state === "running") return;
    if (state === "failed" && task.value?.error) startError.value = task.value.error;
    void refresh();
  },
);

const worldOptions = computed(() =>
  worlds.value.map((w) => ({ value: w.port, label: `${w.motd} (${w.port})` })),
);
const roomCode = computed(() => normalizeRoomCode(code.value));
const codeInvalid = computed(() => code.value.trim() !== "" && roomCode.value === null);
const nameValid = computed(() => isValidPlayerName(playerName.value));
const canHost = computed(() => nameValid.value && isValidPort(port.value) && !starting.value);
const canJoin = computed(() => nameValid.value && roomCode.value !== null && !starting.value);
const suggestSakura = computed(
  () => startError.value !== null && SAKURA_HINT_CODES.includes(startError.value.code),
);

async function run(kind: string, start: () => ReturnType<typeof p2pHost>) {
  startError.value = null;
  try {
    taskId.value = await tasks.start({ kind, label: t(`task.kind.${kind}`), start });
  } catch (err) {
    startError.value = toAppError(err);
  }
}

function host() {
  if (!canHost.value || port.value === null) return;
  const args = { mc_port: port.value, player_name: playerName.value.trim() };
  savePrefs({ port: args.mc_port });
  void run("p2p_host", () => p2pHost(args));
}

function join() {
  if (!canJoin.value || roomCode.value === null) return;
  const args = { room_code: roomCode.value, player_name: playerName.value.trim() };
  void run("p2p_join", () => p2pJoin(args));
}

async function stop() {
  stopping.value = true;
  try {
    await p2pStop();
    taskId.value = null;
    await refresh();
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    stopping.value = false;
  }
}

function confirmStop() {
  const isHost = room.value?.role === "host";
  dialog.warning({
    title: isHost ? t("multiplayer.direct.closeTitle") : t("multiplayer.direct.leaveTitle"),
    content: isHost ? t("multiplayer.direct.closeContent") : t("multiplayer.direct.leaveContent"),
    positiveText: isHost ? t("multiplayer.direct.close") : t("multiplayer.direct.leave"),
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
  <div v-if="room" class="card">
    <div class="head">
      <span class="title">
        {{
          room.role === "host" ? t("multiplayer.direct.hosting") : t("multiplayer.direct.joined")
        }}
      </span>
      <NTag v-if="status?.running" type="success" size="small" :bordered="false">
        {{ t("multiplayer.status.running") }}
      </NTag>
      <NTag v-else type="error" size="small" :bordered="false">
        {{ t("multiplayer.status.exited") }}
      </NTag>
      <template v-if="room.role === 'guest' && status?.running">
        <LatencyTag :latency="room.latency_ms ?? undefined" />
        <NTag v-if="room.direct !== undefined" size="small" :bordered="false">
          {{ room.direct ? t("multiplayer.direct.p2p") : t("multiplayer.direct.relayed") }}
        </NTag>
      </template>
    </div>

    <template v-if="room.role === 'host'">
      <p class="muted">{{ t("multiplayer.direct.shareCode") }}</p>
      <div class="big">
        <code>{{ room.code }}</code>
        <NButton secondary @click="copy(room.code)">
          <template #icon
            ><NIcon><Copy20Regular /></NIcon
          ></template>
          {{ t("multiplayer.copy") }}
        </NButton>
      </div>
    </template>
    <template v-else>
      <p class="muted">{{ t("multiplayer.direct.joinHint") }}</p>
      <div v-if="room.local_address" class="big">
        <code>{{ room.local_address }}</code>
        <NButton secondary @click="copy(room.local_address)">
          <template #icon
            ><NIcon><Copy20Regular /></NIcon
          ></template>
          {{ t("multiplayer.copy") }}
        </NButton>
      </div>
    </template>

    <p class="label">{{ t("multiplayer.direct.players", { count: room.players.length }) }}</p>
    <ul class="players">
      <li v-for="player in room.players" :key="player.name">
        <span class="name">{{ player.name }}</span>
        <NTag v-if="player.kind === 'host'" size="small" type="info" :bordered="false">
          {{ t("multiplayer.direct.hostTag") }}
        </NTag>
        <NTag v-if="player.is_self" size="small" :bordered="false">
          {{ t("multiplayer.direct.selfTag") }}
        </NTag>
        <span class="muted small">{{ player.vendor }}</span>
      </li>
    </ul>
    <p class="muted small">
      {{ t("multiplayer.direct.details", { port: room.mc_port, nat: room.nat_type }) }}
    </p>

    <NAlert v-if="!status?.running" type="error" class="gap">
      {{
        room.role === "host"
          ? t("multiplayer.direct.hostExited")
          : t("multiplayer.direct.guestExited")
      }}
    </NAlert>
    <NAlert v-else-if="room.role === 'guest' && room.direct === false" type="info" class="gap">
      {{ t("multiplayer.direct.relayedHint") }}
    </NAlert>
    <div class="actions">
      <NButton type="error" secondary :loading="stopping" @click="confirmStop">
        <template #icon
          ><NIcon
            ><component
              :is="room.role === 'host' ? Stop20Regular : DoorArrowRight20Regular" /></NIcon
        ></template>
        {{ room.role === "host" ? t("multiplayer.direct.close") : t("multiplayer.direct.leave") }}
      </NButton>
    </div>
  </div>

  <div v-else class="card">
    <ol class="steps">
      <template v-if="role === 'host'">
        <li>{{ t("multiplayer.direct.hostStep1") }}</li>
        <li>{{ t("multiplayer.direct.hostStep2") }}</li>
        <li>{{ t("multiplayer.direct.hostStep3") }}</li>
      </template>
      <template v-else>
        <li>{{ t("multiplayer.direct.joinStep1") }}</li>
        <li>{{ t("multiplayer.direct.joinStep2") }}</li>
        <li>{{ t("multiplayer.direct.joinStep3") }}</li>
      </template>
    </ol>
    <div class="form">
      <label class="field">
        <span class="label">{{ t("multiplayer.direct.playerName") }}</span>
        <NInput
          v-model:value="playerName"
          class="name-input"
          :maxlength="64"
          :status="playerName && !nameValid ? 'error' : undefined"
          :placeholder="t('multiplayer.direct.playerNamePlaceholder')"
        />
      </label>
      <template v-if="role === 'host'">
        <label class="field">
          <span class="label">{{ t("multiplayer.direct.world") }}</span>
          <div class="row">
            <NSelect
              v-if="worlds.length"
              v-model:value="port"
              class="world"
              :options="worldOptions"
            />
            <NInputNumber
              v-else
              v-model:value="port"
              class="port"
              :min="1"
              :max="65535"
              :show-button="false"
              :placeholder="t('multiplayer.direct.portPlaceholder')"
            />
            <NButton quaternary :loading="scanning" @click="scan">
              <template #icon
                ><NIcon><ArrowClockwise20Regular /></NIcon
              ></template>
              {{ t("multiplayer.direct.scan") }}
            </NButton>
          </div>
        </label>
      </template>
      <label v-else class="field">
        <span class="label">{{ t("multiplayer.direct.roomCode") }}</span>
        <NInput
          v-model:value="code"
          class="code-input"
          clearable
          :status="codeInvalid ? 'error' : undefined"
          placeholder="U/XXXX-XXXX-XXXX-XXXX"
          @keyup.enter="join"
        />
      </label>
    </div>
    <p v-if="role === 'host' && scanned && !worlds.length" class="muted small">
      {{ t("multiplayer.direct.noWorlds") }}
    </p>
    <p v-if="role === 'member' && codeInvalid" class="muted small">
      {{ t("errors.P2P_ROOM_CODE_INVALID") }}
    </p>

    <NAlert v-if="startError" type="error" class="gap" :title="t('multiplayer.direct.failed')">
      {{ formatError(startError) }}
      <div v-if="suggestSakura" class="alert-actions">
        <NButton size="small" @click="emit('useSakura')">
          {{ t("multiplayer.direct.useSakura") }}
        </NButton>
      </div>
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
      <NButton
        v-if="role === 'host'"
        type="primary"
        :disabled="!canHost"
        :loading="starting"
        @click="host"
      >
        <template #icon
          ><NIcon><Play20Regular /></NIcon
        ></template>
        {{ t("multiplayer.direct.host") }}
      </NButton>
      <NButton v-else type="primary" :disabled="!canJoin" :loading="starting" @click="join">
        <template #icon
          ><NIcon><DoorArrowRight20Regular /></NIcon
        ></template>
        {{ t("multiplayer.direct.join") }}
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

.head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.title,
.name {
  font-weight: 600;
}

.muted {
  margin: 4px 0;
  color: var(--tmcl-text-secondary);
}

.small {
  font-size: 12px;
}

.label {
  margin: 12px 0 4px;
  font-size: 12px;
}

.field .label {
  margin: 0;
}

.big {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin: 8px 0;
}

.big code {
  font-size: 20px;
  font-weight: 600;
  user-select: all;
}

.players {
  margin: 0;
  padding: 0;
  list-style: none;
}

.players li {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
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

.row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.name-input {
  width: 200px;
}

.world {
  width: 280px;
}

.port {
  width: 140px;
}

.code-input {
  width: 280px;
}

.gap {
  margin-top: 12px;
}

.alert-actions {
  margin-top: 8px;
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
