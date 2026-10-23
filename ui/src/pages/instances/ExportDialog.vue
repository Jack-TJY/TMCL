<script setup lang="ts">
// 导出实例为 .tmclpack：选择要包含的 game/ 条目并填写目标文件路径，导出任务进入任务中心。
// 目标路径默认放在数据目录下，可直接编辑或用系统“另存为”对话框选择。
import {
  NButton,
  NCheckbox,
  NCheckboxGroup,
  NEmpty,
  NForm,
  NFormItem,
  NInput,
  NInputGroup,
  NModal,
  NSpin,
  useMessage,
} from "naive-ui";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { pickSavePath } from "../../api/dialog";
import { INSTANCE_TASK_KINDS, instanceExport, instanceGameEntries } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import { settingsGet } from "../../api/settings";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import { joinPath, packFileName } from "./instances";

/** 默认不勾选的条目：日志与缓存通常没有分享价值。 */
const UNCHECKED = new Set(["logs", "crash-reports", ".cache", "screenshots"]);

const PACK_FILTERS = [{ name: "TMCL", extensions: ["tmclpack"] }];

const props = defineProps<{ instance: InstanceSummary | null }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const entries = ref<string[] | null>(null);
const include = ref<string[]>([]);
const dest = ref("");
const busy = ref(false);

watch(
  () => props.instance,
  async (instance) => {
    entries.value = null;
    if (!instance) return;
    const [listed, snapshot] = await Promise.all([
      instanceGameEntries({ id: instance.id }).catch((err: unknown) => {
        message.error(formatError(toAppError(err)));
        return { entries: [] };
      }),
      settingsGet().catch(() => null),
    ]);
    if (props.instance !== instance) return;
    entries.value = listed.entries;
    include.value = listed.entries.filter((e) => !UNCHECKED.has(e));
    dest.value = snapshot ? joinPath(snapshot.data_dir, packFileName(instance.name)) : "";
  },
  { immediate: true },
);

async function browse() {
  try {
    const picked = await pickSavePath(PACK_FILTERS, dest.value.trim() || undefined);
    if (picked) dest.value = picked;
  } catch (err) {
    message.error(formatError(toAppError(err)));
  }
}

async function confirm() {
  const instance = props.instance;
  const path = dest.value.trim();
  if (!instance || !path || busy.value) return;
  busy.value = true;
  try {
    const args = { id: instance.id, dest: path, include: [...include.value] };
    await tasks.start({
      kind: INSTANCE_TASK_KINDS.export,
      label: instance.name,
      start: () => instanceExport(args),
    });
    message.success(t("instances.export.started", { name: instance.name }));
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
    style="width: 480px"
    :title="t('instances.export.title', { name: instance?.name ?? '' })"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <NSpin :show="entries === null">
      <NForm @submit.prevent="confirm">
        <NFormItem :label="t('instances.export.include')">
          <NEmpty
            v-if="entries && entries.length === 0"
            size="small"
            :description="t('instances.export.noEntries')"
          />
          <NCheckboxGroup v-else v-model:value="include">
            <div class="entries">
              <NCheckbox
                v-for="entry in entries ?? []"
                :key="entry"
                :value="entry"
                :label="entry"
              />
            </div>
          </NCheckboxGroup>
        </NFormItem>
        <NFormItem :label="t('instances.export.dest')">
          <NInputGroup>
            <NInput v-model:value="dest" :placeholder="t('instances.pathPlaceholder')" />
            <NButton @click="browse">{{ t("instances.browse") }}</NButton>
          </NInputGroup>
        </NFormItem>
      </NForm>
    </NSpin>
    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{ t("instances.cancel") }}</NButton>
        <NButton
          type="primary"
          :loading="busy"
          :disabled="entries === null || !dest.trim()"
          @click="confirm"
        >
          {{ t("instances.export.confirm") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.entries {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 12px;
  max-height: 200px;
  overflow-y: auto;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
