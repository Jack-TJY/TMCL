<script setup lang="ts">
// 新建实例并下载游戏文件：填写实例名后一键创建，下载任务进入任务中心。
// 加载器选择由 M1-15 的组件提供，在此之前只能创建原版实例。
import { NButton, NForm, NFormItem, NInput, NModal, useMessage } from "naive-ui";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { gameInstall, GAME_INSTALL_KIND } from "../../api/game";
import { instanceCreate, instanceList } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import type { GameVersion } from "../../bindings/GameVersion";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import { defaultInstanceName } from "./versions";

const props = defineProps<{ version: GameVersion | null }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const name = ref("");
const busy = ref(false);

watch(
  () => props.version,
  async (version) => {
    if (!version) return;
    name.value = version.id;
    const existing = await instanceList()
      .then(({ instances }) => instances.map((i) => i.name))
      .catch(() => []);
    // 用户可能已经开始输入，只在名称未改动时替换。
    if (props.version === version && name.value === version.id) {
      name.value = defaultInstanceName(version.id, existing);
    }
  },
  { immediate: true },
);

async function confirm() {
  const version = props.version;
  const instanceName = name.value.trim();
  if (!version || !instanceName || busy.value) return;
  busy.value = true;
  try {
    await instanceCreate({ name: instanceName, game_version: version.id });
    await tasks.start({
      kind: GAME_INSTALL_KIND,
      label: instanceName,
      start: () => gameInstall({ version_id: version.id }),
    });
    message.success(t("download.install.started", { name: instanceName }));
    emit("close");
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <NModal
    :show="version !== null"
    preset="card"
    style="width: 440px"
    :title="t('download.install.title', { version: version?.id ?? '' })"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <NForm @submit.prevent="confirm">
      <NFormItem :label="t('download.install.name')">
        <NInput
          v-model:value="name"
          :maxlength="64"
          :placeholder="version?.id"
          autofocus
          @keyup.enter="confirm"
        />
      </NFormItem>
      <NFormItem :label="t('download.install.loader')">
        <span class="loader-hint">{{ t("download.install.loaderVanilla") }}</span>
      </NFormItem>
    </NForm>
    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{
          t("download.install.cancel")
        }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="!name.trim()" @click="confirm">
          {{ t("download.install.confirm") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.loader-hint {
  font-size: 13px;
  color: var(--tmcl-text-secondary);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
