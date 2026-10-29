<script setup lang="ts">
// 一个选项的值控件：按 controls.ts 的分类显示开关、下拉、滑块加数值、按键录入或文本框。
import { NInput, NInputNumber, NSelect, NSlider, NSwitch } from "naive-ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { controlFor, fromDisplay, toDisplay } from "./controls";
import KeyCapture from "./KeyCapture.vue";
import { valueLabel } from "./options";

const props = defineProps<{ optionKey: string; value: string }>();
const emit = defineEmits<{ "update:value": [value: string] }>();

const { t, te } = useI18n();
const tr = { t, te };

const control = computed(() => controlFor(props.optionKey, props.value));
const selectOptions = computed(() => {
  const c = control.value;
  if (c.kind !== "select") return [];
  const values =
    c.values.includes(props.value) || !props.value ? c.values : [props.value, ...c.values];
  return values.map((v) => ({ value: v, label: valueLabel(props.optionKey, v, tr) ?? v }));
});
const display = computed(() => toDisplay(control.value, props.value));

function setNumber(n: number | null) {
  if (n !== null) emit("update:value", fromDisplay(control.value, n));
}
</script>

<template>
  <NSwitch
    v-if="control.kind === 'switch'"
    class="switch"
    :value="value === 'true'"
    @update:value="(on: boolean) => emit('update:value', String(on))"
  />
  <NSelect
    v-else-if="control.kind === 'select'"
    size="small"
    :value="value"
    :options="selectOptions"
    :filterable="optionKey === 'lang'"
    :tag="optionKey === 'lang'"
    @update:value="(v: string) => emit('update:value', v)"
  />
  <div v-else-if="control.kind === 'number'" class="number">
    <NSlider
      class="slider"
      :value="display ?? control.min"
      :min="control.min"
      :max="control.max"
      :step="control.step"
      :tooltip="false"
      @update:value="setNumber"
    />
    <NInputNumber
      class="input"
      size="small"
      :value="display"
      :min="control.min"
      :max="control.max"
      :step="control.step"
      :show-button="false"
      @update:value="setNumber"
    >
      <template v-if="control.unit" #suffix>{{ t(`sync.unit.${control.unit}`) }}</template>
    </NInputNumber>
  </div>
  <KeyCapture
    v-else-if="control.kind === 'key'"
    :value="value"
    @update:value="(v: string) => emit('update:value', v)"
  />
  <NInput
    v-else
    size="small"
    :value="value"
    :placeholder="t('sync.editor.value')"
    @update:value="(v: string) => emit('update:value', v)"
  />
</template>

<style scoped>
.switch {
  justify-self: start;
}

.number {
  display: flex;
  align-items: center;
  gap: 10px;
}

.slider {
  flex: 1;
}

.input {
  width: 96px;
}
</style>
