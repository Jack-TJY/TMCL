<script setup lang="ts">
// 当前账号的皮肤：正版账号显示档案皮肤、上传所选皮肤、切换披风；外置登录账号打开皮肤站；
// 离线账号只能本地预览。
import { ArrowUpload20Regular, Open20Regular } from "@vicons/fluent";
import { NAlert, NButton, NIcon, NSelect, NSpin, useMessage, type SelectOption } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { Account } from "../../api/account";
import { toAppError } from "../../api/invoke";
import {
  skinLibraryImportTexture,
  skinOpenSite,
  skinProfile,
  skinSetCape,
  skinUpload,
  type ProfileSkin,
  type SkinEntry,
  type SkinProfile,
} from "../../api/skin";
import type { AppError } from "../../bindings/AppError";
import { formatError } from "../../i18n";

const props = defineProps<{
  account: Account | null;
  /** 皮肤库中选中的皮肤，可上传到正版账号。 */
  selected: SkinEntry | null;
}>();
const emit = defineEmits<{
  /** 点击“预览”：预览档案中正在使用的皮肤。 */
  previewSkin: [skin: ProfileSkin];
  /** 档案读取或修改后，其中正在使用的皮肤。 */
  activeSkin: [skin: ProfileSkin | null];
  /** 正在使用的皮肤已存入皮肤库。 */
  saved: [entry: SkinEntry];
  /** 档案中当前显示的披风地址；没有披风时为 `null`。 */
  cape: [url: string | null];
}>();

const { t } = useI18n();
const message = useMessage();

const profile = ref<SkinProfile | null>(null);
const loading = ref(false);
const error = ref<AppError | null>(null);
const busy = ref(false);
const saving = ref(false);

const activeSkin = computed(() => profile.value?.skins.find((s) => s.active) ?? null);
const activeCape = computed(() => profile.value?.capes.find((c) => c.active) ?? null);

const HIDDEN = "";
const capeOptions = computed<SelectOption[]>(() => [
  { value: HIDDEN, label: t("skin.account.noCape") },
  ...(profile.value?.capes ?? []).map((c) => ({ value: c.id, label: c.alias })),
]);

function apply(next: SkinProfile) {
  profile.value = next;
  emit("activeSkin", activeSkin.value);
  emit("cape", activeCape.value?.url ?? null);
}

async function load() {
  profile.value = null;
  error.value = null;
  emit("cape", null);
  emit("activeSkin", null);
  const account = props.account;
  if (account?.kind !== "microsoft") return;
  loading.value = true;
  try {
    const next = await skinProfile({ account_id: account.id });
    if (props.account?.id === account.id) apply(next);
  } catch (err) {
    if (props.account?.id === account.id) error.value = toAppError(err);
  } finally {
    loading.value = false;
  }
}

watch(() => props.account?.id, load, { immediate: true });

async function run(action: () => Promise<SkinProfile>, done: string) {
  if (busy.value) return;
  busy.value = true;
  try {
    apply(await action());
    message.success(done);
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    busy.value = false;
  }
}

function upload() {
  const account = props.account;
  const skin = props.selected;
  if (!account || !skin) return;
  void run(
    () => skinUpload({ account_id: account.id, skin_id: skin.id }),
    t("skin.account.uploaded", { name: skin.name }),
  );
}

function setCape(id: string) {
  const account = props.account;
  if (!account) return;
  void run(
    // 省略 `cape_id` 即隐藏披风。
    () =>
      skinSetCape(
        id === HIDDEN ? { account_id: account.id } : { account_id: account.id, cape_id: id },
      ),
    t("skin.account.capeChanged"),
  );
}

/** 把正在使用的皮肤存入皮肤库，方便换成别的皮肤后再换回来。 */
async function saveCurrent() {
  const account = props.account;
  const skin = activeSkin.value;
  if (!account || !skin || saving.value) return;
  saving.value = true;
  try {
    const entry = await skinLibraryImportTexture({
      url: skin.url,
      name: t("skin.account.savedName", { name: account.name }),
      variant: skin.variant,
    });
    emit("saved", entry);
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    saving.value = false;
  }
}

async function openSite() {
  if (!props.account) return;
  try {
    await skinOpenSite({ account_id: props.account.id });
  } catch (err) {
    message.error(formatError(toAppError(err)));
  }
}
</script>

<template>
  <section class="account-panel">
    <h2 class="section-title">{{ t("skin.account.title") }}</h2>

    <p v-if="!account" class="hint">{{ t("skin.account.none") }}</p>

    <template v-else-if="account.kind === 'microsoft'">
      <div class="name">{{ account.name }}</div>
      <NSpin :show="loading" size="small">
        <NAlert v-if="error" type="error" class="error">
          <div class="error-body">
            <span>{{ formatError(error) }}</span>
            <NButton size="small" @click="load">{{ t("skin.retry") }}</NButton>
          </div>
        </NAlert>
        <div v-else class="rows">
          <div class="row">
            <span class="label">{{ t("skin.account.current") }}</span>
            <NButton
              v-if="activeSkin"
              size="small"
              secondary
              @click="emit('previewSkin', activeSkin)"
            >
              {{ t("skin.account.previewCurrent") }}
            </NButton>
            <NButton
              v-if="activeSkin"
              size="small"
              secondary
              :loading="saving"
              @click="saveCurrent"
            >
              {{ t("skin.account.saveCurrent") }}
            </NButton>
            <span v-else class="hint">{{ t("skin.account.noSkin") }}</span>
          </div>
          <div class="row">
            <span class="label">{{ t("skin.account.cape") }}</span>
            <NSelect
              class="cape-select"
              size="small"
              :options="capeOptions"
              :value="activeCape?.id ?? HIDDEN"
              :disabled="busy || !profile || profile.capes.length === 0"
              @update:value="setCape"
            />
          </div>
          <NButton type="primary" :disabled="!selected || !profile" :loading="busy" @click="upload">
            <template #icon
              ><NIcon><ArrowUpload20Regular /></NIcon
            ></template>
            {{
              selected
                ? t("skin.account.upload", { name: selected.name })
                : t("skin.account.selectFirst")
            }}
          </NButton>
        </div>
      </NSpin>
    </template>

    <template v-else-if="account.kind === 'authlib'">
      <div class="name">{{ account.name }}</div>
      <p class="hint">{{ t("skin.account.authlib") }}</p>
      <NButton @click="openSite">
        <template #icon
          ><NIcon><Open20Regular /></NIcon
        ></template>
        {{ t("skin.account.openSite") }}
      </NButton>
    </template>

    <template v-else>
      <div class="name">{{ account.name }}</div>
      <p class="hint">{{ t("skin.account.offline") }}</p>
    </template>
  </section>
</template>

<style scoped>
.account-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 8px;
  background: var(--tmcl-card);
}

.section-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}

.name {
  font-weight: 600;
}

.hint {
  margin: 0;
  color: var(--tmcl-text-secondary);
}

.rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.label {
  min-width: 64px;
  color: var(--tmcl-text-secondary);
}

.cape-select {
  flex: 1;
}

.error-body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
</style>
