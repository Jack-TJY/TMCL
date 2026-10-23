<script setup lang="ts">
// 当前账号：显示启动游戏时使用的账号。账号管理（M1-08）完成前只有离线账号（仅限本地测试）。
import { ChevronRight20Regular } from "@vicons/fluent";
import { NButton, NIcon, NTag, NTooltip } from "naive-ui";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { mockLaunchAccount } from "../../api/__mocks__/account";
import type { WidgetProps } from "../types";

defineProps<WidgetProps>();
const { t } = useI18n();
const router = useRouter();
const account = mockLaunchAccount();
</script>

<template>
  <div class="account">
    <div class="avatar" aria-hidden="true">{{ account.name.slice(0, 1).toUpperCase() }}</div>
    <div class="info">
      <div class="name">{{ account.name }}</div>
      <NTag size="small" :bordered="false" type="warning">
        {{ t("widget-account.offline") }}
      </NTag>
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
