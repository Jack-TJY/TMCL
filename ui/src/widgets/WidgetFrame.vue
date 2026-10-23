<script setup lang="ts">
// 组件外框（docs/contracts/widgets.md 第 6 节）：Win11 风格卡片 + 标题栏；
// 编辑模式下标题栏右侧显示设置与删除按钮，内容区被遮罩覆盖，整张卡片都可拖动。
import { Delete20Regular, Settings20Regular } from "@vicons/fluent";
import { NButton, NIcon, NTooltip } from "naive-ui";
import { useI18n } from "vue-i18n";
import type { WidgetDefinition } from "./types";

defineProps<{
  def: WidgetDefinition;
  editing: boolean;
}>();

const emit = defineEmits<{
  configure: [];
  remove: [];
}>();

const { t } = useI18n();
</script>

<template>
  <section class="widget-frame tmcl-card" :class="{ editing }" :aria-label="t(def.titleKey)">
    <header class="widget-header">
      <NIcon :size="16" class="widget-icon"><component :is="def.icon" /></NIcon>
      <h2 class="widget-title">{{ t(def.titleKey) }}</h2>
      <div v-if="editing" class="widget-tools">
        <NTooltip v-if="def.settingsSchema?.length">
          <template #trigger>
            <NButton
              quaternary
              circle
              size="small"
              :aria-label="t('home.widget.configure')"
              @click="emit('configure')"
            >
              <template #icon>
                <NIcon><Settings20Regular /></NIcon>
              </template>
            </NButton>
          </template>
          {{ t("home.widget.configure") }}
        </NTooltip>
        <NTooltip>
          <template #trigger>
            <NButton
              quaternary
              circle
              size="small"
              :aria-label="t('home.widget.remove')"
              @click="emit('remove')"
            >
              <template #icon>
                <NIcon><Delete20Regular /></NIcon>
              </template>
            </NButton>
          </template>
          {{ t("home.widget.remove") }}
        </NTooltip>
      </div>
    </header>
    <div class="widget-body" :inert="editing || undefined">
      <slot />
    </div>
    <div v-if="editing" class="widget-mask" aria-hidden="true" />
  </section>
</template>

<style scoped>
.widget-frame {
  position: relative;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  height: 100%;
  overflow: hidden;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 8px;
  background: var(--tmcl-card);
  box-shadow: var(--tmcl-shadow);
  transition:
    border-color 83ms var(--tmcl-ease),
    box-shadow 167ms var(--tmcl-ease);
}

.widget-frame.editing {
  cursor: move;
}

.widget-frame.editing:hover {
  border-color: var(--tmcl-accent);
}

.widget-header {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  height: 40px;
  padding: 0 8px 0 16px;
}

.widget-icon {
  flex-shrink: 0;
  color: var(--tmcl-text-secondary);
}

.widget-title {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.widget-tools {
  display: flex;
  gap: 2px;
  /* 工具按钮在遮罩之上。 */
  position: relative;
  z-index: 1;
}

.widget-body {
  flex: 1;
  min-height: 0;
  padding: 0 16px 16px;
  overflow: auto;
}

.widget-mask {
  position: absolute;
  inset: 40px 0 0;
}
</style>
