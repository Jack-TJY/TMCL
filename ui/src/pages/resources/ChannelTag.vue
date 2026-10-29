<script setup lang="ts">
// 发布渠道标记：正式版不显示，Beta / Alpha 用醒目的颜色提醒。
import { NTag } from "naive-ui";
import { useI18n } from "vue-i18n";
import type { ResourceChannel } from "../../api/resources";

defineProps<{ channel: ResourceChannel }>();

const { t } = useI18n();
</script>

<template>
  <NTag
    v-if="channel !== 'release'"
    size="small"
    round
    :bordered="false"
    :type="channel === 'beta' ? 'warning' : 'error'"
  >
    {{ t(`resources.channel.${channel}`) }}
  </NTag>
</template>
