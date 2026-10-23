<script setup lang="ts">
// 下载任务：任务中心（M1-12）的精简视图，运行中的任务在前，可选显示最近结束的任务。
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import TaskItem from "../../components/tasks/TaskItem.vue";
import { useTasksStore } from "../../stores/tasks";
import type { WidgetProps } from "../types";

/** 最多显示的任务数，更多的在标题栏的任务中心查看。 */
const MAX_TASKS = 10;

const props = defineProps<WidgetProps>();
const { t } = useI18n();
const tasks = useTasksStore();

onMounted(() => void tasks.init().catch(() => undefined));

const shown = computed(() => {
  const finished = props.settings.showFinished
    ? tasks.entries.filter((e) => e.status !== "running")
    : [];
  return [...tasks.running, ...finished].slice(0, MAX_TASKS);
});
</script>

<template>
  <p v-if="shown.length === 0" class="empty">{{ t("widget-downloads.empty") }}</p>
  <ul v-else class="tasks">
    <li v-for="task in shown" :key="task.task_id">
      <TaskItem :task="task" />
    </li>
  </ul>
</template>

<style scoped>
.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  margin: 0;
  color: var(--tmcl-text-secondary);
}

.tasks {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}
</style>
