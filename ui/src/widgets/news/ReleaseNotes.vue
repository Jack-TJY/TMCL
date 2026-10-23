<script setup lang="ts">
// 更新日志：只读本项目的 Releases。读取命令完成前由 api/__mocks__/releases.ts 提供（返回空列表）。
import { NTag } from "naive-ui";
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { mockReleaseList, type ReleaseNote } from "../../api/__mocks__/releases";
import type { WidgetProps } from "../types";

defineProps<WidgetProps>();
const { t, d } = useI18n();

const releases = ref<ReleaseNote[] | null>(null);

onMounted(async () => {
  releases.value = await mockReleaseList().catch(() => []);
});
</script>

<template>
  <p v-if="releases?.length === 0" class="empty">{{ t("widget-news.empty") }}</p>
  <ul v-else-if="releases" class="releases">
    <li v-for="release in releases" :key="release.tag" class="release">
      <span class="name">{{ release.name || release.tag }}</span>
      <NTag v-if="release.prerelease" size="small" :bordered="false" type="warning">
        {{ t("widget-news.prerelease") }}
      </NTag>
      <span class="date">{{ d(new Date(release.published_at)) }}</span>
    </li>
  </ul>
</template>

<style scoped>
.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  margin: 0;
  color: var(--tmcl-text-secondary);
}

.releases {
  margin: 0;
  padding: 0;
  list-style: none;
}

.release {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 0;
  border-bottom: 1px solid var(--tmcl-stroke);
}

.release:last-child {
  border-bottom: none;
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.date {
  color: var(--tmcl-text-secondary);
  font-size: 12px;
}
</style>
