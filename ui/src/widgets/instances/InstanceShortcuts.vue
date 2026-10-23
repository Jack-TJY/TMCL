<script setup lang="ts">
// 实例快捷列表：最近游玩的若干个实例，每行可直接启动。
import { Play20Filled } from "@vicons/fluent";
import { NButton, NIcon, NSpin, NTooltip, useMessage } from "naive-ui";
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
const { t } = useI18n();
const router = useRouter();
const message = useMessage();
const games = useGameSessions();

const instances = ref<InstanceSummary[] | null>(null);

onMounted(async () => {
  try {
    instances.value = (await instanceList()).instances.filter((i) => i.error === null);
  } catch (err) {
    instances.value = [];
    message.error(formatError(toAppError(err)));
  }
});

const shown = computed(() => (instances.value ?? []).slice(0, props.settings.count as number));

async function launch(instance: InstanceSummary) {
  try {
    await games.launch({ id: instance.id, name: instance.name });
  } catch (err) {
    message.error(
      t("widget-instances.failed", { name: instance.name, reason: formatError(toAppError(err)) }),
    );
  }
}
</script>

<template>
  <NSpin v-if="instances === null" class="center" size="small" />
  <p v-else-if="shown.length === 0" class="center empty">{{ t("widget-instances.empty") }}</p>
  <div v-else class="instances">
    <ul class="list">
      <li v-for="instance in shown" :key="instance.id" class="row">
        <div class="text">
          <div class="name">{{ instance.name }}</div>
          <div class="meta">{{ versionLabel(instance) }}</div>
        </div>
        <NTooltip>
          <template #trigger>
            <NButton
              quaternary
              circle
              :type="isActive(games.sessions[instance.id]) ? 'success' : 'default'"
              :loading="games.sessions[instance.id]?.status === 'starting'"
              :disabled="isActive(games.sessions[instance.id])"
              :aria-label="t('widget-instances.play', { name: instance.name })"
              @click="launch(instance)"
            >
              <template #icon>
                <NIcon><Play20Filled /></NIcon>
              </template>
            </NButton>
          </template>
          {{ t("widget-instances.play", { name: instance.name }) }}
        </NTooltip>
      </li>
    </ul>
    <NButton text class="all" @click="router.push({ name: 'instances' })">
      {{ t("widget-instances.all") }}
    </NButton>
  </div>
</template>

<style scoped>
.center {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  margin: 0;
}

.empty {
  color: var(--tmcl-text-secondary);
}

.instances {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.list {
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: 0;
  overflow: auto;
  list-style: none;
}

.row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 0;
  border-bottom: 1px solid var(--tmcl-stroke);
}

.row:last-child {
  border-bottom: none;
}

.text {
  flex: 1;
  min-width: 0;
}

.name,
.meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  color: var(--tmcl-text-secondary);
  font-size: 12px;
}

.all {
  align-self: flex-start;
  margin-top: 8px;
  color: var(--tmcl-accent);
}
</style>
