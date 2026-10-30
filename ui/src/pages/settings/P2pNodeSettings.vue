<script setup lang="ts">
// 设置页“联机节点”（PLAN.md M2-12）：是否使用内置的 EasyTier 公共节点，以及用户自己的节点。
// 自定义节点排在内置节点之前；保存时后端逐项校验（P2P_NODE_INVALID）。
import { Add24Regular, Delete24Regular, Globe24Regular, Server24Regular } from "@vicons/fluent";
import { NButton, NInput, NSwitch, useMessage, useThemeVars } from "naive-ui";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import { isValidNode, p2pInfo } from "../../api/p2p";
import { formatError } from "../../i18n";
import { useSettingsStore, type Settings } from "../../stores/settings";
import SettingRow from "./SettingRow.vue";

/** 与后端 `MAX_CUSTOM_NODES` 一致。 */
const MAX_CUSTOM_NODES = 16;

const { t } = useI18n();
const message = useMessage();
const store = useSettingsStore();
const themeVars = useThemeVars();

const defaultNodes = ref<string[]>([]);
const draft = ref("");

onMounted(async () => {
  // 浏览器中调试时没有后端，不显示内置节点列表。
  defaultNodes.value = (await p2pInfo().catch(() => null))?.default_nodes ?? [];
});

const p2p = computed(() => store.settings.p2p);
const trimmed = computed(() => draft.value.trim());
const draftError = computed(() => {
  if (!trimmed.value) return null;
  if (!isValidNode(trimmed.value)) return t("settings.p2p.custom.invalid");
  if (p2p.value.custom_nodes.includes(trimmed.value)) return t("settings.p2p.custom.duplicate");
  return null;
});
const canAdd = computed(
  () =>
    trimmed.value !== "" &&
    draftError.value === null &&
    p2p.value.custom_nodes.length < MAX_CUSTOM_NODES,
);
const noNodes = computed(() => !p2p.value.use_default_nodes && p2p.value.custom_nodes.length === 0);

function save(change: (draft: Settings) => void) {
  return store.update(change).catch((err: unknown) => {
    message.error(formatError(toAppError(err)));
    throw err;
  });
}

async function addNode() {
  if (!canAdd.value) return;
  const node = trimmed.value;
  await save((s) => s.p2p.custom_nodes.push(node))
    .then(() => (draft.value = ""))
    .catch(() => {});
}

function removeNode(node: string) {
  save((s) => (s.p2p.custom_nodes = s.p2p.custom_nodes.filter((n) => n !== node))).catch(() => {});
}
</script>

<template>
  <SettingRow
    :icon="Globe24Regular"
    :title="t('settings.p2p.defaults.label')"
    :description="t('settings.p2p.defaults.description')"
  >
    <NSwitch
      :value="p2p.use_default_nodes"
      @update:value="(v: boolean) => save((s) => (s.p2p.use_default_nodes = v)).catch(() => {})"
    />
  </SettingRow>
  <ul v-if="p2p.use_default_nodes && defaultNodes.length" class="node-list secondary">
    <li v-for="node in defaultNodes" :key="node">{{ node }}</li>
  </ul>
  <SettingRow
    :icon="Server24Regular"
    :title="t('settings.p2p.custom.label')"
    :description="noNodes ? t('settings.p2p.custom.empty') : t('settings.p2p.custom.description')"
  >
    <NInput
      v-model:value="draft"
      class="control-node"
      clearable
      :status="draftError ? 'error' : undefined"
      :placeholder="t('settings.p2p.custom.placeholder')"
      @keyup.enter="addNode"
    />
    <NButton :disabled="!canAdd" @click="addNode">
      <template #icon><Add24Regular /></template>
      {{ t("settings.p2p.custom.add") }}
    </NButton>
  </SettingRow>
  <p v-if="draftError" class="draft-error" :style="{ color: themeVars.errorColor }">
    {{ draftError }}
  </p>
  <SettingRow v-for="node in p2p.custom_nodes" :key="node" :icon="Server24Regular" :title="node">
    <NButton quaternary :title="t('settings.p2p.custom.remove')" @click="removeNode(node)">
      <template #icon><Delete24Regular /></template>
    </NButton>
  </SettingRow>
</template>

<style scoped>
.control-node {
  width: 280px;
}

.node-list {
  margin: 0;
  padding: 4px 16px 8px 52px;
  font-size: 12px;
  line-height: 20px;
  list-style: none;
}

.secondary {
  color: var(--tmcl-text-secondary);
}

.draft-error {
  margin: 4px 16px;
  font-size: 12px;
}
</style>
