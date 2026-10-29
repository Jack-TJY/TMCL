<script setup lang="ts">
// 安装整合包为新实例（PLAN.md M2-04、M2-06）：在 Modrinth 或 CurseForge（构建带 Key 时才显示）
// 搜索并选版本，或选择本地 .mrpack / CurseForge zip；填写实例名后以 resources_modpack_install 任务安装。
// CurseForge 整合包中作者禁止第三方下载的文件会列出来，用户可打开网页手动下载后放进对应目录。
import { Search20Regular } from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NDescriptions,
  NDescriptionsItem,
  NEmpty,
  NIcon,
  NInput,
  NList,
  NListItem,
  NModal,
  NSelect,
  NSpin,
  NTab,
  NTabs,
  useMessage,
} from "naive-ui";
import { computed, h, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { pickFile } from "../../api/dialog";
import { toAppError } from "../../api/invoke";
import {
  resourcesModpackInspect,
  resourcesModpackInstall,
  resourcesModpackSearch,
  resourcesModpackVersions,
  resourcesOpenPage,
  resourcesSources,
  RESOURCES_MODPACK_INSTALL_KIND,
  type ManualDownload,
  type ModpackInfo,
  type ModpackSource,
  type ResourceHit,
  type ResourceVersion,
} from "../../api/resources";
import type { AppError } from "../../bindings/AppError";
import type { TaskId } from "../../bindings/TaskId";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";
import ChannelTag from "./ChannelTag.vue";
import ResourceIcon from "./ResourceIcon.vue";
import { defaultVersion, formatCount, formatSize } from "./resources";

type Platform = "modrinth" | "curseforge";
type Mode = Platform | "file";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ close: []; started: [taskId: TaskId] }>();

const { t, locale } = useI18n();
const message = useMessage();
const tasks = useTasksStore();

const mode = ref<Mode>("modrinth");
const name = ref("");
const busy = ref(false);
const curseforge = ref(false);
resourcesSources()
  .then((sources) => (curseforge.value = sources.curseforge))
  .catch(() => (curseforge.value = false));

// Modrinth / CurseForge 搜索
const query = ref("");
const hits = ref<ResourceHit[]>([]);
const searching = ref(false);
const searchError = ref<AppError | null>(null);
const selected = ref<ResourceHit | null>(null);
const versions = ref<ResourceVersion[]>([]);
const versionId = ref<string | null>(null);
const loadingVersions = ref(false);

// 本地文件
const filePath = ref<string | null>(null);
const info = ref<ModpackInfo | null>(null);
const inspecting = ref(false);
const inspectError = ref<AppError | null>(null);

const platform = computed<Platform>(() =>
  mode.value === "curseforge" ? "curseforge" : "modrinth",
);

let searchSeq = 0;
async function search() {
  const id = ++searchSeq;
  searching.value = true;
  searchError.value = null;
  try {
    const page = await resourcesModpackSearch({
      platform: platform.value,
      query: query.value.trim(),
      limit: 20,
    });
    if (id === searchSeq) hits.value = page.hits;
  } catch (err) {
    if (id === searchSeq) searchError.value = toAppError(err);
  } finally {
    if (id === searchSeq) searching.value = false;
  }
}

let timer: ReturnType<typeof setTimeout> | undefined;
watch(query, () => {
  clearTimeout(timer);
  timer = setTimeout(search, 350);
});

watch(mode, (next, prev) => {
  if (next === "file" || next === prev) return;
  selected.value = null;
  hits.value = [];
  void search();
});

watch(
  () => props.show,
  (show) => {
    if (!show) return;
    mode.value = "modrinth";
    name.value = "";
    selected.value = null;
    filePath.value = null;
    info.value = null;
    inspectError.value = null;
    if (hits.value.length === 0) void search();
  },
);

async function select(hit: ResourceHit) {
  selected.value = hit;
  name.value = hit.title;
  versions.value = [];
  versionId.value = null;
  loadingVersions.value = true;
  try {
    const list = await resourcesModpackVersions({
      platform: platform.value,
      project_id: hit.project_id,
    });
    if (selected.value !== hit) return;
    versions.value = list;
    versionId.value = defaultVersion(list)?.id ?? null;
  } catch (err) {
    if (selected.value === hit) message.error(formatError(toAppError(err)));
  } finally {
    if (selected.value === hit) loadingVersions.value = false;
  }
}

const searchKey = computed(() =>
  mode.value === "curseforge" ? "resources.modpack.searchCurseforge" : "resources.modpack.search",
);
const creditKey = computed(() =>
  mode.value === "curseforge" ? "resources.creditCurseforge" : "resources.credit",
);

