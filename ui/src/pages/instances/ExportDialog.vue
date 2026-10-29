<script setup lang="ts">
// 导出实例为 .tmclpack、Modrinth 整合包（.mrpack）或 CurseForge 整合包（.zip，构建带 CurseForge
// Key 时才显示）：选择要包含的 game/ 条目并填写目标文件路径，
// 导出任务进入任务中心。目标路径默认放在数据目录下，可直接编辑或用系统“另存为”对话框选择。
// 整合包还要填写名称、版本与简介；能在 Modrinth / CurseForge 找到的 mod 由后端写成下载链接。
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
  NRadioButton,
  NRadioGroup,
  NSpin,
  useMessage,
} from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { pickSavePath } from "../../api/dialog";
import { INSTANCE_TASK_KINDS, instanceExport, instanceGameEntries } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import {
  RESOURCES_TASK_KINDS,
  resourcesModpackExport,
  resourcesSources,
} from "../../api/resources";
import { settingsGet } from "../../api/settings";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import {
  type PackFormat,
  defaultInclude,
  joinPath,
  packFileName,
  withPackExtension,
} from "./instances";

const PACK_FILTERS: Record<PackFormat, { name: string; extensions: string[] }[]> = {
  tmclpack: [{ name: "TMCL", extensions: ["tmclpack"] }],
  mrpack: [{ name: "Modrinth", extensions: ["mrpack"] }],
  curseforge: [{ name: "CurseForge", extensions: ["zip"] }],
};

const props = defineProps<{ instance: InstanceSummary | null }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const entries = ref<string[] | null>(null);
const include = ref<string[]>([]);
const dest = ref("");
const busy = ref(false);
const format = ref<PackFormat>("tmclpack");
const packName = ref("");
const packVersion = ref("1.0.0");
const summary = ref("");

const isModpack = computed(() => format.value !== "tmclpack");
const curseforge = ref(false);
resourcesSources()
  .then((sources) => (curseforge.value = sources.curseforge))
  .catch(() => (curseforge.value = false));

/** 切换格式时换掉目标扩展名，并按新格式重置默认勾选。 */
watch(format, (next) => {
  dest.value = withPackExtension(dest.value.trim(), next);
  if (entries.value) include.value = defaultInclude(entries.value, next);
});

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
    include.value = defaultInclude(listed.entries, format.value);
    packName.value = instance.name;
    dest.value = snapshot
      ? joinPath(snapshot.data_dir, packFileName(instance.name, format.value))
      : "";
  },
  { immediate: true },
);

async function browse() {
  try {
    const picked = await pickSavePath(PACK_FILTERS[format.value], dest.value.trim() || undefined);
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
    const selected = [...include.value];
    if (isModpack.value) {
      const args = {
        instance_id: instance.id,
        dest: path,
        include: selected,
        name: packName.value.trim(),
        version: packVersion.value.trim(),
        summary: summary.value.trim(),
        format: format.value === "curseforge" ? ("curseforge" as const) : ("mrpack" as const),
      };
      await tasks.start({
        kind: RESOURCES_TASK_KINDS.modpackExport,
        label: instance.name,
        start: () => resourcesModpackExport(args),
      });
    } else {
      const args = { id: instance.id, dest: path, include: selected };
      await tasks.start({
        kind: INSTANCE_TASK_KINDS.export,
        label: instance.name,
        start: () => instanceExport(args),
      });
    }
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
        <NFormItem :label="t('instances.export.format')">
          <NRadioGroup v-model:value="format" size="small">
            <NRadioButton value="tmclpack">{{ t("instances.export.tmclpack") }}</NRadioButton>
            <NRadioButton value="mrpack">{{ t("instances.export.mrpack") }}</NRadioButton>
            <NRadioButton v-if="curseforge" value="curseforge">{{
              t("instances.export.curseforge")
            }}</NRadioButton>
          </NRadioGroup>
        </NFormItem>
        <template v-if="isModpack">
          <div class="modpack-fields">
            <NFormItem :label="t('instances.export.packName')">
              <NInput v-model:value="packName" :placeholder="instance?.name" />
            </NFormItem>
            <NFormItem :label="t('instances.export.packVersion')">
              <NInput v-model:value="packVersion" placeholder="1.0.0" />
            </NFormItem>
          </div>
          <NFormItem :label="t('instances.export.summary')">
            <NInput
              v-model:value="summary"
              :placeholder="t('instances.export.summaryPlaceholder')"
            />
          </NFormItem>
        </template>
        <NFormItem :label="t('instances.export.include')">
          <NEmpty
            v-if="entries && entries.length === 0"
            size="small"
            :description="t('instances.export.noEntries')"
          />
          <div v-else class="include">
            <NCheckboxGroup v-model:value="include">
              <div class="entries">
                <NCheckbox
                  v-for="entry in entries ?? []"
                  :key="entry"
                  :value="entry"
                  :label="entry"
                />
              </div>
            </NCheckboxGroup>
            <p v-if="format === 'mrpack'" class="hint">{{ t("instances.export.mrpackHint") }}</p>
            <p v-else-if="format === 'curseforge'" class="hint">
              {{ t("instances.export.curseforgeHint") }}
            </p>
          </div>
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

.include {
  width: 100%;
}

.modpack-fields {
  display: grid;
  grid-template-columns: 2fr 1fr;
  gap: 0 12px;
}

.hint {
  margin: 6px 0 0;
  font-size: 12px;
  opacity: 0.7;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
