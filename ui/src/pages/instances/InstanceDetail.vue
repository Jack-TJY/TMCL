<script setup lang="ts">
// 实例详情抽屉：版本与加载器（只读）、分组、Java、内存、JVM / 游戏参数、窗口；可打开游戏目录。
import { FolderOpen20Regular } from "@vicons/fluent";
import {
  NAlert,
  NAutoComplete,
  NButton,
  NDescriptions,
  NDescriptionsItem,
  NDrawer,
  NDrawerContent,
  NForm,
  NFormItem,
  NIcon,
  NInput,
  NInputNumber,
  NRadioButton,
  NRadioGroup,
  NSpin,
  NSwitch,
  useMessage,
} from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { instanceGet, instanceSave } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import type { AppError } from "../../bindings/AppError";
import type { InstanceConfig } from "../../bindings/InstanceConfig";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import { formatError } from "../../i18n";
import { formatArgs, parseArgs } from "./instances";

const MEMORY_MIN_MB = 256;
const MEMORY_MAX_MB = 32768;

const props = defineProps<{ instance: InstanceSummary | null; groups: string[] }>();
const emit = defineEmits<{ close: []; saved: []; openDir: [id: string] }>();

const { t, locale } = useI18n();
const message = useMessage();

const config = ref<InstanceConfig | null>(null);
/** 读取或保存后的配置，用于回退被清空的数字框。 */
const original = ref<InstanceConfig | null>(null);
const loadError = ref<AppError | null>(null);
const saving = ref(false);

// 表单中以文本编辑的字段，保存时再转换。
const group = ref("");
const javaMode = ref<"auto" | "path">("auto");
const javaPath = ref("");
const jvmArgs = ref("");
const gameArgs = ref("");

watch(
  () => props.instance?.id,
  async (id) => {
    config.value = null;
    original.value = null;
    loadError.value = null;
    if (!id) return;
    try {
      const loaded = await instanceGet({ id });
      if (props.instance?.id !== id) return;
      config.value = loaded;
      original.value = structuredClone(loaded);
      group.value = loaded.group ?? "";
      javaMode.value = loaded.java.mode;
      javaPath.value = loaded.java.mode === "path" ? loaded.java.path : "";
      jvmArgs.value = formatArgs(loaded.jvm_args);
      gameArgs.value = formatArgs(loaded.game_args);
    } catch (err) {
      loadError.value = toAppError(err);
    }
  },
  { immediate: true },
);

const groupOptions = computed(() =>
  props.groups.filter((g) => g.toLowerCase().includes(group.value.trim().toLowerCase())),
);

const loaderText = computed(() => {
  const loader = config.value?.loader;
  return loader
    ? `${t(`instances.loader.${loader.kind}`)} ${loader.version}`
    : t("instances.loader.vanilla");
});

function formatDate(value: string | null | undefined): string {
  if (!value) return t("instances.neverPlayed");
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleString(locale.value);
}

