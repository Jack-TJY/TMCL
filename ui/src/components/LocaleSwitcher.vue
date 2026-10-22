<script setup lang="ts">
// 界面语言选择：切换后立即生效并写入 config.json。M1-11 的设置页会复用此组件。
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../api/invoke";
import { formatError, isLocale, LOCALE_NAMES, setLocale, SUPPORTED_LOCALES } from "../i18n";

const { t, locale } = useI18n();
const error = ref<string | null>(null);

async function change(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  if (!isLocale(value)) return;
  error.value = null;
  try {
    await setLocale(value);
  } catch (err) {
    error.value = formatError(toAppError(err));
  }
}
</script>

<template>
  <label class="locale-switcher">
    {{ t("common.language.label") }}
    <select :value="locale" @change="change">
      <option v-for="l in SUPPORTED_LOCALES" :key="l" :value="l">{{ LOCALE_NAMES[l] }}</option>
    </select>
  </label>
  <p v-if="error" class="locale-error">{{ error }}</p>
</template>

<style scoped>
.locale-switcher {
  display: flex;
  gap: 8px;
  align-items: center;
}
</style>
