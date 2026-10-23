<script setup lang="ts">
// Win11 设置页式的一行：左侧图标、标题与说明，右侧控件。
import { NIcon } from "naive-ui";
import type { Component } from "vue";

defineProps<{ icon: Component; title: string; description?: string }>();
</script>

<template>
  <div class="setting-row">
    <NIcon :size="20" class="setting-icon"><component :is="icon" /></NIcon>
    <div class="setting-text">
      <div class="setting-title">{{ title }}</div>
      <div v-if="description" class="setting-description">{{ description }}</div>
    </div>
    <div class="setting-control"><slot /></div>
  </div>
</template>

<style scoped>
.setting-row {
  display: flex;
  align-items: center;
  gap: 16px;
  min-height: 68px;
  padding: 0 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
}

.setting-row + .setting-row {
  margin-top: 4px;
}

.setting-icon {
  flex-shrink: 0;
}

.setting-text {
  flex: 1;
  min-width: 0;
  padding: 12px 0;
}

.setting-title {
  font-size: 14px;
  line-height: 20px;
}

.setting-description {
  font-size: 12px;
  line-height: 16px;
  color: var(--tmcl-text-secondary);
  overflow-wrap: anywhere;
}

.setting-control {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  white-space: nowrap;
}

.setting-control > :deep(*) {
  flex-shrink: 0;
}
</style>
