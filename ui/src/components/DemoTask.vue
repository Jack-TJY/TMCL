<script setup lang="ts">
// M0-06 的 IPC 演示：显示 app_info，并运行 app_demo_task 展示进度与结构化错误。
// 界面文字待 M0-07 的 i18n 框架就绪后迁移。
import type { UnlistenFn } from "@tauri-apps/api/event";
import { onBeforeUnmount, onMounted, ref } from "vue";
import { appDemoTask, appInfo, DEMO_TASK_KIND } from "../api/app";
import { onTaskFinished, onTaskProgress } from "../api/events";
import { toAppError } from "../api/invoke";
import type { AppDemoTaskArgs } from "../bindings/AppDemoTaskArgs";
import type { AppError } from "../bindings/AppError";
import type { AppInfo } from "../bindings/AppInfo";
import type { TaskState } from "../bindings/TaskState";

const info = ref<AppInfo | null>(null);
const running = ref(false);
const done = ref(0);
const total = ref(0);
const result = ref<TaskState | null>(null);
const error = ref<AppError | null>(null);
let taskId: string | null = null;
const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
  // 先订阅再启动任务：首条进度可能早于命令返回。
  unlisteners.push(
    await onTaskProgress((p) => {
      if (!running.value || p.kind !== DEMO_TASK_KIND) return;
      if (taskId !== null && p.task_id !== taskId) return;
      done.value = p.done;
      total.value = p.total;
    }),
    await onTaskFinished((f) => {
      if (!running.value || (taskId !== null && f.task_id !== taskId)) return;
      running.value = false;
      result.value = f.state;
      error.value = f.error ?? null;
    }),
  );
  try {
    info.value = await appInfo();
  } catch (err) {
    error.value = toAppError(err);
  }
});

onBeforeUnmount(() => unlisteners.forEach((stop) => stop()));

async function start(overrides: Partial<AppDemoTaskArgs> = {}) {
  running.value = true;
  done.value = 0;
  total.value = 0;
  result.value = null;
  error.value = null;
  taskId = null;
  try {
    taskId = (await appDemoTask({ steps: 20, step_ms: 100, ...overrides })).task_id;
  } catch (err) {
    running.value = false;
    error.value = toAppError(err);
  }
}
</script>

<template>
  <section class="demo">
    <p v-if="info">TMCL {{ info.version }} · {{ info.os }}/{{ info.arch }}</p>
    <progress :value="done" :max="total || 1" />
    <p>
      {{ done }} / {{ total }}<span v-if="result"> · {{ result }}</span>
    </p>
    <div class="actions">
      <button :disabled="running" @click="start()">app_demo_task</button>
      <button :disabled="running" @click="start({ fail_at: 10 })">fail_at = 10</button>
      <button :disabled="running" @click="start({ steps: 0 })">steps = 0</button>
    </div>
    <pre v-if="error">{{ error }}</pre>
  </section>
</template>

<style scoped>
.demo {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}

progress {
  width: 320px;
}

.actions {
  display: flex;
  gap: 8px;
}
</style>
