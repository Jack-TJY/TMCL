<script setup lang="ts">
// 本地皮肤库：头像缩略图网格，点击预览，可改名、改模型、删除。
import { Delete20Regular, Edit20Regular } from "@vicons/fluent";
import { NButton, NEmpty, NIcon, NTag, NTooltip } from "naive-ui";
import { useI18n } from "vue-i18n";
import type { SkinEntry } from "../../api/skin";
import { headStyle } from "./skins";

defineProps<{
  entries: SkinEntry[];
  /** 皮肤 ID → 图片 object URL；尚未读取的没有缩略图。 */
  urls: Record<string, string>;
  selectedId: string | null;
}>();
const emit = defineEmits<{
  select: [entry: SkinEntry];
  edit: [entry: SkinEntry];
  remove: [entry: SkinEntry];
}>();

const { t } = useI18n();
</script>

<template>
  <NEmpty v-if="entries.length === 0" class="empty" :description="t('skin.library.empty')" />
  <div v-else class="grid">
    <div
      v-for="entry in entries"
      :key="entry.id"
      class="item"
      :class="{ selected: entry.id === selectedId }"
      role="button"
      tabindex="0"
      @click="emit('select', entry)"
      @keydown.enter="emit('select', entry)"
    >
      <div
        class="head"
        :style="urls[entry.id] ? headStyle(urls[entry.id], entry.height) : undefined"
        aria-hidden="true"
      />
      <div class="info">
        <span class="name" :title="entry.name">{{ entry.name }}</span>
        <NTag size="tiny" :bordered="false">{{ t(`skin.model.${entry.variant}`) }}</NTag>
      </div>
      <div class="actions" @click.stop>
        <NTooltip>
          <template #trigger>
            <NButton
              quaternary
              circle
              size="tiny"
              :aria-label="t('skin.edit.title')"
              @click="emit('edit', entry)"
            >
              <template #icon
                ><NIcon><Edit20Regular /></NIcon
              ></template>
            </NButton>
          </template>
          {{ t("skin.edit.title") }}
        </NTooltip>
        <NTooltip>
          <template #trigger>
            <NButton
              quaternary
              circle
              size="tiny"
              :aria-label="t('skin.remove.action')"
              @click="emit('remove', entry)"
            >
              <template #icon
                ><NIcon><Delete20Regular /></NIcon
              ></template>
            </NButton>
          </template>
          {{ t("skin.remove.action") }}
        </NTooltip>
      </div>
    </div>
  </div>
</template>

<style scoped>
.empty {
  padding: 32px 0;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 8px;
}

.item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 8px 8px 10px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 8px;
  background: var(--tmcl-card);
  cursor: pointer;
}

.item.selected {
  border-color: var(--tmcl-accent);
}

.head {
  flex: none;
  width: 32px;
  height: 32px;
  border-radius: 4px;
  background-color: var(--tmcl-stroke);
  background-repeat: no-repeat;
  image-rendering: pixelated;
}

.info {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  min-width: 0;
}

.name {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.actions {
  display: flex;
  opacity: 0;
  transition: opacity 0.15s;
}

.item:hover .actions,
.item:focus-within .actions {
  opacity: 1;
}
</style>
