<script setup lang="ts">
// 下载界面的镜像来源说明：MCIM 条款要求标明来源，整合包还会经 BMCLAPI 安装加载器。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

const props = defineProps<{
  /** 是否同时说明加载器的 BMCLAPI 镜像（整合包安装）。 */
  loaders?: boolean;
}>();

const { t } = useI18n();
const settings = useSettingsStore();

const note = computed(() => {
  if (settings.settings.download.source === "official") return "";
  const mcim = t("resources.mirror.mcim");
  return props.loaders ? `${mcim}${t("resources.mirror.loaders")}` : mcim;
});
</script>

<template>
  <p v-if="note" class="mirror-note">{{ note }}</p>
</template>

<style scoped>
.mirror-note {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}
</style>
