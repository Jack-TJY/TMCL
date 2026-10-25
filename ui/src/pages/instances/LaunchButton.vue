<script setup lang="ts">
// 卡片上的启动按钮：未运行时“启动”，启动中显示加载，运行中显示“运行中”，安装中显示“正在安装”且不可点；
// 有日志时可打开日志窗口。
import { DocumentText20Regular, Play20Filled } from "@vicons/fluent";
import { NButton, NIcon, useMessage } from "naive-ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import { isActive, useGameSessions } from "../../components/game-log/sessions";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";

const props = defineProps<{ instance: InstanceSummary; primary?: boolean }>();
const emit = defineEmits<{ log: [instanceId: string]; launched: [] }>();

const { t } = useI18n();
const message = useMessage();
const games = useGameSessions();
const tasks = useTasksStore();

const preparing = ref(false);
const session = computed(() => games.sessions[props.instance.id]);
const running = computed(() => isActive(session.value));
const installing = computed(() => tasks.installing(props.instance.id));

async function launch() {
  if (preparing.value || running.value || installing.value) return;
  preparing.value = true;
  try {
    await games.launch({ id: props.instance.id, name: props.instance.name });
    emit("launched");
  } catch (err) {
    message.error(
      t("instances.launch.failed", {
        name: props.instance.name,
        reason: formatError(toAppError(err)),
      }),
    );
  } finally {
    preparing.value = false;
  }
}
</script>

<template>
  <span class="launch" @click.stop @keyup.enter.stop>
    <NButton
      :type="running ? 'success' : 'primary'"
      :secondary="!primary && !running"
      :size="primary ? 'medium' : 'small'"
      :loading="preparing || session?.status === 'starting'"
      :disabled="running || installing"
      @click="launch"
    >
      <template #icon
        ><NIcon><Play20Filled /></NIcon
      ></template>
      {{
        running
          ? t("instances.launch.running")
          : installing
            ? t("instances.launch.installing")
            : primary
              ? t("instances.launch.recent", { name: instance.name })
              : t("instances.launch.action")
      }}
    </NButton>
    <NButton
      v-if="session"
      :size="primary ? 'medium' : 'small'"
      quaternary
      :aria-label="t('instances.launch.log')"
      :title="t('instances.launch.log')"
      @click="emit('log', instance.id)"
    >
      <template #icon
        ><NIcon><DocumentText20Regular /></NIcon
      ></template>
    </NButton>
  </span>
</template>

<style scoped>
.launch {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.launch :deep(.n-button__content) {
  display: block;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
