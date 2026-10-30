<script setup lang="ts">
// 樱花穿透账号（PLAN.md M2-15）：首次使用先确认服务条款，再填写访问密钥；已保存时显示账号信息。
// 密钥只经 `sakura_save_key` 交给后端存入系统密钥库，界面不保存也不回显。
import { Key20Regular, SignOut20Regular } from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NIcon,
  NInput,
  NModal,
  NSpin,
  NTag,
  useDialog,
  useMessage,
} from "naive-ui";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import {
  sakuraClearKey,
  sakuraSaveKey,
  sakuraTerms,
  sakuraUser,
  type SakuraPolicy,
  type SakuraUser,
} from "../../api/sakura";
import type { AppError } from "../../bindings/AppError";
import { formatBytes } from "../../components/tasks/format";
import { formatError } from "../../i18n";
import { acceptTerms, termsAccepted } from "./multiplayer";

const props = defineProps<{ hasKey: boolean }>();
/** `user` 为 `null` 表示还不能开房（没有密钥、密钥失效或读取失败）。 */
const emit = defineEmits<{ user: [user: SakuraUser | null]; keyChanged: [] }>();

const { t } = useI18n();
const message = useMessage();
const dialog = useDialog();

const user = ref<SakuraUser | null>(null);
const loading = ref(false);
const loadError = ref<AppError | null>(null);
const terms = ref<SakuraPolicy | null>(null);
const termsError = ref<AppError | null>(null);
const showTerms = ref(false);
const accepted = ref(false);
const key = ref("");
const saving = ref(false);
const editing = ref(false);

/** 没有密钥（或正在更换）时显示填写表单。 */
const needKey = computed(() => !props.hasKey || editing.value);

function setUser(value: SakuraUser | null) {
  user.value = value;
  emit("user", value);
}

async function loadUser() {
  loading.value = true;
  loadError.value = null;
  try {
    setUser(await sakuraUser());
  } catch (err) {
    loadError.value = toAppError(err);
    setUser(null);
  } finally {
    loading.value = false;
  }
}

async function loadTerms() {
  termsError.value = null;
  try {
    terms.value = await sakuraTerms();
    accepted.value = termsAccepted(terms.value.updated);
  } catch (err) {
    termsError.value = toAppError(err);
  }
}

onMounted(() => {
  if (props.hasKey) {
    // 已保存过密钥说明之前同意过条款。
    accepted.value = true;
    void loadUser();
  } else {
    void loadTerms();
  }
});

function agree() {
  if (terms.value) acceptTerms(terms.value.updated);
  accepted.value = true;
  showTerms.value = false;
}

async function saveKey() {
  const value = key.value.trim();
  if (!value || saving.value) return;
  saving.value = true;
  try {
    const saved = await sakuraSaveKey(value);
    key.value = "";
    editing.value = false;
    loadError.value = null;
    setUser(saved);
    emit("keyChanged");
    message.success(t("multiplayer.sakura.account.saved", { name: saved.name }));
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    saving.value = false;
  }
}

function confirmClear() {
  dialog.warning({
    title: t("multiplayer.sakura.account.clearTitle"),
    content: t("multiplayer.sakura.account.clearContent"),
    positiveText: t("multiplayer.sakura.account.clear"),
    negativeText: t("multiplayer.cancel"),
    onPositiveClick: async () => {
      try {
        await sakuraClearKey();
        setUser(null);
        emit("keyChanged");
      } catch (err) {
        message.error(formatError(toAppError(err)));
      }
    },
  });
}
</script>

