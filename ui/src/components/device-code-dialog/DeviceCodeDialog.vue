<script setup lang="ts">
// 微软设备码登录弹窗（PLAN.md M1-14）：显示设备码与验证网址，可复制、打开浏览器；
// 倒计时到期前后台自动轮询，成功后关闭弹窗。关闭弹窗即取消登录。
import { Copy20Regular, Open20Regular } from "@vicons/fluent";
import { NAlert, NButton, NIcon, NModal, NSpin, useMessage } from "naive-ui";
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  isAppNotApproved,
  msLoginStart,
  openVerificationPage,
  type Account,
  type DeviceCode,
  type MsLogin,
} from "../../api/account";
import { toAppError } from "../../api/invoke";
import type { AppError } from "../../bindings/AppError";
import { formatError } from "../../i18n";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ close: []; done: [account: Account] }>();

const { t } = useI18n();
const message = useMessage();

type Phase = "requesting" | "waiting" | "expired" | "failed";

const phase = ref<Phase>("requesting");
const code = ref<DeviceCode | null>(null);
const remaining = ref(0);
const error = ref<AppError | null>(null);
let login: MsLogin | null = null;
let timer: ReturnType<typeof setInterval> | null = null;

function stopTimer() {
  if (timer) clearInterval(timer);
  timer = null;
}

/** 取消进行中的登录；之后到达的结果一律忽略。 */
function abort() {
  stopTimer();
  login?.cancel();
  login = null;
}

function startCountdown(seconds: number) {
  stopTimer();
  remaining.value = seconds;
  timer = setInterval(() => {
    remaining.value = Math.max(remaining.value - 1, 0);
    if (remaining.value === 0) {
      abort();
      phase.value = "expired";
    }
  }, 1000);
}

function start() {
  abort();
  phase.value = "requesting";
  code.value = null;
  error.value = null;
  const current = msLoginStart();
  login = current;
  current.deviceCode
    .then((value) => {
      if (login !== current) return;
      code.value = value;
      phase.value = "waiting";
      startCountdown(value.expires_in);
    })
    .catch(() => undefined);
  current.result.then(
    (account) => {
      if (login !== current) return;
      stopTimer();
      login = null;
      message.success(t("accounts.microsoft.done", { name: account.name }));
      emit("done", account);
      emit("close");
    },
    (err) => {
      if (login !== current) return;
      stopTimer();
      login = null;
      error.value = toAppError(err);
      phase.value = "failed";
    },
  );
}

watch(
  () => props.show,
  (show) => (show ? start() : abort()),
  { immediate: true },
);
onBeforeUnmount(abort);

const countdown = computed(() => {
  const m = Math.floor(remaining.value / 60);
  const s = remaining.value % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
});
const notApproved = computed(() => error.value !== null && isAppNotApproved(error.value));

async function copy() {
  if (!code.value) return;
  try {
    await navigator.clipboard.writeText(code.value.user_code);
    message.success(t("accounts.microsoft.copied"));
  } catch {
    message.error(t("accounts.microsoft.copyFailed"));
  }
}

function openBrowser() {
  if (!code.value) return;
  openVerificationPage(code.value.verification_uri).catch((err) =>
    message.error(formatError(toAppError(err))),
  );
}
</script>

<template>
  <NModal
    :show="show"
    preset="card"
    style="width: 460px"
    :title="t('accounts.microsoft.title')"
    :mask-closable="false"
    @update:show="(value: boolean) => !value && emit('close')"
  >
    <div v-if="phase === 'requesting'" class="center">
      <NSpin size="small" />
      <span>{{ t("accounts.microsoft.requesting") }}</span>
    </div>

    <template v-else-if="phase === 'waiting' && code">
      <p class="step">{{ t("accounts.microsoft.step1") }}</p>
      <div class="uri">{{ code.verification_uri }}</div>
      <p class="step">{{ t("accounts.microsoft.step2") }}</p>
      <div class="code-row">
        <span class="code">{{ code.user_code }}</span>
        <NButton secondary @click="copy">
          <template #icon
            ><NIcon><Copy20Regular /></NIcon
          ></template>
          {{ t("accounts.microsoft.copy") }}
        </NButton>
      </div>
      <div class="center waiting">
        <NSpin size="small" />
        <span>{{ t("accounts.microsoft.waiting", { time: countdown }) }}</span>
      </div>
    </template>

    <NAlert v-else-if="phase === 'expired'" type="warning">
      {{ t("accounts.microsoft.expired") }}
    </NAlert>

    <template v-else-if="phase === 'failed' && error">
      <NAlert v-if="notApproved" type="warning" :title="t('accounts.microsoft.notApproved.title')">
        <p class="explain">{{ t("accounts.microsoft.notApproved.reason") }}</p>
        <p class="explain">{{ t("accounts.microsoft.notApproved.meanwhile") }}</p>
      </NAlert>
      <NAlert v-else type="error" :title="t('accounts.microsoft.failed')">
        {{ formatError(error) }}
      </NAlert>
    </template>

    <template #footer>
      <div class="dialog-actions">
        <NButton @click="emit('close')">{{ t("accounts.cancel") }}</NButton>
        <NButton v-if="phase === 'waiting'" type="primary" @click="openBrowser">
          <template #icon
            ><NIcon><Open20Regular /></NIcon
          ></template>
          {{ t("accounts.microsoft.openBrowser") }}
        </NButton>
        <NButton
          v-else-if="phase === 'expired' || phase === 'failed'"
          type="primary"
          @click="start"
        >
          {{ t("accounts.retry") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.center {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 12px 0;
  color: var(--tmcl-text-secondary);
}

.step {
  margin: 0 0 6px;
}

.uri {
  margin-bottom: 16px;
  font-family: ui-monospace, monospace;
  user-select: all;
}

.code-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.code {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 6px;
  font-family: ui-monospace, monospace;
  font-size: 22px;
  font-weight: 600;
  letter-spacing: 2px;
  text-align: center;
  user-select: all;
}

.waiting {
  margin-top: 12px;
}

.explain {
  margin: 0 0 6px;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
