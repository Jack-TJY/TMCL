<script setup lang="ts">
// 预览结果：每个目标实例一项，展开后逐条列出将要修改的选项（原值 → 新值）。
import { NCollapse, NCollapseItem, NTag } from "naive-ui";
import { useI18n } from "vue-i18n";
import type { SyncPreview } from "../../bindings/SyncPreview";

defineProps<{ preview: SyncPreview; names: Record<string, string> }>();

const { t } = useI18n();
</script>

<template>
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
            <th>{{ t("sync.editor.key") }}</th>
            <th>{{ t("sync.preview.category") }}</th>
            <th>{{ t("sync.preview.old") }}</th>
            <th>{{ t("sync.preview.new") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="change in target.changes" :key="change.key">
            <td class="mono">{{ change.key }}</td>
            <td>{{ t(`sync.category.${change.category}`) }}</td>
            <td class="mono" :class="change.old === null ? 'muted' : 'old'">
              {{ change.old ?? t("sync.preview.missing") }}
            </td>
            <td class="mono">{{ change.new }}</td>
          </tr>
        </tbody>
      </table>
    </NCollapseItem>
  </NCollapse>
</template>

<style scoped>
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

.muted {
  color: var(--tmcl-text-secondary);
}

.old {
  color: var(--tmcl-text-secondary);
  text-decoration: line-through;
}
</style>
