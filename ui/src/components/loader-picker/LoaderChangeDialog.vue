<script setup lang="ts">
// 为已有实例后装、更换或移除加载器。安装在任务中心进行，成功后实例的加载器才会改变；
// 改回原版直接保存配置。确认前提示该操作会修改实例。
import { NAlert, NButton, NModal, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { instanceSave } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import { loaderInstall, LOADER_INSTALL_KIND } from "../../api/loader";
import type { InstanceConfig } from "../../bindings/InstanceConfig";
import type { LoaderSpec } from "../../bindings/LoaderSpec";
import type { TaskId } from "../../bindings/TaskId";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import LoaderPicker from "./LoaderPicker.vue";

const props = defineProps<{ instance: InstanceConfig | null }>();
const emit = defineEmits<{
  close: [];
  /** 已改回原版并保存。 */
  saved: [config: InstanceConfig];
  /** 已开始安装任务，任务成功后实例的加载器才会改变。 */
  started: [taskId: TaskId];
}>();

const { t } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const loader = ref<LoaderSpec | null>(null);
const busy = ref(false);

watch(
  () => props.instance,
  (instance) => {
    loader.value = instance?.loader ? { ...instance.loader } : null;
  },
  { immediate: true },
);

const unchanged = computed(() => {
  const current = props.instance?.loader ?? null;
  const next = loader.value;
  return current === null || next === null
    ? current === next
    : current.kind === next.kind && current.version === next.version;
});

async function confirm() {
  const instance = props.instance;
  if (!instance || unchanged.value || busy.value) return;
  const next = loader.value;
  busy.value = true;
  try {
    if (next === null) {
      emit("saved", await instanceSave({ config: { ...instance, loader: null } }));
      message.success(t("loaderPicker.change.removed"));
    } else {
      const taskId = await tasks.start({
        kind: LOADER_INSTALL_KIND,
        label: instance.name,
        start: () => loaderInstall({ instance_id: instance.id, ...next }),
      });
      emit("started", taskId);
      message.success(t("loaderPicker.change.started", { name: instance.name }));
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
    :show="instance !== null"
    preset="card"
    style="width: 560px"
    :title="t('loaderPicker.change.title', { name: instance?.name ?? '' })"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <div class="body">
      <LoaderPicker
        v-model="loader"
        :game-version="instance?.game_version ?? null"
        :disabled="busy"
      />
      <NAlert type="warning" :show-icon="true">
        {{ t("loaderPicker.change.warning") }}
      </NAlert>
    </div>
    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{
          t("loaderPicker.change.cancel")
        }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="unchanged" @click="confirm">
          {{ t("loaderPicker.change.confirm") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
