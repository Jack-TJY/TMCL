<script setup lang="ts">
// 按键录入：点一下后按下键盘按键或鼠标按键完成绑定，Esc 表示未绑定（与游戏一致）。
import { NButton } from "naive-ui";
import { onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { keyFromEvent, keyFromMouse } from "./controls";
import { keyLabel } from "./options";

const props = defineProps<{ value: string }>();
const emit = defineEmits<{ "update:value": [value: string] }>();

const { t, te } = useI18n();
const tr = { t, te };
const listening = ref(false);

function stop() {
  listening.value = false;
  window.removeEventListener("keydown", onKey, true);
  window.removeEventListener("mousedown", onMouse, true);
}

function onKey(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();
  const key = keyFromEvent(e.code);
  if (key) emit("update:value", key);
  stop();
}

function onMouse(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  emit("update:value", keyFromMouse(e.button));
  stop();
}

function start() {
  if (listening.value) return;
  listening.value = true;
  // 等本次点击结束后再监听，避免把这次左键当成绑定。
  setTimeout(() => {
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("mousedown", onMouse, true);
  });
}

onBeforeUnmount(stop);
</script>

<template>
  <NButton
    size="small"
    class="key-capture"
    :type="listening ? 'primary' : 'default'"
    :title="props.value"
    @click="start"
    @contextmenu.prevent
  >
    {{ listening ? t("sync.editor.pressKey") : (keyLabel(props.value, tr) ?? props.value) }}
  </NButton>
</template>

<style scoped>
.key-capture {
  width: 100%;
  justify-content: center;
}
</style>
