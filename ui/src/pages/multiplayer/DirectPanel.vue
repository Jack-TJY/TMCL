<script setup lang="ts">
// 直连（P2P）：EasyTier + Scaffolding 联机（PLAN.md M2-12）的命令合并后在这里接通。
// 在此之前只说明这种方式，并引导需要现在联机的用户用樱花穿透。
import { NButton } from "naive-ui";
import { useI18n } from "vue-i18n";

const emit = defineEmits<{ useSakura: [] }>();
const { t } = useI18n();
</script>

<template>
  <div class="card">
    <p class="title">{{ t("multiplayer.direct.soonTitle") }}</p>
    <p class="muted">{{ t("multiplayer.direct.soonBody") }}</p>
    <NButton type="primary" @click="emit('useSakura')">{{
      t("multiplayer.direct.useSakura")
    }}</NButton>
  </div>
</template>

<style scoped>
.card {
  margin-top: 12px;
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
</style>
