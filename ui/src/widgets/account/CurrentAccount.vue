<script setup lang="ts">
// 当前账号：显示启动游戏时使用的账号，与账号页共用账号 store。
import { ChevronRight20Regular } from "@vicons/fluent";
import { NButton, NIcon, NTag, NTooltip } from "naive-ui";
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useAccountsStore } from "../../stores/accounts";
import type { WidgetProps } from "../types";

defineProps<WidgetProps>();
const { t } = useI18n();
const router = useRouter();
const store = useAccountsStore();
const account = computed(() => store.current);

// 读取失败时显示“没有账号”，到账号页可重试。
onMounted(() => store.ensureLoaded().catch(() => undefined));
</script>

<template>
  <div class="account">
    <div class="avatar" :class="{ empty: !account }" aria-hidden="true">
      {{ account ? account.name.slice(0, 1).toUpperCase() : "?" }}
    </div>
    <div v-if="account" class="info">
      <div class="name">{{ account.name }}</div>
      <NTag size="small" :bordered="false" :type="account.kind === 'offline' ? 'warning' : 'info'">
        {{ t(`accounts.kind.${account.kind}`) }}
      </NTag>
    </div>
    <div v-else class="info">
      <div class="none">{{ t("widget-account.none") }}</div>
    </div>
    <NTooltip>
      <template #trigger>
        <NButton
          quaternary
          circle
          :aria-label="t('widget-account.manage')"
          @click="router.push({ name: 'accounts' })"
        >
          <template #icon>
            <NIcon><ChevronRight20Regular /></NIcon>
          </template>
        </NButton>
      </template>
      {{ t("widget-account.manage") }}
    </NTooltip>
  </div>
</template>

<style scoped>
.account {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 100%;
}

.avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: var(--tmcl-accent);
  color: #fff;
  font-size: 18px;
  font-weight: 600;
}

.avatar.empty {
  background: var(--tmcl-subtle);
  color: var(--tmcl-text-secondary);
}

.none {
  color: var(--tmcl-text-secondary);
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

.name {
  max-width: 100%;
  overflow: hidden;
  font-size: 16px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
