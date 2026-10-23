<script setup lang="ts">
// 快捷操作：打开最近实例的游戏目录、打开数据目录、新建实例、打开设置。
import {
  Add20Regular,
  Folder20Regular,
  FolderOpen20Regular,
  Settings20Regular,
} from "@vicons/fluent";
import { NButton, NIcon, NTooltip, useMessage } from "naive-ui";
import { computed, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { instanceList, instanceOpenDir } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import { settingsOpenDataDir } from "../../api/settings";
import { formatError } from "../../i18n";
import type { WidgetProps } from "../types";

const props = defineProps<WidgetProps>();
const { t } = useI18n();
const router = useRouter();
const message = useMessage();

interface Action {
  key: string;
  icon: Component;
  run: () => Promise<unknown>;
}

async function openGameDir() {
  const latest = (await instanceList()).instances.find((i) => i.error === null);
  if (!latest) {
    message.info(t("widget-quick-actions.noInstance"));
    return;
  }
  await instanceOpenDir({ id: latest.id });
}

const actions: Action[] = [
  { key: "gameDir", icon: FolderOpen20Regular, run: openGameDir },
  { key: "dataDir", icon: Folder20Regular, run: settingsOpenDataDir },
  { key: "newInstance", icon: Add20Regular, run: () => router.push({ name: "download" }) },
  { key: "settings", icon: Settings20Regular, run: () => router.push({ name: "settings" }) },
];

const labels = computed(() => props.settings.labels !== false);

async function run(action: Action) {
  try {
    await action.run();
  } catch (err) {
    message.error(formatError(toAppError(err)));
  }
}

/** 动作名的 key 以字面量列出，供 i18n 检查识别。 */
const LABELS: Record<string, string> = {
  gameDir: "widget-quick-actions.actions.gameDir",
  dataDir: "widget-quick-actions.actions.dataDir",
  newInstance: "widget-quick-actions.actions.newInstance",
  settings: "widget-quick-actions.actions.settings",
};
</script>

<template>
  <div class="quick-actions" :class="{ compact: !labels }">
    <NTooltip v-for="action in actions" :key="action.key" :disabled="labels">
      <template #trigger>
        <NButton class="action" secondary :aria-label="t(LABELS[action.key])" @click="run(action)">
          <template #icon>
            <NIcon><component :is="action.icon" /></NIcon>
          </template>
          <span v-if="labels" class="action-label">{{ t(LABELS[action.key]) }}</span>
        </NButton>
      </template>
      {{ t(LABELS[action.key]) }}
    </NTooltip>
  </div>
</template>

<style scoped>
.quick-actions {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
  gap: 8px;
}

.quick-actions.compact {
  grid-template-columns: repeat(auto-fill, 28px);
}

.action {
  justify-content: flex-start;
  min-width: 0;
}

.action-label {
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
