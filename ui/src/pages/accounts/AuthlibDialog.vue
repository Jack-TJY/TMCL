<script setup lang="ts">
// 外置登录（authlib-injector）：先输入皮肤站地址并读取其名称，再用皮肤站账号登录。
import { NButton, NForm, NFormItem, NInput, NModal, NSteps, NStep, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { accountAddAuthlib, authlibServerInfo, type AuthlibServer } from "../../api/account";
import { toAppError } from "../../api/invoke";
import { formatError } from "../../i18n";
import { useAccountsStore } from "../../stores/accounts";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const message = useMessage();
const accounts = useAccountsStore();

const address = ref("");
const server = ref<AuthlibServer | null>(null);
const username = ref("");
const password = ref("");
const busy = ref(false);

watch(
  () => props.show,
  (show) => {
    if (!show) return;
    address.value = "";
    server.value = null;
    username.value = "";
    password.value = "";
  },
);

const step = computed(() => (server.value ? 2 : 1));
const canLogin = computed(() => username.value.trim() !== "" && password.value !== "");

async function run(task: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  try {
    await task();
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    busy.value = false;
  }
}

function lookup() {
  const api_url = address.value.trim();
  if (!api_url) return;
  void run(async () => {
    server.value = await authlibServerInfo({ api_url });
  });
}

function login() {
  const target = server.value;
  if (!target || !canLogin.value) return;
  void run(async () => {
    const account = await accountAddAuthlib({
      api_url: target.api_url,
      username: username.value.trim(),
      password: password.value,
    });
    password.value = "";
    await accounts.load();
    message.success(t("accounts.authlib.added", { name: account.name, server: target.name }));
    emit("close");
  });
}
</script>

<template>
  <NModal
    :show="show"
    preset="card"
    style="width: 460px"
    :title="t('accounts.authlib.title')"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(value: boolean) => !value && emit('close')"
  >
    <NSteps :current="step" size="small" class="steps">
      <NStep :title="t('accounts.authlib.stepServer')" />
      <NStep :title="t('accounts.authlib.stepLogin')" />
    </NSteps>

    <NForm v-if="!server" @submit.prevent="lookup">
      <NFormItem :label="t('accounts.authlib.server')" :feedback="t('accounts.authlib.serverHint')">
        <NInput
          v-model:value="address"
          :placeholder="t('accounts.authlib.serverPlaceholder')"
          autofocus
          @keyup.enter="lookup"
        />
      </NFormItem>
    </NForm>

    <NForm v-else @submit.prevent="login">
      <div class="server">
        <span class="server-name">{{ server.name }}</span>
        <span class="server-url">{{ server.api_url }}</span>
      </div>
      <NFormItem :label="t('accounts.authlib.username')">
        <NInput v-model:value="username" autofocus :input-props="{ autocomplete: 'username' }" />
      </NFormItem>
      <NFormItem :label="t('accounts.authlib.password')">
        <NInput
          v-model:value="password"
          type="password"
          show-password-on="click"
          :input-props="{ autocomplete: 'current-password' }"
          @keyup.enter="login"
        />
      </NFormItem>
    </NForm>

    <template #footer>
      <div class="dialog-actions">
        <NButton v-if="server" :disabled="busy" @click="server = null">
          {{ t("accounts.authlib.back") }}
        </NButton>
        <div class="spacer" />
        <NButton :disabled="busy" @click="emit('close')">{{ t("accounts.cancel") }}</NButton>
        <NButton
          v-if="!server"
          type="primary"
          :loading="busy"
          :disabled="!address.trim()"
          @click="lookup"
        >
          {{ t("accounts.authlib.next") }}
        </NButton>
        <NButton v-else type="primary" :loading="busy" :disabled="!canLogin" @click="login">
          {{ t("accounts.authlib.login") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.steps {
  margin-bottom: 20px;
}

.server {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-bottom: 16px;
  padding: 8px 12px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 6px;
}

.server-name {
  font-weight: 600;
}

.server-url {
  overflow: hidden;
  color: var(--tmcl-text-secondary);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dialog-actions {
  display: flex;
  gap: 8px;
}

.spacer {
  flex: 1;
}
</style>
