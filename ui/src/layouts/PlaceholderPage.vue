<script setup lang="ts">
// 尚未实现的页面：显示标题和说明，由对应步骤替换。
import { NEmpty } from "naive-ui";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

const { t } = useI18n();
const route = useRoute();
</script>

<template>
  <div class="placeholder">
    <h1 class="page-title">{{ route.meta.titleKey ? t(route.meta.titleKey) : "" }}</h1>
    <NEmpty class="placeholder-body" :description="t('shell.placeholder')" />
  </div>
</template>

<style scoped>
.placeholder {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.placeholder-body {
  flex: 1;
  justify-content: center;
}
</style>
