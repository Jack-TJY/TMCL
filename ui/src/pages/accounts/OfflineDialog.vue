<script setup lang="ts">
// 添加离线账号：输入玩家名并即时校验；离线账号仅限本地测试，弹窗内说明用途。
import { NAlert, NButton, NForm, NFormItem, NInput, NModal, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { OFFLINE_NAME_PATTERN } from "../../api/account";
import { toAppError } from "../../api/invoke";
import { formatError } from "../../i18n";
import { useAccountsStore } from "../../stores/accounts";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const message = useMessage();
const accounts = useAccountsStore();

const name = ref("");
const busy = ref(false);

watch(
  () => props.show,
  (show) => {
    if (show) name.value = "";
  },
);

const trimmed = computed(() => name.value.trim());
const valid = computed(() => OFFLINE_NAME_PATTERN.test(trimmed.value));
/** 输入过内容且不合法时才提示，避免一打开就报错。 */
const feedback = computed(() =>
  trimmed.value && !valid.value ? t("errors.AUTH_OFFLINE_INVALID_NAME") : undefined,
);

async function confirm() {
  if (!valid.value || busy.value) return;
  busy.value = true;
  try {
    const account = await accounts.addOffline(trimmed.value);
    message.success(t("accounts.offline.added", { name: account.name }));
    emit("close");
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <NModal
    :show="show"
    preset="card"
    style="width: 440px"
    :title="t('accounts.offline.title')"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(value: boolean) => !value && emit('close')"
  >
    <NAlert type="warning" :show-icon="true" class="notice">
      {{ t("accounts.offline.notice") }}
    </NAlert>
    <NForm @submit.prevent="confirm">
      <NFormItem
        :label="t('accounts.offline.name')"
        :validation-status="feedback ? 'error' : undefined"
        :feedback="feedback ?? t('accounts.offline.nameHint')"
      >
        <NInput
          v-model:value="name"
          :maxlength="16"
          :placeholder="t('accounts.offline.placeholder')"
          autofocus
          @keyup.enter="confirm"
        />
      </NFormItem>
    </NForm>
    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{ t("accounts.cancel") }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="!valid" @click="confirm">
          {{ t("accounts.offline.confirm") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.notice {
  margin-bottom: 16px;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
