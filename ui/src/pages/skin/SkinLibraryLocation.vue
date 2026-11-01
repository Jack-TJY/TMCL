<script setup lang="ts">
// 皮肤库位置：显示当前文件夹，可打开、更改（可选复制现有皮肤）或恢复默认。
// 放到网盘同步文件夹或另一个 TMCL 的皮肤库，即可跨电脑、跨安装目录共用。
import { Folder20Regular } from "@vicons/fluent";
import { NButton, NIcon, NPopover, useDialog, useMessage } from "naive-ui";
import { h, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { pickFolder } from "../../api/dialog";
import { toAppError } from "../../api/invoke";
import {
  skinLibraryLocation,
  skinLibraryOpenDir,
  skinLibrarySetLocation,
  type SkinLibraryLocation,
} from "../../api/skin";
import { formatError } from "../../i18n";

const emit = defineEmits<{ changed: [] }>();

const { t } = useI18n();
const message = useMessage();
const dialog = useDialog();

const location = ref<SkinLibraryLocation | null>(null);
const busy = ref(false);

function showError(err: unknown) {
  message.error(formatError(toAppError(err)));
}

async function load() {
  try {
    location.value = await skinLibraryLocation();
  } catch (err) {
    showError(err);
  }
}

async function apply(dir: string | undefined, copyExisting: boolean) {
  busy.value = true;
  try {
    location.value = await skinLibrarySetLocation({ dir, copy_existing: copyExisting });
    message.success(t("skin.location.changed"));
    emit("changed");
  } catch (err) {
    showError(err);
  } finally {
    busy.value = false;
  }
}

/** 询问是否复制现有皮肤；关闭对话框视为取消。 */
function askCopy(dir: string | undefined, shown: string) {
  dialog.info({
    title: t("skin.location.copyTitle"),
    content: () =>
      h("div", { style: "white-space: pre-line" }, t("skin.location.copyContent", { dir: shown })),
    positiveText: t("skin.location.copy"),
    negativeText: t("skin.location.noCopy"),
    onPositiveClick: () => apply(dir, true),
    onNegativeClick: () => apply(dir, false),
  });
}

async function change() {
  const dir = await pickFolder().catch((err) => {
    showError(err);
    return null;
  });
  if (!dir || dir === location.value?.dir) return;
  askCopy(dir, dir);
}

function reset() {
  askCopy(undefined, t("skin.location.default"));
}

function open() {
  skinLibraryOpenDir().catch(showError);
}

onMounted(load);
</script>

<template>
  <NPopover trigger="click" placement="bottom-end" :width="320">
    <template #trigger>
      <NButton size="small" :loading="busy" :title="location?.dir">
        <template #icon
          ><NIcon><Folder20Regular /></NIcon
        ></template>
        {{ t("skin.location.title") }}
      </NButton>
    </template>
    <div class="location">
      <div class="kind">
        {{ location?.custom ? t("skin.location.custom") : t("skin.location.default") }}
      </div>
      <div class="dir">{{ location?.dir }}</div>
      <p class="tip">{{ t("skin.location.tip") }}</p>
      <div class="actions">
        <NButton size="small" @click="open">{{ t("skin.location.open") }}</NButton>
        <NButton size="small" type="primary" :disabled="busy" @click="change">
          {{ t("skin.location.change") }}
        </NButton>
        <NButton v-if="location?.custom" size="small" :disabled="busy" @click="reset">
          {{ t("skin.location.reset") }}
        </NButton>
      </div>
    </div>
  </NPopover>
</template>

<style scoped>
.location {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.kind {
  font-weight: 600;
}

.dir {
  font-family: var(--tmcl-font-mono, monospace);
  font-size: 12px;
  word-break: break-all;
}

.tip {
  margin: 0;
  color: var(--tmcl-text-secondary);
  font-size: 12px;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
}
</style>
