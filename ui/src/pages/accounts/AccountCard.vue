<script setup lang="ts">
// 账号卡片：头像、玩家名、账号类型与 UUID；非当前账号可设为当前，右侧删除。
import { Checkmark16Regular, Delete20Regular } from "@vicons/fluent";
import { NButton, NIcon, NTag, NTooltip } from "naive-ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { Account } from "../../api/account";

const props = defineProps<{ account: Account; current: boolean }>();
const emit = defineEmits<{ use: []; remove: [] }>();

const { t } = useI18n();

const kindLabel = computed(() =>
  props.account.kind === "authlib" && props.account.server
    ? t("accounts.kind.authlibAt", { server: props.account.server.name })
    : t(`accounts.kind.${props.account.kind}`),
);
const kindType = computed(() => (props.account.kind === "offline" ? "warning" : "info"));
</script>

<template>
  <div class="account-card" :class="{ current }">
    <div class="avatar" aria-hidden="true">{{ account.name.slice(0, 1).toUpperCase() }}</div>
    <div class="info">
      <div class="name-row">
        <span class="name" :title="account.name">{{ account.name }}</span>
        <NTag v-if="current" size="small" type="success" :bordered="false" round>
          <template #icon
            ><NIcon><Checkmark16Regular /></NIcon
          ></template>
          {{ t("accounts.current") }}
        </NTag>
      </div>
      <NTag size="small" :type="kindType" :bordered="false">{{ kindLabel }}</NTag>
      <div class="uuid" :title="account.uuid">{{ account.uuid }}</div>
    </div>
    <div class="actions">
      <NButton v-if="!current" size="small" secondary @click="emit('use')">
        {{ t("accounts.use") }}
      </NButton>
      <NTooltip>
        <template #trigger>
          <NButton
            quaternary
            circle
            size="small"
            :aria-label="t('accounts.remove.action')"
            @click="emit('remove')"
          >
            <template #icon
              ><NIcon><Delete20Regular /></NIcon
            ></template>
          </NButton>
        </template>
        {{ t("accounts.remove.action") }}
      </NTooltip>
    </div>
  </div>
</template>

<style scoped>
.account-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 12px 12px 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 8px;
  background: var(--tmcl-card);
}

.account-card.current {
  border-color: var(--tmcl-accent);
}

.avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  background: var(--tmcl-accent);
  color: #fff;
  font-size: 20px;
  font-weight: 600;
}

.info {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  min-width: 0;
}

.info :deep(.n-tag) {
  max-width: 100%;
}

.name-row {
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: 100%;
}

.name {
  overflow: hidden;
  font-size: 15px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.uuid {
  max-width: 100%;
  overflow: hidden;
  color: var(--tmcl-text-secondary);
  font-family: ui-monospace, monospace;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.actions {
  display: flex;
  align-items: center;
  gap: 4px;
}
</style>
