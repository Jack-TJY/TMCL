<script setup lang="ts">
// 从 .tmclpack 导入实例：填写包的绝对路径，导入任务进入任务中心，完成后列表刷新。
import { NButton, NForm, NFormItem, NInput, NModal, useMessage } from "naive-ui";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { INSTANCE_TASK_KINDS, instanceImport } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import type { TaskId } from "../../bindings/TaskId";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ close: []; started: [taskId: TaskId] }>();

const { t } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const pack = ref("");
const busy = ref(false);

watch(
  () => props.show,
  (show) => {
    if (show) pack.value = "";
  },
);

async function confirm() {
  // 从资源管理器“复制为路径”得到的路径带双引号。
  const path = pack.value.trim().replace(/^"(.*)"$/, "$1");
  if (!path || busy.value) return;
  busy.value = true;
  try {
    const taskId = await tasks.start({
      kind: INSTANCE_TASK_KINDS.import,
      label: path.split(/[\\/]/).pop() ?? path,
      start: () => instanceImport({ pack: path }),
    });
    message.success(t("instances.import.started"));
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
    :show="show"
    preset="card"
    style="width: 480px"
    :title="t('instances.import.title')"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(value: boolean) => !value && emit('close')"
  >
    <NForm @submit.prevent="confirm">
      <NFormItem :label="t('instances.import.pack')">
        <NInput
          v-model:value="pack"
          :placeholder="t('instances.pathPlaceholder')"
          autofocus
          @keyup.enter="confirm"
        />
      </NFormItem>
    </NForm>
    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{ t("instances.cancel") }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="!pack.trim()" @click="confirm">
          {{ t("instances.import.confirm") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
