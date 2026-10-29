<script setup lang="ts">
// 预览结果：每个目标实例一项，展开后逐条列出将要修改的选项（原值 → 新值）。
import { NCheckbox, NCollapse, NCollapseItem, NTag } from "naive-ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { SyncOptionChange } from "../../bindings/SyncOptionChange";
import type { SyncPreview } from "../../bindings/SyncPreview";
import { isHiddenOption, optionLabel, valueLabel } from "./options";
import { OPTION_SPANS, spanText, supportedOn, usesLegacyKeys } from "./versions";

const props = defineProps<{ preview: SyncPreview; names: Record<string, string> }>();

const { t, te } = useI18n();
const tr = { t, te };

// 游戏中没有对应设置的选项默认不列出（仍会同步），勾选后显示。
const showHidden = ref(false);
const hiddenCount = computed(
  () =>
    props.preview.targets.flatMap((x) => x.changes).filter((c) => isHiddenOption(c.key, tr)).length,
);
const shown = (changes: SyncOptionChange[]) =>
  showHidden.value ? changes : changes.filter((c) => !isHiddenOption(c.key, tr));
</script>

<template>
  <NCheckbox v-if="hiddenCount > 0" v-model:checked="showHidden" class="hidden-toggle">
    {{ t("sync.hidden.showHidden", { count: hiddenCount }) }}
  </NCheckbox>
  <NCollapse class="preview" display-directive="show">
    <NCollapseItem
      v-for="target in preview.targets"
      :key="target.instance_id"
      :name="target.instance_id"
      :disabled="target.changes.length === 0"
    >
      <template #header>
        <span class="target-name">{{ names[target.instance_id] ?? target.instance_id }}</span>
        <span class="target-version">{{ target.game_version }}</span>
      </template>
      <template #header-extra>
        <NTag v-if="target.changes.length" size="small" type="info" :bordered="false">
          {{ t("sync.preview.changes", { count: target.changes.length }) }}
        </NTag>
        <span v-else class="unchanged">{{ t("sync.preview.unchanged") }}</span>
      </template>
      <table class="changes">
        <thead>
          <tr>
            <th>{{ t("sync.preview.option") }}</th>
            <th>{{ t("sync.preview.category") }}</th>
            <th>{{ t("sync.preview.old") }}</th>
            <th>{{ t("sync.preview.new") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="change in shown(target.changes)" :key="change.key">
            <td>
              <div>{{ optionLabel(change.key, tr) ?? change.key }}</div>
              <div v-if="optionLabel(change.key, tr)" class="mono raw">{{ change.key }}</div>
              <div class="marks">
                <NTag
                  v-if="supportedOn(change.key, target.game_version) === false"
                  size="tiny"
                  type="warning"
                  :bordered="false"
                >
                  {{ t("sync.version.missing", { version: target.game_version }) }}
                </NTag>
                <NTag
                  v-else-if="OPTION_SPANS[change.key]"
                  size="tiny"
                  :bordered="false"
                  :title="t('sync.version.range', { range: spanText(OPTION_SPANS[change.key]) })"
                >
                  {{ spanText(OPTION_SPANS[change.key]) }}
                </NTag>
                <NTag
                  v-if="change.key.startsWith('key_') && usesLegacyKeys(target.game_version)"
                  size="tiny"
                  type="info"
                  :bordered="false"
                >
                  {{ t("sync.version.legacyKeys") }}
                </NTag>
              </div>
            </td>
            <td>{{ t(`sync.category.${change.category}`) }}</td>
            <td v-if="change.old === null" class="muted">
              {{ t("sync.preview.missing") }}
            </td>
            <td v-else class="old" :title="change.old">
              {{ valueLabel(change.key, change.old, tr) ?? change.old }}
            </td>
            <td :title="change.new">{{ valueLabel(change.key, change.new, tr) ?? change.new }}</td>
          </tr>
        </tbody>
      </table>
    </NCollapseItem>
  </NCollapse>
</template>

<style scoped>
.hidden-toggle {
  margin-bottom: 8px;
}

.target-name {
  font-weight: 600;
}

.target-version,
.unchanged {
  margin-left: 8px;
  color: var(--tmcl-text-secondary);
}

.changes {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.changes th {
  text-align: left;
  font-weight: 600;
  color: var(--tmcl-text-secondary);
}

.changes th,
.changes td {
  padding: 4px 8px;
  border-bottom: 1px solid var(--tmcl-stroke);
  word-break: break-all;
}

.mono {
  font-family: Consolas, "Cascadia Mono", monospace;
}

.marks {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 2px;
}

.raw {
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.muted {
  color: var(--tmcl-text-secondary);
}

.old {
  color: var(--tmcl-text-secondary);
  text-decoration: line-through;
}
</style>