function orElse(value: number | null, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

const javaPathMissing = computed(() => javaMode.value === "path" && !javaPath.value.trim());

async function save() {
  const current = config.value;
  const loaded = original.value;
  if (!current || !loaded || saving.value || javaPathMissing.value) return;
  // 数字框被清空时值为 null，回退到读取时的值。
  const max = orElse(current.memory.max_mb, loaded.memory.max_mb);
  const memory = {
    min_mb: Math.min(orElse(current.memory.min_mb, loaded.memory.min_mb), max),
    max_mb: max,
  };
  const window = {
    width: orElse(current.window.width, loaded.window.width),
    height: orElse(current.window.height, loaded.window.height),
    fullscreen: current.window.fullscreen,
  };
  const next: InstanceConfig = {
    ...current,
    window,
    group: group.value.trim() || null,
    java:
      javaMode.value === "auto" ? { mode: "auto" } : { mode: "path", path: javaPath.value.trim() },
    memory,
    jvm_args: parseArgs(jvmArgs.value),
    game_args: parseArgs(gameArgs.value),
  };
  saving.value = true;
  try {
    const saved = await instanceSave({ config: next });
    config.value = saved;
    original.value = structuredClone(saved);
    message.success(t("instances.detail.saved"));
    emit("saved");
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <NDrawer
    :show="instance !== null"
    :width="480"
    placement="right"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <NDrawerContent :title="instance?.name" closable>
      <NAlert v-if="loadError" type="error" :title="t('instances.detail.loadFailed')">
        {{ formatError(loadError) }}
      </NAlert>
      <NSpin v-else :show="config === null">
        <template v-if="config">
          <NDescriptions :column="1" label-placement="left" size="small" class="summary">
            <NDescriptionsItem :label="t('instances.detail.version')">
              {{ config.game_version }}
            </NDescriptionsItem>
            <NDescriptionsItem :label="t('instances.detail.loader')">
              {{ loaderText }}
            </NDescriptionsItem>
            <NDescriptionsItem :label="t('instances.detail.created')">
              {{ formatDate(config.created_at) }}
            </NDescriptionsItem>
            <NDescriptionsItem :label="t('instances.detail.lastPlayed')">
              {{ formatDate(config.last_played_at) }}
            </NDescriptionsItem>
          </NDescriptions>

          <NForm label-placement="top" size="small" @submit.prevent="save">
            <NFormItem :label="t('instances.detail.group')">
              <NAutoComplete
                v-model:value="group"
                :options="groupOptions"
                :placeholder="t('instances.ungrouped')"
                clearable
              />
            </NFormItem>
            <NFormItem
              :label="t('instances.detail.java')"
              :feedback="javaPathMissing ? t('instances.detail.javaPathRequired') : undefined"
              :validation-status="javaPathMissing ? 'error' : undefined"
            >
              <div class="stack">
                <NRadioGroup v-model:value="javaMode" size="small">
                  <NRadioButton value="auto">{{ t("instances.detail.javaAuto") }}</NRadioButton>
                  <NRadioButton value="path">{{ t("instances.detail.javaPath") }}</NRadioButton>
                </NRadioGroup>
                <NInput
                  v-if="javaMode === 'path'"
                  v-model:value="javaPath"
                  :placeholder="t('instances.detail.javaPathPlaceholder')"
                />
              </div>
            </NFormItem>
            <NFormItem :label="t('instances.detail.memory')">
              <div class="row">
                <NInputNumber
                  v-model:value="config.memory.min_mb"
                  :min="MEMORY_MIN_MB"
                  :max="config.memory.max_mb"
                  :step="256"
                  :show-button="false"
                >
                  <template #prefix>{{ t("instances.detail.memoryMin") }}</template>
                  <template #suffix>MB</template>
                </NInputNumber>
                <NInputNumber
                  v-model:value="config.memory.max_mb"
                  :min="MEMORY_MIN_MB"
                  :max="MEMORY_MAX_MB"
                  :step="256"
                  :show-button="false"
                >
                  <template #prefix>{{ t("instances.detail.memoryMax") }}</template>
                  <template #suffix>MB</template>
                </NInputNumber>
              </div>
            </NFormItem>
            <NFormItem :label="t('instances.detail.jvmArgs')">
              <NInput
                v-model:value="jvmArgs"
                type="textarea"
                :autosize="{ minRows: 1, maxRows: 4 }"
                placeholder="-XX:+UseG1GC"
              />
            </NFormItem>
            <NFormItem :label="t('instances.detail.gameArgs')">
              <NInput
                v-model:value="gameArgs"
                type="textarea"
                :autosize="{ minRows: 1, maxRows: 4 }"
              />
            </NFormItem>
            <NFormItem :label="t('instances.detail.window')">
              <div class="row">
                <NInputNumber
                  v-model:value="config.window.width"
                  :min="320"
                  :max="16384"
                  :disabled="config.window.fullscreen"
                  :show-button="false"
                >
                  <template #prefix>{{ t("instances.detail.width") }}</template>
                </NInputNumber>
                <NInputNumber
                  v-model:value="config.window.height"
                  :min="240"
                  :max="16384"
                  :disabled="config.window.fullscreen"
                  :show-button="false"
                >
                  <template #prefix>{{ t("instances.detail.height") }}</template>
                </NInputNumber>
                <label class="switch">
                  <NSwitch v-model:value="config.window.fullscreen" size="small" />
                  {{ t("instances.detail.fullscreen") }}
                </label>
              </div>
            </NFormItem>
          </NForm>
        </template>
      </NSpin>
      <template #footer>
        <div class="footer">
          <NButton :disabled="!instance" @click="instance && emit('openDir', instance.id)">
            <template #icon
              ><NIcon><FolderOpen20Regular /></NIcon
            ></template>
            {{ t("instances.action.open") }}
          </NButton>
          <div class="spacer" />
          <NButton @click="emit('close')">{{ t("instances.cancel") }}</NButton>
          <NButton
            type="primary"
            :loading="saving"
            :disabled="!config || javaPathMissing"
            @click="save"
          >
            {{ t("instances.detail.save") }}
          </NButton>
        </div>
      </template>
    </NDrawerContent>
  </NDrawer>
</template>

<style scoped>
.summary {
  margin-bottom: 16px;
}

.stack {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
}

.row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.row > :deep(.n-input-number) {
  flex: 1;
}

.switch {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  white-space: nowrap;
}

.footer {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.spacer {
  flex: 1;
}
</style>
