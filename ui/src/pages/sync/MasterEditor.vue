<script setup lang="ts">
// 手动编辑主配置：按游戏中的样子用开关、下拉、滑块和按键录入修改选项，未收录的选项退回键值输入；
// 可添加、删除选项并载入大版本模板。保存时整体替换主配置（来源实例清空）。只在部分版本存在的选项标出适用版本。
import { Delete20Regular, DocumentCopy20Regular, Search20Regular } from "@vicons/fluent";
import {
  NButton,
  NCheckbox,
  NDropdown,
  NEmpty,
  NIcon,
  NInput,
  NModal,
  NScrollbar,
  NSelect,
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
import { defaultValue, KNOWN_SETTINGS } from "./controls";
import OptionControl from "./OptionControl.vue";
import { isHiddenOption, modBinding, optionLabel } from "./options";
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
const showHidden = ref(false);
/** 本次打开后新添加的行：即使键名未收录也不隐藏，否则刚加的“自定义选项”会消失。 */
const added = ref(new Set<number>());

watch(
  () => props.show,
  (show) => {
    if (!show) return;
    rows.value = toRows(props.options);
    query.value = "";
    showHidden.value = false;
    added.value = new Set();
  },
  { immediate: true },
);

const issues = computed(() => {
  const list = validateRows(rows.value);
  return new Map(rows.value.map((row, i) => [row.id, list[i]]));
});
const hidden = (row: EditorRow) => isHiddenOption(row.key, tr) && !added.value.has(row.id);
const hiddenCount = computed(() => rows.value.filter(hidden).length);
const visible = computed(() =>
  filterRows(rows.value, query.value, (key) => optionLabel(key, tr)).filter(
    (row) => showHidden.value || !hidden(row),
  ),
);
const parsed = computed(() => rowsToOptions(rows.value));
const tooMany = computed(() => rows.value.length > MAX_OPTIONS);

const CUSTOM = "\u0000custom";
const addOptions = computed(() => {
  const used = new Set(rows.value.map((r) => r.key.trim()));
  return [
    { value: CUSTOM, label: t("sync.editor.custom") },
    ...KNOWN_SETTINGS.filter((key) => !used.has(key))
      .map((key) => ({ value: key, label: optionLabel(key, tr) ?? key }))
      .sort((a, b) => a.label.localeCompare(b.label)),
  ];
});

function addRow(key: string) {
  query.value = "";
  const row = key === CUSTOM ? newRow() : newRow(key, defaultValue(key));
  added.value.add(row.id);
  rows.value.push(row);
}

/** 未收录或尚未填写键的行显示键输入框；已有的模组按键按名称显示，刚添加的自定义行一直保持输入框。 */
const isCustom = (row: EditorRow) =>
  optionLabel(row.key.trim(), tr) === null &&
  (added.value.has(row.id) || modBinding(row.key.trim()) === null);
const labelOf = (key: string) => optionLabel(key.trim(), tr) ?? modBinding(key.trim())?.name;
const modOf = (key: string) =>
  optionLabel(key.trim(), tr) === null ? (modBinding(key.trim())?.mod ?? null) : null;

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
      <NSelect
        class="add"
        :value="null"
        filterable
        :options="addOptions"
        :placeholder="t('sync.editor.add')"
        @update:value="addRow"
      />
      <NDropdown trigger="click" :options="templateOptions" @select="loadTemplate">
        <NButton secondary>
          <template #icon
            ><NIcon><DocumentCopy20Regular /></NIcon
          ></template>
          {{ t("sync.template.action") }}
        </NButton>
      </NDropdown>
    </div>
    <div v-if="hiddenCount > 0" class="hidden-toggle">
      <NCheckbox v-model:checked="showHidden">
        {{ t("sync.hidden.showHidden", { count: hiddenCount }) }}
      </NCheckbox>
      <span class="hidden-hint">{{ t("sync.hidden.hiddenHint") }}</span>
    </div>
    <NScrollbar style="max-height: 50vh">
      <NEmpty v-if="visible.length === 0" class="empty" :description="t('sync.editor.empty')" />
      <div v-for="row in visible" :key="row.id" class="row">
        <NInput
          v-if="isCustom(row)"
          v-model:value="row.key"
          class="key"
          size="small"
          :placeholder="t('sync.editor.key')"
          :status="issues.get(row.id) ? 'error' : undefined"
        />
        <div v-else class="label" :title="row.key">
          <div class="label-line">
            <span class="label-text">{{ labelOf(row.key) }}</span>
            <NTag v-if="modOf(row.key)" size="tiny" type="info" :bordered="false">
              {{ t("sync.modBinding") }} · {{ modOf(row.key) }}
            </NTag>
            <NTag
              v-if="spanOf(row.key)"
              size="tiny"
              :bordered="false"
              :title="t('sync.version.range', { range: spanOf(row.key) })"
            >
              {{ spanOf(row.key) }}
            </NTag>
          </div>
          <div class="raw">{{ row.key }}</div>
        </div>
        <OptionControl v-model:value="row.value" :option-key="row.key.trim()" />
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

.hidden-toggle {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 12px;
  margin-bottom: 8px;
}

.hidden-hint {
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.empty {
  padding: 24px 0;
}

.row {
  display: grid;
  grid-template-columns: 2fr 3fr auto;
  gap: 12px;
  align-items: center;
  padding: 4px 0;
  border-bottom: 1px solid var(--tmcl-stroke);
}

.label {
  min-width: 0;
}

.label-line {
  display: flex;
  align-items: center;
  gap: 6px;
}

.raw {
  overflow: hidden;
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 11px;
  color: var(--tmcl-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.add {
  width: 220px;
  flex-shrink: 0;
}

.label-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