const version = computed(() => versions.value.find((v) => v.id === versionId.value) ?? null);
const versionOptions = computed(() =>
  versions.value.map((v) => ({ label: v.version_number, value: v.id, version: v })),
);
function renderVersionLabel(option: { label: string; version: ResourceVersion }) {
  return [option.label, " ", h(ChannelTag, { channel: option.version.channel })];
}

async function chooseFile() {
  const filters = [{ name: "Modrinth", extensions: ["mrpack"] }];
  if (curseforge.value) filters.push({ name: "CurseForge", extensions: ["zip"] });
  const path = await pickFile(filters).catch(() => null);
  if (!path) return;
  filePath.value = path;
  info.value = null;
  inspectError.value = null;
  inspecting.value = true;
  try {
    const result = await resourcesModpackInspect({ path });
    if (filePath.value !== path) return;
    info.value = result;
    name.value = result.name;
  } catch (err) {
    if (filePath.value === path) inspectError.value = toAppError(err);
  } finally {
    if (filePath.value === path) inspecting.value = false;
  }
}

const loaderText = computed(() => {
  const loader = info.value?.loader;
  return loader
    ? `${t(`instances.loader.${loader.kind}`)} ${loader.version}`
    : t("instances.loader.vanilla");
});

const source = computed<ModpackSource | null>(() => {
  if (mode.value === "modrinth") {
    return versionId.value ? { kind: "modrinth", version_id: versionId.value } : null;
  }
  if (mode.value === "curseforge") {
    return versionId.value && selected.value
      ? {
          kind: "curseforge",
          project_id: Number(selected.value.project_id),
          file_id: Number(versionId.value),
        }
      : null;
  }
  return filePath.value && info.value ? { kind: "file", path: filePath.value } : null;
});

const manual = computed<ManualDownload[]>(() => info.value?.manual_downloads ?? []);

async function openPage(item: ManualDownload) {
  try {
    await resourcesOpenPage({ url: item.page_url });
  } catch (err) {
    message.error(formatError(toAppError(err)));
  }
}

