<script setup lang="ts">
// 组件设置对话框：按组件的 settingsSchema 生成表单，“确定”后写回编辑中的布局。
import { NButton, NForm, NFormItem, NInputNumber, NModal, NSelect, NSwitch } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { instanceList } from "../../api/instance";
import type { HomeItem } from "../../stores/layout";
import { findWidget } from "../../widgets/registry";
import type { WidgetSettings } from "../../widgets/types";

const props = defineProps<{ item: HomeItem | null }>();
const emit = defineEmits<{ close: []; save: [settings: WidgetSettings] }>();
const { t } = useI18n();

const def = computed(() => (props.item ? findWidget(props.item.type) : undefined));
const draft = ref<WidgetSettings>({});
/** 实例选择器的选项；`""` 代表“最近游玩的实例”（NSelect 不接受 null 作为值）。 */
const instanceOptions = ref<{ label: string; value: string }[]>([]);

watch(
  () => props.item,
  async (item) => {
    draft.value = { ...item?.settings };
    if (!def.value?.settingsSchema?.some((f) => f.type === "instance")) return;
    const latest = { label: t("home.settings.latestInstance"), value: "" };
    instanceOptions.value = [latest];
    try {
      const { instances } = await instanceList();
      instanceOptions.value = [
        latest,
        ...instances.filter((i) => !i.error).map((i) => ({ label: i.name, value: i.id })),
      ];
    } catch {
      // 读取失败时只提供“最近游玩的实例”。
    }
  },
);

function instanceValue(key: string): string {
  return (draft.value[key] as string | null) ?? "";
}

function save() {
  emit("save", draft.value);
  emit("close");
}
</script>

<template>
  <NModal
    :show="item !== null"
    preset="card"
    :title="def ? t('home.settings.title', { name: t(def.titleKey) }) : ''"
    :style="{ width: '440px' }"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <NForm v-if="def" label-placement="left" label-width="auto">
      <NFormItem v-for="field in def.settingsSchema" :key="field.key" :label="t(field.labelKey)">
        <NSwitch
          v-if="field.type === 'boolean'"
          :value="draft[field.key] as boolean"
          @update:value="(v: boolean) => (draft[field.key] = v)"
        />
        <NInputNumber
          v-else-if="field.type === 'number'"
          :value="draft[field.key] as number"
          :min="field.min"
          :max="field.max"
          :step="field.step ?? 1"
          @update:value="(v: number | null) => (draft[field.key] = v ?? field.default)"
        />
        <NSelect
          v-else-if="field.type === 'select'"
          :value="draft[field.key] as string"
          :options="field.options.map((o) => ({ label: t(o.labelKey), value: o.value }))"
          @update:value="(v: string) => (draft[field.key] = v)"
        />
        <NSelect
          v-else
          :value="instanceValue(field.key)"
          :options="instanceOptions"
          @update:value="(v: string) => (draft[field.key] = v || null)"
        />
      </NFormItem>
    </NForm>
    <template #action>
      <div class="actions">
        <NButton @click="emit('close')">{{ t("home.cancel") }}</NButton>
        <NButton type="primary" @click="save">{{ t("home.settings.save") }}</NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
