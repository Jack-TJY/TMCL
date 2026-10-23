<script setup lang="ts">
// 重命名或复制实例：输入新名称；复制时可选择是否包含存档，复制任务进入任务中心。
import { NButton, NCheckbox, NForm, NFormItem, NInput, NModal, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { INSTANCE_TASK_KINDS, instanceDuplicate, instanceRename } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import type { TaskId } from "../../bindings/TaskId";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import { copyName } from "./instances";

const props = defineProps<{
  request: { mode: "rename" | "duplicate"; instance: InstanceSummary } | null;
  existing: string[];
}>();
const emit = defineEmits<{ close: []; renamed: []; started: [taskId: TaskId] }>();

const { t } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const name = ref("");
const includeSaves = ref(true);
const busy = ref(false);

watch(
  () => props.request,
  (request) => {
    if (!request) return;
    const current = request.instance.name;
    name.value = request.mode === "rename" ? current : copyName(current, props.existing);
    includeSaves.value = true;
  },
  { immediate: true },
);

const mode = computed(() => props.request?.mode ?? "rename");

async function confirm() {
  const request = props.request;
  const newName = name.value.trim();
  if (!request || !newName || busy.value) return;
  busy.value = true;
  try {
    const { id } = request.instance;
    if (request.mode === "rename") {
      await instanceRename({ id, name: newName });
      emit("renamed");
    } else {
      const include_saves = includeSaves.value;
      const taskId = await tasks.start({
        kind: INSTANCE_TASK_KINDS.duplicate,
        label: newName,
        start: () => instanceDuplicate({ id, name: newName, include_saves }),
      });
      message.success(t("instances.duplicate.started", { name: newName }));
      emit("started", taskId);
    }
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
    :show="request !== null"
    preset="card"
    style="width: 420px"
    :title="t(`instances.${mode}.title`, { name: request?.instance.name ?? '' })"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <NForm @submit.prevent="confirm">
      <NFormItem :label="t('instances.name')">
        <NInput v-model:value="name" :maxlength="64" autofocus @keyup.enter="confirm" />
      </NFormItem>
      <NCheckbox v-if="mode === 'duplicate'" v-model:checked="includeSaves">
        {{ t("instances.duplicate.includeSaves") }}
      </NCheckbox>
    </NForm>
    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{ t("instances.cancel") }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="!name.trim()" @click="confirm">
          {{ t(`instances.${mode}.confirm`) }}
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
