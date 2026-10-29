<script setup lang="ts">
// 未选实例时安装资源：先选择装到哪个实例。按搜索结果中的游戏版本与加载器标出不兼容的实例；
// 没有实例或没有兼容的实例时，可以为它一键新建实例。
import { NButton, NEmpty, NModal, NRadio, NRadioGroup } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ResourceHit, ResourceKind } from "../../api/resources";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import { incompatibility, loaderName, type Incompatibility } from "./targets";

const props = defineProps<{
  hit: ResourceHit | null;
  kind: ResourceKind;
  instances: InstanceSummary[];
}>();
const emit = defineEmits<{ close: []; pick: [instance: InstanceSummary]; create: [] }>();

const { t } = useI18n();

const rows = computed(() => {
  const hit = props.hit;
  if (!hit) return [];
  return props.instances
    .map((instance) => ({ instance, problem: incompatibility(hit, props.kind, instance) }))
    .sort((a, b) => Number(a.problem !== null) - Number(b.problem !== null));
});
const compatible = computed(() => rows.value.filter((r) => r.problem === null));

const choice = ref<string | null>(null);
watch(
  () => props.hit,
  () => {
    choice.value = compatible.value[0]?.instance.id ?? null;
  },
);

function problemText(problem: Incompatibility): string {
  switch (problem.reason) {
    case "version":
      return t("resources.pick.version", { version: problem.version });
    case "noLoader":
      return t("resources.pick.noLoader");
    case "loader":
      return t("resources.pick.loader", { loaders: problem.loaders.map(loaderName).join(" / ") });
  }
}

function describe(instance: InstanceSummary): string {
  const loader = instance.loader ? loaderName(instance.loader.kind) : t("resources.global.vanilla");
  return `${instance.game_version} · ${loader}`;
}

function confirm() {
  const row = rows.value.find((r) => r.instance.id === choice.value);
  if (row) emit("pick", row.instance);
}
</script>

<template>
  <NModal
    :show="hit !== null"
    preset="card"
    style="width: 520px"
    :title="t('resources.pick.title', { name: hit?.title ?? '' })"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <NEmpty v-if="instances.length === 0" class="empty" :description="t('resources.pick.none')" />
    <template v-else>
      <div v-if="compatible.length === 0" class="hint">{{ t("resources.pick.noCompatible") }}</div>
      <NRadioGroup v-model:value="choice" class="list">
        <label
          v-for="{ instance, problem } in rows"
          :key="instance.id"
          class="row"
          :class="{ disabled: problem }"
        >
          <NRadio :value="instance.id" :disabled="problem !== null" />
          <span class="row-body">
            <span class="row-name">{{ instance.name }}</span>
            <span class="row-meta">{{ describe(instance) }}</span>
          </span>
          <span v-if="problem" class="row-problem">{{ problemText(problem) }}</span>
        </label>
      </NRadioGroup>
    </template>
    <template #footer>
      <div class="dialog-actions">
        <NButton
          :type="compatible.length === 0 ? 'primary' : 'default'"
          class="create"
          @click="emit('create')"
        >
          {{ t("resources.pick.create") }}
        </NButton>
        <NButton @click="emit('close')">{{ t("resources.install.cancel") }}</NButton>
        <NButton type="primary" :disabled="choice === null" @click="confirm">
          {{ t("resources.pick.next") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.empty {
  padding: 24px 0;
}

.hint {
  margin-bottom: 8px;
  font-size: 13px;
  color: var(--tmcl-text-secondary);
}

.list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  max-height: 360px;
  overflow-y: auto;
}

.row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border-radius: 6px;
  cursor: pointer;
}

.row:hover {
  background: var(--tmcl-subtle);
}

.row.disabled {
  cursor: default;
  opacity: 0.6;
}

.row-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.row-name {
  font-weight: 600;
}

.row-meta,
.row-problem {
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.dialog-actions {
  display: flex;
  gap: 8px;
}

.create {
  margin-right: auto;
}
</style>
