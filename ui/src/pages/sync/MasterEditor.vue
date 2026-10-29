<script setup lang="ts">
// 手动编辑主配置：逐行修改键与值、增删行，可载入大版本模板；保存时整体替换主配置（来源实例清空）。
// 只在部分版本存在的选项标出适用版本。
import {
  Add20Regular,
  Delete20Regular,
  DocumentCopy20Regular,
  Search20Regular,
} from "@vicons/fluent";
import {
  NButton,
  NDropdown,
  NEmpty,
  NIcon,
  NInput,
  NModal,
  NScrollbar,
  NTag,
  useDialog,
  useMessage,
} from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import { syncSaveMaster } from "../../api/sync";
import type { SyncOption } from "../../bindings/SyncOption";
import type { SyncProfile } from "../../bindings/SyncProfile";
import { formatError } from "../../i18n";
import { optionLabel, valueLabel } from "./options";
import {
  filterRows,
  MAX_OPTIONS,
  newRow,
  rowsToOptions,
  toRows,
  validateRows,
  type EditorRow,
} from "./sync";
import { mergeOptions, SYNC_TEMPLATES, templateNoteKey } from "./templates";
import { OPTION_SPANS, spanText } from "./versions";

const props = defineProps<{ show: boolean; options: SyncOption[] }>();
const emit = defineEmits<{ close: []; saved: [profile: SyncProfile] }>();

const { t, te } = useI18n();
const tr = { t, te };
const message = useMessage();
const dialog = useDialog();

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
const visible = computed(() => filterRows(rows.value, query.value, (key) => optionLabel(key, tr)));
const parsed = computed(() => rowsToOptions(rows.value));
const tooMany = computed(() => rows.value.length > MAX_OPTIONS);

function addRow() {
  query.value = "";
  rows.value.push(newRow());
}

function removeRow(id: number) {
  rows.value = rows.value.filter((r) => r.id !== id);
}

const templateOptions = computed(() =>
  SYNC_TEMPLATES.map((tpl) => ({
    key: tpl.version,
    label: `${tpl.version} · ${t(templateNoteKey(tpl.version))}`,
  })),
);

function loadTemplate(version: string) {
  const tpl = SYNC_TEMPLATES.find((x) => x.version === version);
  if (!tpl) return;
  const done = (options: SyncOption[]) => {
    rows.value = toRows(options);
    query.value = "";
    message.success(t("sync.template.loaded", { version }));
  };
  const current = rows.value.map((r) => ({ key: r.key.trim(), value: r.value }));
  if (current.length === 0) return done(tpl.options);
  dialog.info({
    title: t("sync.template.title", { version }),
    content: t("sync.template.content", { version, count: tpl.options.length }),
    positiveText: t("sync.template.merge"),
    negativeText: t("sync.template.replace"),
    onPositiveClick: () => done(mergeOptions(current, tpl.options)),
    onNegativeClick: () => done(tpl.options),
  });
}

function spanOf(key: string) {
  const span = OPTION_SPANS[key.trim()];
  return span ? spanText(span) : null;
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
    style="width: 860px; max-width: 90vw"
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
      <NDropdown trigger="click" :options="templateOptions" @select="loadTemplate">
        <NButton secondary>
          <template #icon
            ><NIcon><DocumentCopy20Regular /></NIcon
          ></template>
          {{ t("sync.template.action") }}
        </NButton>
      </NDropdown>
    </div>
    <NScrollbar style="max-height: 50vh">
      <NEmpty v-if="visible.length === 0" class="empty" :description="t('sync.editor.empty')" />
      <div v-for="row in visible" :key="row.id" class="row">
        <div class="label">
          <span class="label-text">{{
            optionLabel(row.key.trim(), tr) ?? t("sync.editor.unknown")
          }}</span>
          <NTag
            v-if="spanOf(row.key)"
            size="tiny"
            :bordered="false"
            :title="t('sync.version.range', { range: spanOf(row.key) })"
          >
            {{ spanOf(row.key) }}
          </NTag>
        </div>
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
        >
          <template v-if="valueLabel(row.key.trim(), row.value, tr)" #suffix>
            <span class="value-label">{{ valueLabel(row.key.trim(), row.value, tr) }}</span>
          </template>
        </NInput>
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

.empty {
  padding: 24px 0;
}

.row {
  display: grid;
  grid-template-columns: 3fr 3fr 3fr auto;
  gap: 6px;
  align-items: center;
  padding: 3px 0;
}

.label {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.label-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.value-label {
  font-size: 12px;
  color: var(--tmcl-text-secondary);
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
