<script setup lang="ts">
// 项目图标：Modrinth 给出的图标地址，没有或加载失败时显示拼图占位。
import { PuzzlePiece20Regular } from "@vicons/fluent";
import { NIcon } from "naive-ui";
import { ref, watch } from "vue";

const props = defineProps<{ url: string | null; size: number }>();

const failed = ref(false);
watch(
  () => props.url,
  () => (failed.value = false),
);
</script>

<template>
  <div class="icon" :style="{ width: `${size}px`, height: `${size}px` }">
    <img
      v-if="url && !failed"
      :src="url"
      alt=""
      loading="lazy"
      referrerpolicy="no-referrer"
      @error="failed = true"
    />
    <NIcon v-else :size="size * 0.55"><PuzzlePiece20Regular /></NIcon>
  </div>
</template>

<style scoped>
.icon {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  color: var(--tmcl-text-secondary);
  background: var(--tmcl-subtle);
  border-radius: 8px;
}

.icon img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
</style>
