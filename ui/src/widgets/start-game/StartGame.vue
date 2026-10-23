<script setup lang="ts">
// 开始游戏：显示设置中选定的实例（默认最近游玩的实例），一键启动。
import { Play20Filled } from "@vicons/fluent";
import { NButton, NIcon, NSpin, useMessage } from "naive-ui";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { instanceList } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import { isActive, useGameSessions } from "../../components/game-log/sessions";
import { formatError } from "../../i18n";
import { versionLabel } from "../instance-label";
import type { WidgetProps } from "../types";

const props = defineProps<WidgetProps>();
const { t, d } = useI18n();
const router = useRouter();
const message = useMessage();
const games = useGameSessions();

const instances = ref<InstanceSummary[] | null>(null);
const preparing = ref(false);

onMounted(async () => {
  try {
    instances.value = (await instanceList()).instances.filter((i) => i.error === null);
  } catch (err) {
    instances.value = [];
    message.error(formatError(toAppError(err)));
  }
});

/** 选定的实例不存在（例如已删除）时回退到最近游玩的实例。 */
const target = computed(() => {
  const list = instances.value ?? [];
  return list.find((i) => i.id === props.settings.instance) ?? list[0] ?? null;
});
const session = computed(() => (target.value ? games.sessions[target.value.id] : undefined));
const running = computed(() => isActive(session.value));

async function launch() {
  const instance = target.value;
  if (!instance || preparing.value || running.value) return;
  preparing.value = true;
  try {
    await games.launch({ id: instance.id, name: instance.name });
  } catch (err) {
    message.error(
      t("widget-start-game.failed", { name: instance.name, reason: formatError(toAppError(err)) }),
    );
  } finally {
    preparing.value = false;
  }
}
</script>

<template>
  <NSpin v-if="instances === null" class="center" size="small" />
  <div v-else-if="!target" class="center empty">
    <p>{{ t("widget-start-game.empty") }}</p>
    <NButton secondary @click="router.push({ name: 'download' })">
      {{ t("widget-start-game.create") }}
    </NButton>
  </div>
  <div v-else class="start-game">
    <div class="info">
      <div class="name">{{ target.name }}</div>
      <div class="meta">{{ versionLabel(target) }}</div>
      <div class="meta">
        {{
          target.last_played_at
            ? t("widget-start-game.lastPlayed", { date: d(new Date(target.last_played_at)) })
            : t("widget-start-game.neverPlayed")
        }}
      </div>
    </div>
    <NButton
      type="primary"
      size="large"
      class="play"
      :loading="preparing || session?.status === 'starting'"
      :disabled="running"
      @click="launch"
    >
      <template #icon>
        <NIcon><Play20Filled /></NIcon>
      </template>
      {{ running ? t("widget-start-game.running") : t("widget-start-game.play") }}
    </NButton>
  </div>
</template>

<style scoped>
.center {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.empty p {
  margin: 0 0 12px;
  color: var(--tmcl-text-secondary);
}

.start-game {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 12px;
  height: 100%;
}

.info {
  min-width: 0;
}

.name {
  overflow: hidden;
  font-size: 20px;
  font-weight: 600;
  line-height: 28px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  overflow: hidden;
  color: var(--tmcl-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.play {
  align-self: flex-start;
  min-width: 160px;
}
</style>
