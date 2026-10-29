<script setup lang="ts">
// 手动编辑主配置：逐行修改键与值、增删行，保存时整体替换主配置（来源实例清空）。
import { Add20Regular, Delete20Regular, Search20Regular } from "@vicons/fluent";
import { NButton, NEmpty, NIcon, NInput, NModal, NScrollbar, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import { syncSaveMaster } from "../../api/sync";
import type { SyncOption } from "../../bindings/SyncOption";
import type { SyncProfile } from "../../bindings/SyncProfile";
import { formatError } from "../../i18n";
import {
  filterRows,
  MAX_OPTIONS,
  newRow,
  rowsToOptions,
  toRows,
  validateRows,
  type EditorRow,
} from "./sync";

const props = defineProps<{ show: boolean; options: SyncOption[] }>();
const emit = defineEmits<{ close: []; saved: [profile: SyncProfile] }>();

const { t } = useI18n();
const message = useMessage();

const rows = ref<EditorRow[]>([]);
const query = ref("");
const busy = ref(false);

watch(
  () => props.show,
  (show) => {
    if (!show) return;
    rows.value = toRows(props.options);
    query.value = "";
  },
  { immediate: true },
);

const issues = computed(() => {
  const list = validateRows(rows.value);
  return new Map(rows.value.map((row, i) => [row.id, list[i]]));
});
const visible = computed(() => filterRows(rows.value, query.value));
const parsed = computed(() => rowsToOptions(rows.value));
const tooMany = computed(() => rows.value.length > MAX_OPTIONS);

function addRow() {
  query.value = "";
  rows.value.push(newRow());
}

function removeRow(id: number) {
  rows.value = rows.value.filter((r) => r.id !== id);
}

async function save() {
  const opts = parsed.value;
  if (!opts || busy.value) return;
  busy.value = true;
  try {
    const profile = await syncSaveMaster({ options: opts });
    message.success(t("sync.editor.saved"));
    emit("saved", profile);
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
    style="width: 720px; max-width: 90vw"
    :title="t('sync.editor.title')"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(value: boolean) => !value && emit('close')"
  >
    <p class="hint">{{ t("sync.editor.hint") }}</p>
    <div class="toolbar">
      <NInput v-model:value="query" clearable :placeholder="t('sync.editor.search')">
        <template #prefix
          ><NIcon><Search20Regular /></NIcon
        ></template>
      </NInput>
      <NButton secondary @click="addRow">
        <template #icon
          ><NIcon><Add20Regular /></NIcon
        ></template>
        {{ t("sync.editor.add") }}
      </NButton>
    </div>
    <NScrollbar class="rows">
      <NEmpty v-if="visible.length === 0" class="empty" :description="t('sync.editor.empty')" />
      <div v-for="row in visible" :key="row.id" class="row">
        <NInput
          v-model:value="row.key"
          class="key"
          size="small"
          :placeholder="t('sync.editor.key')"
          :status="issues.get(row.id) ? 'error' : undefined"
        />
        <NInput
          v-model:value="row.value"
          class="value"
          size="small"
          :placeholder="t('sync.editor.value')"
        />
        <NButton
          quaternary
          size="small"
          :title="t('sync.editor.remove')"
          @click="removeRow(row.id)"
        >
          <template #icon
            ><NIcon><Delete20Regular /></NIcon
          ></template>
        </NButton>
        <div v-if="issues.get(row.id)" class="issue">
          {{ t(`sync.editor.issue.${issues.get(row.id)}`) }}
        </div>
      </div>
    </NScrollbar>
    <template #footer>
      <div class="dialog-actions">
        <span class="count" :class="{ error: tooMany }">
          {{ t("sync.editor.count", { count: rows.length, max: MAX_OPTIONS }) }}
        </span>
        <NButton :disabled="busy" @click="emit('close')">{{ t("sync.cancel") }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="!parsed" @click="save">
          {{ t("sync.editor.save") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.hint {
  margin: 0 0 12px;
  color: var(--tmcl-text-secondary);
}

.toolbar {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}

.rows {
  max-height: 50vh;
}

.empty {
  padding: 24px 0;
}

.row {
  display: grid;
  grid-template-columns: 2fr 3fr auto;
  gap: 6px;
  align-items: center;
  padding: 3px 0;
}

.key :deep(input) {
  font-family: Consolas, "Cascadia Mono", monospace;
}

.issue {
  grid-column: 1 / -1;
  font-size: 12px;
  color: #c42b1c;
}

.dialog-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.count {
  margin-right: auto;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.count.error {
  color: #c42b1c;
}
</style>
