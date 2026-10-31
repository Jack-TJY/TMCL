<script setup lang="ts">
// 延迟标签：`undefined` 为还在测量，`null` 为连不上。
import { NTag } from "naive-ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { latencyLevel } from "./multiplayer";

const props = defineProps<{ latency: number | null | undefined }>();
const { t } = useI18n();

const TYPES = { good: "success", fair: "warning", poor: "error", unreachable: "error" } as const;

const type = computed(() =>
  props.latency === undefined ? "default" : TYPES[latencyLevel(props.latency)],
);
const text = computed(() => {
  if (props.latency === undefined) return t("multiplayer.latency.measuring");
  if (props.latency === null) return t("multiplayer.latency.unreachable");
  return t("multiplayer.latency.value", { ms: props.latency });
});
</script>

<template>
  <NTag size="small" :type="type" :bordered="false">{{ text }}</NTag>
</template>
