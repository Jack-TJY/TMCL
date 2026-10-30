<script setup lang="ts">
// 游戏日志（PLAN.md M1-13），显示在独立的日志窗口中：实时滚动（停在底部时跟随新日志）、
// 按关键字筛选、复制与导出，运行中可强制结束游戏。日志由后端脱敏后推送（M1-07），这里不再处理令牌。
import { Copy20Regular, Dismiss20Regular, Save20Regular, Search20Regular } from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NIcon,
  NInput,
  NTag,
  NVirtualList,
  useMessage,
  useThemeVars,
  type VirtualListInst,
} from "naive-ui";
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { launchKill } from "../../api/launch";
import { formatError } from "../../i18n";
import { filterLines, logFileName, logText } from "./log";
import { isActive, type GameSession, type SessionStatus } from "./sessions";

/** 每行高度（像素），与样式中的 line-height 一致。 */
const LINE_HEIGHT = 18;
const STATUS_TYPE: Record<SessionStatus, "default" | "info" | "success" | "error"> = {
  starting: "info",
  running: "success",
  exited: "default",
  crashed: "error",
  failed: "error",
};

const props = defineProps<{ session: GameSession | null }>();

const { t } = useI18n();
const message = useMessage();
const theme = useThemeVars();

const query = ref("");
const follow = ref(true);
const list = ref<VirtualListInst | null>(null);

const session = computed(() => props.session ?? undefined);
const lines = computed(() => filterLines(session.value?.lines ?? [], query.value));

watch(
  () => [lines.value.length, session.value?.lines.at(-1)?.seq],
  async () => {
    if (!follow.value) return;
    await nextTick();
    list.value?.scrollTo({ position: "bottom" });
  },
);

function onScroll(event: Event) {
  const el = event.target as HTMLElement;
  follow.value = el.scrollHeight - el.scrollTop - el.clientHeight < LINE_HEIGHT * 2;
}

async function copy() {
  try {
    await navigator.clipboard.writeText(logText(lines.value));
    message.success(t("gameLog.copied"));
  } catch {
    message.error(t("gameLog.copyFailed"));
  }
}

/** 以浏览器下载的方式导出当前显示的日志。 */
function save() {
  const current = session.value;
  if (!current) return;
  const url = URL.createObjectURL(new Blob([logText(lines.value)], { type: "text/plain" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = logFileName(current.name, new Date());
  link.click();
  URL.revokeObjectURL(url);
}

async function kill() {
  if (!session.value) return;
  await launchKill({ instance_id: session.value.instanceId }).catch(() =>
    message.error(t("gameLog.killFailed")),
  );
}
</script>

<template>
  <div class="game-log">
    <NAlert v-if="session?.error" type="error" class="launch-error" :title="t('gameLog.failed')">
      {{ formatError(session.error) }}
    </NAlert>

    <div class="toolbar">
      <NTag v-if="session" size="small" :bordered="false" :type="STATUS_TYPE[session.status]">
        {{
          t(`gameLog.status.${session.status}`, {
            code: session.exitCode ?? "-",
          })
        }}
      </NTag>
      <NInput v-model:value="query" size="small" clearable :placeholder="t('gameLog.search')">
        <template #prefix
          ><NIcon><Search20Regular /></NIcon
        ></template>
      </NInput>
      <span class="count">{{
        t("gameLog.count", { shown: lines.length, total: session?.lines.length ?? 0 })
      }}</span>
      <NButton size="small" quaternary :disabled="lines.length === 0" @click="copy">
        <template #icon
          ><NIcon><Copy20Regular /></NIcon
        ></template>
        {{ t("gameLog.copy") }}
      </NButton>
      <NButton size="small" quaternary :disabled="lines.length === 0" @click="save">
        <template #icon
          ><NIcon><Save20Regular /></NIcon
        ></template>
        {{ t("gameLog.export") }}
      </NButton>
      <NButton v-if="isActive(session)" size="small" type="error" secondary @click="kill">
        <template #icon
          ><NIcon><Dismiss20Regular /></NIcon
        ></template>
        {{ t("gameLog.kill") }}
      </NButton>
    </div>

    <div class="log" :style="{ '--log-error': theme.errorColor }">
      <p v-if="lines.length === 0" class="log-empty">
        {{ query ? t("gameLog.noMatch") : t("gameLog.empty") }}
      </p>
      <NVirtualList
        v-else
        ref="list"
        :items="lines"
        :item-size="LINE_HEIGHT"
        key-field="seq"
        class="log-list"
        @scroll="onScroll"
      >
        <template #default="{ item }">
          <div class="log-line" :class="item.stream">{{ item.text }}</div>
        </template>
      </NVirtualList>
    </div>
  </div>
</template>

<style scoped>
.game-log {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.launch-error {
  flex-shrink: 0;
  margin-bottom: 12px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  margin-bottom: 8px;
}

.toolbar > .n-input {
  max-width: 280px;
}

.count {
  flex: 1;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.log {
  flex: 1;
  min-height: 0;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-layer);
}

.log-list {
  height: 100%;
}

.log-empty {
  margin: 0;
  padding: 16px;
  font-size: 13px;
  color: var(--tmcl-text-secondary);
}

.log-line {
  padding: 0 8px;
  font-family: "Cascadia Mono", Consolas, "SF Mono", Menlo, monospace;
  font-size: 12px;
  line-height: 18px;
  white-space: pre;
}

.log-line.stderr {
  color: var(--log-error);
}
</style>
