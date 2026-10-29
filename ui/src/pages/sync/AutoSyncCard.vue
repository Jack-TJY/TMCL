<script setup lang="ts">
// 新实例自动同步：开关与类别，修改后立即保存。实际写入发生在实例首次启动前（后端 launch_start）。
import { NCheckbox, NCheckboxGroup, NSwitch, useMessage } from "naive-ui";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import { syncGetAuto, syncSetAuto } from "../../api/sync";
import type { SyncAutoSettings } from "../../bindings/SyncAutoSettings";
import type { SyncCategory } from "../../bindings/SyncCategory";
import { formatError } from "../../i18n";
import { DEFAULT_CATEGORIES, SYNC_CATEGORIES } from "./sync";

defineProps<{ hasMaster: boolean }>();

const { t } = useI18n();
const message = useMessage();

const settings = ref<SyncAutoSettings>({ enabled: false, categories: [...DEFAULT_CATEGORIES] });
const saving = ref(false);

onMounted(async () => {
  try {
    settings.value = await syncGetAuto();
  } catch (err) {
    message.error(formatError(toAppError(err)));
  }
});

const categoryOptions = computed(() =>
  SYNC_CATEGORIES.map((value) => ({ value, label: t(`sync.category.${value}`) })),
);

async function save(next: SyncAutoSettings) {
  const previous = settings.value;
  settings.value = next;
  saving.value = true;
  try {
    settings.value = await syncSetAuto(next);
  } catch (err) {
    settings.value = previous;
    message.error(formatError(toAppError(err)));
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="card auto">
    <div class="head">
      <div>
        <div class="title">{{ t("sync.auto.title") }}</div>
        <div class="hint">{{ t("sync.auto.hint") }}</div>
      </div>
      <NSwitch
        :value="settings.enabled"
        :loading="saving"
        @update:value="(enabled: boolean) => save({ ...settings, enabled })"
      />
    </div>
    <NCheckboxGroup
      v-if="settings.enabled"
      :value="settings.categories"
      class="checks"
      @update:value="
        (v: (string | number)[]) => save({ ...settings, categories: v as SyncCategory[] })
      "
    >
      <NCheckbox v-for="c in categoryOptions" :key="c.value" :value="c.value">
        {{ c.label }}
      </NCheckbox>
    </NCheckboxGroup>
    <p v-if="settings.enabled && !hasMaster" class="warn">{{ t("sync.auto.noMaster") }}</p>
  </div>
</template>

<style scoped>
.card {
  padding: 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.title {
  font-weight: 600;
}

.hint {
  margin-top: 2px;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.checks {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 20px;
  margin-top: 12px;
}

.warn {
  margin: 8px 0 0;
  font-size: 12px;
  color: #9d5d00;
}
</style>
