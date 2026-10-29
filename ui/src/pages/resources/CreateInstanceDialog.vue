<script setup lang="ts">
// 为资源一键新建实例：游戏版本默认取资源支持的最新正式版，加载器默认取资源需要的那个
// （Iris 光影用 Fabric），创建后下载游戏文件与加载器；任务成功后由页面继续安装资源。
import { NButton, NForm, NFormItem, NInput, NModal, NSelect, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { gameInstall, gameVersionList, GAME_INSTALL_KIND } from "../../api/game";
import { instanceCreate, instanceList } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import { loaderInstall, LOADER_INSTALL_KIND } from "../../api/loader";
import type { ResourceHit, ResourceKind } from "../../api/resources";
import type { InstanceConfig } from "../../bindings/InstanceConfig";
import type { LoaderSpec } from "../../bindings/LoaderSpec";
import type { TaskId } from "../../bindings/TaskId";
import LoaderPicker from "../../components/loader-picker/LoaderPicker.vue";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import { defaultInstanceName } from "../download/versions";
import { releaseVersions, suggestedLoader } from "./targets";

const props = defineProps<{ hit: ResourceHit | null; kind: ResourceKind }>();
const emit = defineEmits<{ close: []; started: [taskId: TaskId, instance: InstanceConfig] }>();

const { t } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const name = ref("");
const versions = ref<string[]>([]);
const gameVersion = ref<string | null>(null);
const loader = ref<LoaderSpec | null>(null);
const busy = ref(false);

watch(
  () => props.hit,
  async (hit) => {
    if (!hit) return;
    name.value = hit.title;
    versions.value = releaseVersions(hit.game_versions);
    gameVersion.value = versions.value[0] ?? null;
    // 版本号留空，加载器列表读到后 LoaderPicker 会换成推荐版本，不支持时退回原版。
    const kind = suggestedLoader(hit, props.kind);
    loader.value = kind ? { kind, version: "" } : null;
    const existing = await instanceList()
      .then(({ instances }) => instances.map((i) => i.name))
      .catch(() => []);
    if (props.hit === hit && name.value === hit.title) {
      name.value = defaultInstanceName(hit.title, existing);
    }
    // 搜索结果没有版本信息时，从版本清单中选正式版。
    if (versions.value.length === 0) {
      const list = await gameVersionList().catch(() => null);
      if (props.hit !== hit || !list) return;
      versions.value = list.versions.filter((v) => v.kind === "release").map((v) => v.id);
      gameVersion.value = versions.value[0] ?? null;
    }
  },
  { immediate: true },
);

const versionOptions = computed(() => versions.value.map((v) => ({ label: v, value: v })));
/** Mod 必须装在有加载器的实例上。 */
const needsLoader = computed(
  () => props.kind === "mod" && (!loader.value || !loader.value.version),
);
const ready = computed(() => !!name.value.trim() && !!gameVersion.value && !needsLoader.value);

async function confirm() {
  const version = gameVersion.value;
  const instanceName = name.value.trim();
  if (!version || !ready.value || busy.value) return;
  busy.value = true;
  try {
    const instance = await instanceCreate({ name: instanceName, game_version: version });
    const spec = loader.value?.version ? loader.value : null;
    const taskId = await tasks.start(
      spec
        ? {
            kind: LOADER_INSTALL_KIND,
            label: instanceName,
            start: () => loaderInstall({ instance_id: instance.id, ...spec }),
            instanceId: instance.id,
          }
        : {
            kind: GAME_INSTALL_KIND,
            label: instanceName,
            start: () => gameInstall({ version_id: version }),
            instanceId: instance.id,
          },
    );
    message.success(t("resources.create.started", { name: instanceName }));
    emit("started", taskId, instance);
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
    :show="hit !== null"
    preset="card"
    style="width: 560px"
    :title="t('resources.create.title', { name: hit?.title ?? '' })"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(show: boolean) => !show && emit('close')"
  >
    <p class="intro">{{ t("resources.create.intro") }}</p>
    <NForm @submit.prevent="confirm">
      <NFormItem :label="t('resources.create.name')">
        <NInput v-model:value="name" :maxlength="64" />
      </NFormItem>
      <NFormItem :label="t('resources.create.gameVersion')">
        <NSelect v-model:value="gameVersion" :options="versionOptions" filterable />
      </NFormItem>
      <NFormItem
        :label="t('resources.create.loader')"
        :feedback="needsLoader ? t('resources.create.needsLoader') : undefined"
        :validation-status="needsLoader ? 'warning' : undefined"
      >
        <LoaderPicker v-model="loader" :game-version="gameVersion" :disabled="busy" />
      </NFormItem>
    </NForm>
    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{
          t("resources.install.cancel")
        }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="!ready" @click="confirm">
          {{ t("resources.create.confirm") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.intro {
  margin: 0 0 12px;
  font-size: 13px;
  color: var(--tmcl-text-secondary);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
