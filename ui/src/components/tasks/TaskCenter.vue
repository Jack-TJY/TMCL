<script setup lang="ts">
// 任务中心（PLAN.md M1-12）：标题栏上的按钮，显示运行中的任务数与总体进度，点击展开任务列表。
import { TaskListLtr20Regular } from "@vicons/fluent";
import { NBadge, NButton, NEmpty, NIcon, NPopover, NProgress, useThemeVars } from "naive-ui";
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useTasksStore } from "../../stores/tasks";
import TaskItem from "./TaskItem.vue";

const { t } = useI18n();
const store = useTasksStore();
const themeVars = useThemeVars();

onMounted(() => void store.init());
</script>

<template>
  <NPopover trigger="click" placement="bottom-end" :show-arrow="false" raw>
    <template #trigger>
      <button
        class="task-center-button"
        :title="t('task.center.title')"
        :aria-label="t('task.center.title')"
      >
        <NBadge :value="store.running.length" :show="store.running.length > 0" :max="99">
          <NIcon :size="16"><TaskListLtr20Regular /></NIcon>
        </NBadge>
        <NProgress
          v-if="store.overallProgress !== null"
          class="task-center-progress"
          type="line"
          :percentage="store.overallProgress * 100"
          :show-indicator="false"
          :height="2"
        />
      </button>
    </template>
    <div class="task-center-panel">
      <div class="panel-head">
        <span class="panel-title">{{ t("task.center.title") }}</span>
        <NButton
          text
          size="small"
          :disabled="store.entries.length === store.running.length"
          @click="store.clearFinished()"
        >
          {{ t("task.center.clearFinished") }}
        </NButton>
      </div>
      <div v-if="store.entries.length" class="panel-list">
        <TaskItem v-for="task in store.entries" :key="task.task_id" :task="task" />
      </div>
      <NEmpty v-else class="panel-empty" size="small" :description="t('task.center.empty')" />
    </div>
  </NPopover>
</template>

<style scoped>
.task-center-button {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  align-self: stretch;
  border: none;
  background: transparent;
  color: var(--tmcl-text);
  transition: background-color 83ms var(--tmcl-ease);
  cursor: default;
}

.task-center-button:hover {
  background: var(--tmcl-subtle);
}

.task-center-button:active {
  background: var(--tmcl-subtle-pressed);
}

.task-center-progress {
  position: absolute;
  left: 10px;
  right: 10px;
  bottom: 4px;
  width: auto;
}

.task-center-panel {
  display: flex;
  flex-direction: column;
  width: 380px;
  max-height: 480px;
  border-radius: 8px;
  border: 1px solid var(--tmcl-stroke);
  background: v-bind("themeVars.popoverColor");
  box-shadow: v-bind("themeVars.boxShadow2");
}

.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px 8px;
}

.panel-title {
  font-size: 14px;
  font-weight: 600;
}

.panel-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 4px 12px 12px;
  overflow-y: auto;
}

.panel-empty {
  padding: 24px 0 32px;
}
</style>
