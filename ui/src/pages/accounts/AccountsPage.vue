<script setup lang="ts">
// 账号页（PLAN.md M1-14）：账号列表、切换启动用的账号、删除；添加微软正版（设备码）、
// 离线（仅限本地测试）与外置登录账号。默认引导正版登录（AGENTS.md §15）。
import { Add20Regular, ChevronDown16Regular } from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NDropdown,
  NEmpty,
  NIcon,
  NSpin,
  useDialog,
  useMessage,
  type DropdownOption,
} from "naive-ui";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { msLoginStatus, type Account, type MsLoginStatus } from "../../api/account";
import { toAppError } from "../../api/invoke";
import type { AppError } from "../../bindings/AppError";
import { formatError } from "../../i18n";
import DeviceCodeDialog from "../../components/device-code-dialog/DeviceCodeDialog.vue";
import { useAccountsStore } from "../../stores/accounts";
import AccountCard from "./AccountCard.vue";
import AuthlibDialog from "./AuthlibDialog.vue";
import OfflineDialog from "./OfflineDialog.vue";

type AddKind = "microsoft" | "offline" | "authlib";

const { t } = useI18n();
const message = useMessage();
const dialog = useDialog();
const store = useAccountsStore();

const loading = ref(false);
const error = ref<AppError | null>(null);
const adding = ref<AddKind | null>(null);
/** 构建时未注入 Client ID 或功能尚未完成时禁用微软登录。 */
const msStatus = ref<MsLoginStatus>("enabled");
const msEnabled = computed(() => msStatus.value === "enabled");

async function load() {
  loading.value = true;
  error.value = null;
  try {
    await store.load();
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  void load();
  msStatus.value = await msLoginStatus();
});

const addOptions = computed<DropdownOption[]>(() => [
  { key: "microsoft", label: t("accounts.add.microsoft"), disabled: !msEnabled.value },
  { key: "authlib", label: t("accounts.add.authlib") },
  { key: "offline", label: t("accounts.add.offline") },
]);

function showError(err: unknown) {
  message.error(formatError(toAppError(err)));
}

async function use(account: Account) {
  try {
    await store.setDefault(account.id);
    message.success(t("accounts.switched", { name: account.name }));
  } catch (err) {
    showError(err);
  }
}

function confirmRemove(account: Account) {
  dialog.warning({
    title: t("accounts.remove.title", { name: account.name }),
    content: t(account.kind === "offline" ? "accounts.remove.offline" : "accounts.remove.online"),
    positiveText: t("accounts.remove.confirm"),
    negativeText: t("accounts.cancel"),
    onPositiveClick: async () => {
      try {
        await store.remove(account.id);
        message.success(t("accounts.remove.done", { name: account.name }));
      } catch (err) {
        showError(err);
      }
    },
  });
}
</script>

<template>
  <div class="accounts-page">
    <h1 class="page-title">{{ t("shell.nav.accounts") }}</h1>

    <div class="toolbar">
      <span class="hint">{{ t("accounts.hint") }}</span>
      <div class="spacer" />
      <NDropdown trigger="click" :options="addOptions" @select="(key: AddKind) => (adding = key)">
        <NButton type="primary">
          <template #icon
            ><NIcon><Add20Regular /></NIcon
          ></template>
          {{ t("accounts.add.action") }}
          <NIcon class="chevron"><ChevronDown16Regular /></NIcon>
        </NButton>
      </NDropdown>
    </div>

    <NAlert
      v-if="!msEnabled"
      type="info"
      class="ms-disabled"
      :title="t('accounts.msDisabled.title')"
    >
      {{ t(`accounts.msDisabled.${msStatus}`) }}
    </NAlert>

    <NAlert v-if="error" type="error" class="load-error" :title="t('accounts.loadFailed')">
      <div class="load-error-body">
        <span>{{ formatError(error) }}</span>
        <NButton size="small" @click="load">{{ t("accounts.retry") }}</NButton>
      </div>
    </NAlert>

    <NSpin :show="loading && store.accounts.length === 0" class="list">
      <NEmpty
        v-if="!loading && !error && store.accounts.length === 0"
        class="empty"
        :description="t('accounts.empty')"
      >
        <template #extra>
          <div class="empty-actions">
            <NButton
              size="small"
              type="primary"
              :disabled="!msEnabled"
              @click="adding = 'microsoft'"
            >
              {{ t("accounts.add.microsoft") }}
            </NButton>
            <NButton size="small" @click="adding = 'authlib'">
              {{ t("accounts.add.authlib") }}
            </NButton>
            <NButton size="small" @click="adding = 'offline'">
              {{ t("accounts.add.offline") }}
            </NButton>
          </div>
        </template>
      </NEmpty>
      <div class="cards">
        <AccountCard
          v-for="account in store.accounts"
          :key="account.id"
          :account="account"
          :current="account.id === store.defaultId"
          @use="use(account)"
          @remove="confirmRemove(account)"
        />
      </div>
    </NSpin>

    <DeviceCodeDialog :show="adding === 'microsoft'" @close="adding = null" @done="load" />
    <AuthlibDialog :show="adding === 'authlib'" @close="adding = null" />
    <OfflineDialog :show="adding === 'offline'" @close="adding = null" />
  </div>
</template>

<style scoped>
.accounts-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 1200px;
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.hint {
  color: var(--tmcl-text-secondary);
}

.spacer {
  flex: 1;
}

.chevron {
  margin-left: 4px;
}

.ms-disabled,
.load-error {
  margin-bottom: 12px;
}

.load-error-body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.empty {
  padding: 48px 0;
}

.empty-actions {
  display: flex;
  gap: 8px;
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 8px;
}
</style>
