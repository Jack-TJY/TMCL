<script setup lang="ts">
// 新建实例并下载游戏文件：填写实例名、选择加载器（默认原版）后一键创建，下载任务进入任务中心。
// 选了加载器时以 loader_install 任务补全原版并安装加载器，成功后实例才会带上加载器。
import { NButton, NForm, NFormItem, NInput, NModal, useMessage } from "naive-ui";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { gameInstall, GAME_INSTALL_KIND } from "../../api/game";
import { instanceCreate, instanceList } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import { loaderInstall, LOADER_INSTALL_KIND } from "../../api/loader";
import type { GameVersion } from "../../bindings/GameVersion";
import type { LoaderSpec } from "../../bindings/LoaderSpec";
import LoaderPicker from "../../components/loader-picker/LoaderPicker.vue";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import { defaultInstanceName } from "./versions";

const props = defineProps<{ version: GameVersion | null }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const name = ref("");
const loader = ref<LoaderSpec | null>(null);
const busy = ref(false);

watch(
  () => props.version,
  async (version) => {
    if (!version) return;
    name.value = version.id;
    loader.value = null;
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
    const instance = await instanceCreate({ name: instanceName, game_version: version.id });
    const spec = loader.value;
    await tasks.start(
      spec
        ? {
            kind: LOADER_INSTALL_KIND,
            label: instanceName,
            start: () => loaderInstall({ instance_id: instance.id, ...spec }),
          }
        : {
            kind: GAME_INSTALL_KIND,
            label: instanceName,
            start: () => gameInstall({ version_id: version.id }),
          },
    );
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
    style="width: 560px"
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
        <LoaderPicker v-model="loader" :game-version="version?.id ?? null" :disabled="busy" />
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
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
