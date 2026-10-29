<script setup lang="ts">
// 修改皮肤库中皮肤的名称与模型（Steve / Alex）。
import {
  NButton,
  NForm,
  NFormItem,
  NInput,
  NModal,
  NRadioButton,
  NRadioGroup,
  useMessage,
} from "naive-ui";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import { skinLibraryUpdate, type SkinEntry, type SkinVariant } from "../../api/skin";
import { formatError } from "../../i18n";

const props = defineProps<{ entry: SkinEntry | null }>();
const emit = defineEmits<{ close: []; saved: [entry: SkinEntry] }>();

const { t } = useI18n();
const message = useMessage();

const name = ref("");
const variant = ref<SkinVariant>("classic");
const busy = ref(false);

watch(
  () => props.entry,
  (entry) => {
    if (!entry) return;
    name.value = entry.name;
    variant.value = entry.variant;
  },
  { immediate: true },
);

async function confirm() {
  const entry = props.entry;
  const newName = name.value.trim();
  if (!entry || !newName || busy.value) return;
  busy.value = true;
  try {
    emit("saved", await skinLibraryUpdate({ id: entry.id, name: newName, variant: variant.value }));
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
    :show="entry !== null"
    preset="card"
    style="width: 420px"
    :title="t('skin.edit.title')"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <NForm @submit.prevent="confirm">
      <NFormItem :label="t('skin.edit.name')">
        <NInput v-model:value="name" :maxlength="64" autofocus @keyup.enter="confirm" />
      </NFormItem>
      <NFormItem :label="t('skin.model.label')">
        <NRadioGroup v-model:value="variant">
          <NRadioButton value="classic">{{ t("skin.model.classic") }}</NRadioButton>
          <NRadioButton value="slim">{{ t("skin.model.slim") }}</NRadioButton>
        </NRadioGroup>
      </NFormItem>
    </NForm>
    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{ t("skin.cancel") }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="!name.trim()" @click="confirm">
          {{ t("skin.edit.confirm") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
