<script setup lang="ts">
// 任务中心的一项：名称、阶段、进度条、速度（停滞时提示网络可能中断），以及取消 / 重试 / 复制错误详情。
import {
  ArrowClockwise20Regular,
  Copy20Regular,
  Dismiss20Regular,
  DismissCircle20Regular,
} from "@vicons/fluent";
import { NButton, NIcon, NProgress, NTooltip, useMessage, useThemeVars } from "naive-ui";
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import { formatError } from "../../i18n";
import { isStalled, useTasksStore, type TaskEntry } from "../../stores/tasks";
import { errorDetails, formatBytes, progressPercent } from "./format";

const props = defineProps<{ task: TaskEntry }>();

const { t, te } = useI18n();
const message = useMessage();
const store = useTasksStore();
const themeVars = useThemeVars();

const kindLabel = computed(() => {
  const key = `task.kind.${props.task.kind}`;
  return te(key) ? t(key) : props.task.kind;
});

const title = computed(() =>
  props.task.label ? `${kindLabel.value} · ${props.task.label}` : kindLabel.value,
);

const percent = computed(() => progressPercent(props.task.progress));

/** 每秒更新，用于判断进度是否停滞。 */
const now = ref(Date.now());
let timer: ReturnType<typeof setInterval> | undefined;
onMounted(() => {
  timer = setInterval(() => (now.value = Date.now()), 1000);
});
onUnmounted(() => clearInterval(timer));

const stalled = computed(() => isStalled(props.task, now.value));

/** 阶段与条目 / 字节计数，例如“资源文件 · 120/3500 · 12.0 MB / 300 MB”。 */
const detail = computed(() => {
  const p = props.task.progress;
  if (!p) return t("task.status.waiting");
  const stageKey = `task.stage.${p.stage}`;
  const parts = [te(stageKey) ? t(stageKey) : p.stage];
  if (p.total > 0) parts.push(`${p.done}/${p.total}`);
  if (p.bytes_total > 0) {
    parts.push(`${formatBytes(p.bytes_done)} / ${formatBytes(p.bytes_total)}`);
  }
  return parts.join(" · ");
});

const errorText = computed(() => (props.task.error ? formatError(props.task.error) : ""));

function showError(err: unknown) {
  message.error(formatError(toAppError(err)));
}

async function cancel() {
  await store.cancel(props.task.task_id).catch(showError);
}

async function retry() {
  await store.retry(props.task.task_id).catch(showError);
}

async function copyError() {
  if (!props.task.error) return;
  try {
    await navigator.clipboard.writeText(errorDetails(errorText.value, props.task.error));
    message.success(t("task.action.copied"));
  } catch {
    message.error(t("task.action.copyFailed"));
  }
}
</script>

<template>
  <div class="task-item" :class="`task-${task.status}`">
    <div class="task-head">
      <span class="task-title" :title="title">{{ title }}</span>
      <div class="task-actions">
        <NTooltip v-if="task.status === 'running'">
          <template #trigger>
            <NButton
              quaternary
              circle
              size="small"
              :aria-label="t('task.action.cancel')"
              @click="cancel"
            >
              <template #icon
                ><NIcon><DismissCircle20Regular /></NIcon
              ></template>
            </NButton>
          </template>
          {{ t("task.action.cancel") }}
        </NTooltip>
        <template v-else>
          <NTooltip v-if="task.restart && task.status !== 'succeeded'">
            <template #trigger>
              <NButton
                quaternary
                circle
                size="small"
                :aria-label="t('task.action.retry')"
                @click="retry"
              >
                <template #icon
                  ><NIcon><ArrowClockwise20Regular /></NIcon
                ></template>
              </NButton>
            </template>
            {{ t("task.action.retry") }}
          </NTooltip>
          <NTooltip>
            <template #trigger>
              <NButton
                quaternary
                circle
                size="small"
                :aria-label="t('task.action.dismiss')"
                @click="store.dismiss(task.task_id)"
              >
                <template #icon
                  ><NIcon><Dismiss20Regular /></NIcon
                ></template>
              </NButton>
            </template>
            {{ t("task.action.dismiss") }}
          </NTooltip>
        </template>
      </div>
    </div>

    <template v-if="task.status === 'running'">
      <NProgress
        type="line"
        :percentage="percent ?? 0"
        :processing="percent === null"
        :show-indicator="false"
        :height="4"
      />
      <div class="task-meta">
        <span class="task-detail">{{ detail }}</span>
        <span v-if="stalled" class="task-stalled">{{ t("task.stalled") }}</span>
        <span v-else-if="task.progress && task.progress.speed_bps > 0">
          {{ t("task.speed", { speed: formatBytes(task.progress.speed_bps) }) }}
        </span>
      </div>
    </template>
    <div v-else-if="task.status === 'failed'" class="task-error">
      <span class="task-error-text">{{ errorText || t("task.status.failed") }}</span>
      <NButton v-if="task.error" text size="tiny" @click="copyError">
        <template #icon
          ><NIcon><Copy20Regular /></NIcon
        ></template>
        {{ t("task.action.copy") }}
      </NButton>
    </div>
    <div v-else class="task-meta">
      <span>{{ t(`task.status.${task.status}`) }}</span>
    </div>
  </div>
</template>

<style scoped>
.task-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-radius: 6px;
  background: var(--tmcl-card);
  border: 1px solid var(--tmcl-stroke);
}

.task-head {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 28px;
}

.task-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
}

.task-actions {
  display: flex;
  gap: 2px;
}

.task-meta {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.task-detail {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-stalled {
  flex-shrink: 0;
  color: v-bind("themeVars.warningColor");
}

.task-error {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 12px;
}

.task-error-text {
  flex: 1;
  color: v-bind("themeVars.errorColor");
  word-break: break-word;
}
</style>