async function confirm() {
  const chosen = source.value;
  if (!chosen || busy.value) return;
  const instanceName = name.value.trim();
  busy.value = true;
  try {
    const label = instanceName || selected.value?.title || info.value?.name || null;
    const taskId = await tasks.start({
      kind: RESOURCES_MODPACK_INSTALL_KIND,
      label,
      start: () => resourcesModpackInstall({ source: chosen, name: instanceName || undefined }),
    });
    message.success(t("resources.modpack.started", { name: label ?? "" }));
    emit("started", taskId);
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
    :show="show"
    preset="card"
    style="width: 640px"
    :title="t('resources.modpack.title')"
    :mask-closable="!busy"
    :closable="!busy"
    @update:show="(value: boolean) => !value && emit('close')"
  >
    <NTabs v-model:value="mode" type="segment" size="small" class="modes">
      <NTab name="modrinth">Modrinth</NTab>
      <NTab v-if="curseforge" name="curseforge">CurseForge</NTab>
      <NTab name="file">{{ t("resources.modpack.file") }}</NTab>
    </NTabs>

    <template v-if="mode !== 'file'">
      <template v-if="!selected">
        <NInput v-model:value="query" clearable :placeholder="t(searchKey)">
          <template #prefix
            ><NIcon><Search20Regular /></NIcon
          ></template>
        </NInput>
        <NAlert v-if="searchError" type="error" class="block">
          {{ formatError(searchError) }}
        </NAlert>
        <NSpin :show="searching" class="hits">
          <NEmpty
            v-if="!searching && !searchError && hits.length === 0"
            :description="t('resources.browse.empty')"
          />
          <button
            v-for="hit in hits"
            :key="hit.project_id"
            type="button"
            class="hit"
            @click="select(hit)"
          >
            <ResourceIcon :url="hit.icon_url" :size="40" />
            <span class="hit-body">
              <span class="hit-title">{{ hit.title }}</span>
              <span class="hit-desc">{{ hit.description }}</span>
              <span class="hit-meta">
                {{ t("resources.browse.downloads", { count: formatCount(hit.downloads, locale) }) }}
              </span>
            </span>
          </button>
        </NSpin>
        <div class="credit">{{ t(creditKey) }}</div>
      </template>
      <template v-else>
        <div class="chosen">
          <ResourceIcon :url="selected.icon_url" :size="40" />
          <div class="hit-body">
            <span class="hit-title">{{ selected.title }}</span>
            <span class="hit-desc">{{ selected.description }}</span>
          </div>
          <NButton size="small" :disabled="busy" @click="selected = null">
            {{ t("resources.modpack.change") }}
          </NButton>
        </div>
        <div class="field">
          <span class="field-label">{{ t("resources.install.version") }}</span>
          <NSelect
            v-model:value="versionId"
            :loading="loadingVersions"
            :options="versionOptions"
            :render-label="renderVersionLabel"
            :disabled="busy"
            filterable
          />
        </div>
        <div v-if="version" class="hint">
          {{
            t("resources.modpack.supports", {
              versions: version.game_versions.join(", "),
              loaders: version.loaders.join(", "),
            })
          }}
        </div>
        <div v-if="mode === 'curseforge'" class="hint">{{ t("resources.modpack.manualHint") }}</div>
      </template>
    </template>

    <template v-else>
      <div class="field">
        <NButton :disabled="busy" @click="chooseFile">{{ t("resources.modpack.choose") }}</NButton>
        <span class="path" :title="filePath ?? ''">{{
          filePath ?? t("resources.modpack.none")
        }}</span>
      </div>
      <NSpin :show="inspecting">
        <NAlert v-if="inspectError" type="error" :title="t('resources.modpack.invalid')">
          {{ formatError(inspectError) }}
        </NAlert>
        <NDescriptions v-if="info" :column="2" size="small" label-placement="left" bordered>
          <NDescriptionsItem :label="t('resources.modpack.name')">
            {{ info.name }} {{ info.version }}
          </NDescriptionsItem>
          <NDescriptionsItem :label="t('instances.detail.version')">
            {{ info.game_version }}
          </NDescriptionsItem>
          <NDescriptionsItem :label="t('instances.detail.loader')">
            {{ loaderText }}
          </NDescriptionsItem>
          <NDescriptionsItem :label="t('resources.modpack.download')">
            {{
              t("resources.install.files", {
                count: info.file_count,
                size: formatSize(info.download_size),
              })
            }}
          </NDescriptionsItem>
        </NDescriptions>
        <div v-if="info?.summary" class="hint">{{ info.summary }}</div>
        <NAlert
          v-if="manual.length > 0"
          type="warning"
          class="block"
          :title="t('resources.modpack.manualTitle', { count: manual.length })"
        >
          {{ t("resources.modpack.manualBody") }}
          <NList size="small" class="manual">
            <NListItem v-for="item in manual" :key="item.page_url">
              <span class="manual-name">{{ item.project_name }}</span>
              <span class="manual-file">{{ item.file_name }} → {{ item.target_dir }}/</span>
              <template #suffix>
                <NButton size="tiny" @click="openPage(item)">
                  {{ t("resources.modpack.openPage") }}
                </NButton>
              </template>
            </NListItem>
          </NList>
        </NAlert>
      </NSpin>
    </template>

    <div v-if="source" class="field name">
      <span class="field-label">{{ t("instances.name") }}</span>
      <NInput v-model:value="name" :maxlength="64" :disabled="busy" />
    </div>

    <template #footer>
      <div class="dialog-actions">
        <NButton :disabled="busy" @click="emit('close')">{{
          t("resources.install.cancel")
        }}</NButton>
        <NButton type="primary" :loading="busy" :disabled="!source" @click="confirm">
          {{ t("resources.install.confirm") }}
        </NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.modes {
  margin-bottom: 12px;
}

.block {
  margin-top: 8px;
}

.hits {
  height: 320px;
  margin-top: 8px;
  overflow-y: auto;
}

.hit,
.chosen {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 8px;
  color: inherit;
  font: inherit;
  text-align: left;
  background: none;
  border: none;
  border-radius: 8px;
}

.hit {
  cursor: pointer;
}

.hit:hover {
  background: var(--tmcl-subtle);
}

.chosen {
  margin-bottom: 12px;
  background: var(--tmcl-subtle);
}

.hit-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.hit-title {
  font-weight: 600;
}

.hit-desc {
  overflow: hidden;
  font-size: 13px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.hit-meta,
.hint,
.credit {
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.credit {
  margin-top: 4px;
}

.hint {
  margin-top: 8px;
}

.field {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.field.name {
  margin: 16px 0 0;
}

.field-label {
  flex-shrink: 0;
  font-size: 13px;
}

.manual {
  max-height: 160px;
  margin-top: 8px;
  overflow-y: auto;
  background: transparent;
}

.manual-name {
  margin-right: 8px;
  font-weight: 600;
}

.manual-file {
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.path {
  overflow: hidden;
  font-size: 13px;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--tmcl-text-secondary);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