<template>
  <div class="card">
    <template v-if="needKey && !accepted">
      <p class="title">{{ t("multiplayer.sakura.terms.title") }}</p>
      <p class="muted">{{ t("multiplayer.sakura.terms.intro") }}</p>
      <NAlert v-if="termsError" type="error" class="gap">
        {{ formatError(termsError) }}
        <NButton size="small" text type="primary" @click="loadTerms">{{
          t("multiplayer.retry")
        }}</NButton>
      </NAlert>
      <NButton type="primary" :disabled="!terms" @click="showTerms = true">
        {{ t("multiplayer.sakura.terms.read") }}
      </NButton>
    </template>

    <template v-else-if="needKey">
      <p class="title">{{ t("multiplayer.sakura.account.keyTitle") }}</p>
      <p class="muted">{{ t("multiplayer.sakura.account.keyHelp") }}</p>
      <div class="row">
        <NInput
          v-model:value="key"
          class="key-input"
          type="password"
          show-password-on="click"
          :input-props="{ autocomplete: 'off', spellcheck: false }"
          :placeholder="t('multiplayer.sakura.account.keyPlaceholder')"
          @keyup.enter="saveKey"
        />
        <NButton type="primary" :disabled="!key.trim()" :loading="saving" @click="saveKey">
          <template #icon
            ><NIcon><Key20Regular /></NIcon
          ></template>
          {{ t("multiplayer.sakura.account.save") }}
        </NButton>
        <NButton v-if="hasKey" quaternary @click="editing = false">{{
          t("multiplayer.cancel")
        }}</NButton>
      </div>
      <p class="muted small">{{ t("multiplayer.sakura.account.keyPrivacy") }}</p>
    </template>

    <NSpin v-else :show="loading">
      <NAlert v-if="loadError" type="error" :title="formatError(loadError)">
        <div class="row">
          <NButton size="small" @click="loadUser">{{ t("multiplayer.retry") }}</NButton>
          <NButton size="small" @click="editing = true">{{
            t("multiplayer.sakura.account.change")
          }}</NButton>
        </div>
      </NAlert>
      <div v-else-if="user" class="account">
        <div class="info">
          <span class="title">{{ user.name }}</span>
          <NTag size="small" :bordered="false">{{ user.group }}</NTag>
          <NTag v-if="!user.verified" size="small" type="warning" :bordered="false">
            {{ t("multiplayer.sakura.account.unverified") }}
          </NTag>
          <span class="muted">
            {{
              t("multiplayer.sakura.account.summary", {
                speed: user.speed,
                traffic: formatBytes(user.traffic_left),
              })
            }}
          </span>
        </div>
        <div class="row">
          <NButton size="small" quaternary @click="editing = true">
            <template #icon
              ><NIcon><Key20Regular /></NIcon
            ></template>
            {{ t("multiplayer.sakura.account.change") }}
          </NButton>
          <NButton size="small" quaternary @click="confirmClear">
            <template #icon
              ><NIcon><SignOut20Regular /></NIcon
            ></template>
            {{ t("multiplayer.sakura.account.clear") }}
          </NButton>
        </div>
      </div>
      <NAlert v-if="user && !user.verified" type="warning" class="gap">
        {{ t("multiplayer.sakura.account.verifyHint") }}
        <NButton size="small" text type="primary" @click="loadUser">{{
          t("multiplayer.retry")
        }}</NButton>
      </NAlert>
    </NSpin>

    <NModal
      v-model:show="showTerms"
      preset="card"
      style="width: 640px"
      :title="t('multiplayer.sakura.terms.dialogTitle', { updated: terms?.updated ?? '' })"
    >
      <NAlert v-if="terms?.alert" type="warning" class="gap">{{ terms.alert }}</NAlert>
      <div class="terms">{{ terms?.content }}</div>
      <template #footer>
        <div class="footer">
          <NButton @click="showTerms = false">{{ t("multiplayer.cancel") }}</NButton>
          <NButton type="primary" @click="agree">{{ t("multiplayer.sakura.terms.agree") }}</NButton>
        </div>
      </template>
    </NModal>
  </div>
</template>

<style scoped>
.card {
  padding: 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
}

.title {
  margin: 0;
  font-weight: 600;
}

.muted {
  margin: 4px 0 12px;
  color: var(--tmcl-text-secondary);
}

.small {
  margin: 8px 0 0;
  font-size: 12px;
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.key-input {
  width: 360px;
}

.account {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.info {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.info .muted {
  margin: 0;
}

.gap {
  margin: 12px 0 0;
}

.terms {
  max-height: 50vh;
  overflow-y: auto;
  white-space: pre-wrap;
  font-size: 13px;
  line-height: 20px;
}

.footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
