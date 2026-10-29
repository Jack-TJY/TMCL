<script setup lang="ts">
// 设置同步页（PLAN.md M2-10）：主配置从实例采集或手动编辑，勾选类别与目标实例，
// 预览差异后执行；每个实例可撤销最近一次同步（恢复备份）。
import {
  ArrowDownload20Regular,
  ArrowSync24Regular,
  ArrowUndo20Regular,
  Edit20Regular,
  Eye20Regular,
} from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NCheckbox,
  NCheckboxGroup,
  NEmpty,
  NIcon,
  NSelect,
  NSpin,
  useDialog,
  useMessage,
} from "naive-ui";
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { instanceList } from "../../api/instance";
import { toAppError } from "../../api/invoke";
import { syncApply, syncCapture, syncGetMaster, syncPreview, syncRestore } from "../../api/sync";
import type { AppError } from "../../bindings/AppError";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import type { SyncCategory } from "../../bindings/SyncCategory";
import type { SyncPreview } from "../../bindings/SyncPreview";
import type { SyncProfile } from "../../bindings/SyncProfile";
import { formatError } from "../../i18n";
import MasterEditor from "./MasterEditor.vue";
import PreviewList from "./PreviewList.vue";
import { DEFAULT_CATEGORIES, summarizePreview, SYNC_CATEGORIES, usableInstances } from "./sync";

const { t, locale } = useI18n();
const message = useMessage();
const dialog = useDialog();

const instances = ref<InstanceSummary[]>([]);
const master = ref<SyncProfile | null>(null);
const loading = ref(false);
const loadError = ref<AppError | null>(null);

const source = ref<string | null>(null);
const categories = ref<SyncCategory[]>([...DEFAULT_CATEGORIES]);
const targets = ref<string[]>([]);
const preview = ref<SyncPreview | null>(null);
const editing = ref(false);
const busy = ref<"capture" | "preview" | "apply" | null>(null);
const restoring = ref<string | null>(null);

async function load() {
  loading.value = true;
  loadError.value = null;
  try {
    const [list, current] = await Promise.all([instanceList(), syncGetMaster()]);
    instances.value = usableInstances(list.instances);
    master.value = current.profile;
    const ids = new Set(instances.value.map((i) => i.id));
    targets.value = targets.value.filter((id) => ids.has(id));
    if (source.value && !ids.has(source.value)) source.value = null;
    source.value ??= master.value?.source_instance ?? instances.value[0]?.id ?? null;
  } catch (err) {
    loadError.value = toAppError(err);
  } finally {
    loading.value = false;
  }
}

onMounted(load);

// 任一输入变化后旧预览不再可信。
watch([master, categories, targets], () => (preview.value = null), { deep: true });

const names = computed(() => Object.fromEntries(instances.value.map((i) => [i.id, i.name])));
const instanceOptions = computed(() =>
  instances.value.map((i) => ({ label: `${i.name} (${i.game_version})`, value: i.id })),
);
const categoryOptions = computed(() =>
  SYNC_CATEGORIES.map((value) => ({ value, label: t(`sync.category.${value}`) })),
);
const masterSource = computed(() => {
  const profile = master.value;
  if (!profile) return "";
  if (!profile.source_instance) return t("sync.master.manual");
  const name = names.value[profile.source_instance] ?? profile.source_instance;
  return profile.source_version ? `${name} (${profile.source_version})` : name;
});
const masterUpdated = computed(() =>
  master.value ? new Date(master.value.updated_at).toLocaleString(locale.value) : "",
);
const summary = computed(() => (preview.value ? summarizePreview(preview.value) : null));
const canPreview = computed(
  () => master.value !== null && categories.value.length > 0 && targets.value.length > 0,
);

function showError(err: unknown) {
  message.error(formatError(toAppError(err)));
}

async function capture() {
  const id = source.value;
  if (!id) return;
  busy.value = "capture";
  try {
    master.value = await syncCapture({ instance_id: id });
    message.success(t("sync.master.captured", { name: names.value[id] ?? id }));
  } catch (err) {
    showError(err);
  } finally {
    busy.value = null;
  }
}

function confirmCapture() {
  if (!master.value) return void capture();
  dialog.warning({
    title: t("sync.master.overwriteTitle"),
    content: t("sync.master.overwriteContent"),
    positiveText: t("sync.master.capture"),
    negativeText: t("sync.cancel"),
    onPositiveClick: () => void capture(),
  });
}

function selection() {
  return { instance_ids: [...targets.value], categories: [...categories.value] };
}

async function runPreview() {
  if (!canPreview.value) return;
  busy.value = "preview";
  try {
    preview.value = await syncPreview(selection());
  } catch (err) {
    showError(err);
  } finally {
    busy.value = null;
  }
}

async function apply() {
  busy.value = "apply";
  try {
    const result = await syncApply(selection());
    const changed = result.targets.filter((r) => r.changed > 0).length;
    message.success(t("sync.apply.done", { count: changed }));
    preview.value = await syncPreview(selection());
  } catch (err) {
    showError(err);
  } finally {
    busy.value = null;
  }
}

function confirmApply() {
  if (!summary.value?.changes) return;
  dialog.warning({
    title: t("sync.apply.title"),
    content: t("sync.apply.content", summary.value),
    positiveText: t("sync.apply.confirm"),
    negativeText: t("sync.cancel"),
    onPositiveClick: () => void apply(),
  });
}

async function restore(id: string) {
  restoring.value = id;
  try {
    const { restored } = await syncRestore({ instance_id: id });
    const name = names.value[id] ?? id;
    if (restored) message.success(t("sync.restore.done", { name }));
    else message.info(t("sync.restore.none", { name }));
    if (restored && preview.value) preview.value = await syncPreview(selection());
  } catch (err) {
    showError(err);
  } finally {
    restoring.value = null;
  }
}

function confirmRestore(instance: InstanceSummary) {
  dialog.warning({
    title: t("sync.restore.title", { name: instance.name }),
    content: t("sync.restore.content"),
    positiveText: t("sync.restore.confirm"),
    negativeText: t("sync.cancel"),
    onPositiveClick: () => void restore(instance.id),
  });
}

function selectAll(all: boolean) {
  targets.value = all ? instances.value.map((i) => i.id) : [];
}
</script>

<template>
  <div class="sync-page">
    <h1 class="page-title">{{ t("sync.title") }}</h1>

    <NAlert v-if="loadError" type="error" class="block" :title="t('sync.loadFailed')">
      <div class="alert-body">
        <span>{{ formatError(loadError) }}</span>
        <NButton size="small" @click="load">{{ t("sync.retry") }}</NButton>
      </div>
    </NAlert>

    <NSpin :show="loading" class="body">
      <section class="block">
        <h2 class="section-title">{{ t("sync.master.title") }}</h2>
        <div class="card">
          <div v-if="master" class="master-info">
            <div>
              <span class="label">{{ t("sync.master.source") }}</span
              >{{ masterSource }}
            </div>
            <div>
              <span class="label">{{ t("sync.master.options") }}</span
              >{{ master.options.length }}
            </div>
            <div>
              <span class="label">{{ t("sync.master.updated") }}</span
              >{{ masterUpdated }}
            </div>
          </div>
          <p v-else class="muted">{{ t("sync.master.none") }}</p>
          <div class="actions">
            <NSelect
              v-model:value="source"
              class="source"
              :options="instanceOptions"
              :placeholder="t('sync.master.pickSource')"
            />
            <NButton :disabled="!source" :loading="busy === 'capture'" @click="confirmCapture">
              <template #icon
                ><NIcon><ArrowDownload20Regular /></NIcon
              ></template>
              {{ t("sync.master.capture") }}
            </NButton>
            <NButton secondary @click="editing = true">
              <template #icon
                ><NIcon><Edit20Regular /></NIcon
              ></template>
              {{ t("sync.master.edit") }}
            </NButton>
          </div>
        </div>
      </section>

      <section class="block">
        <h2 class="section-title">{{ t("sync.categories") }}</h2>
        <div class="card">
          <NCheckboxGroup v-model:value="categories" class="checks">
            <NCheckbox v-for="c in categoryOptions" :key="c.value" :value="c.value">
              {{ c.label }}
            </NCheckbox>
          </NCheckboxGroup>
        </div>
      </section>

      <section class="block">
        <h2 class="section-title">
          {{ t("sync.targets.title") }}
          <span class="section-actions">
            <NButton text size="small" @click="selectAll(true)">{{
              t("sync.targets.all")
            }}</NButton>
            <NButton text size="small" @click="selectAll(false)">
              {{ t("sync.targets.none") }}
            </NButton>
          </span>
        </h2>
        <div class="card">
          <NEmpty v-if="instances.length === 0" :description="t('sync.targets.empty')" />
          <NCheckboxGroup v-else v-model:value="targets" class="target-list">
            <div v-for="instance in instances" :key="instance.id" class="target">
              <NCheckbox :value="instance.id">
                {{ instance.name }}
                <span class="muted">{{ instance.game_version }}</span>
                <span v-if="instance.id === master?.source_instance" class="muted">
                  · {{ t("sync.targets.isSource") }}
                </span>
              </NCheckbox>
              <NButton
                quaternary
                size="small"
                :loading="restoring === instance.id"
                @click="confirmRestore(instance)"
              >
                <template #icon
                  ><NIcon><ArrowUndo20Regular /></NIcon
                ></template>
                {{ t("sync.restore.action") }}
              </NButton>
            </div>
          </NCheckboxGroup>
        </div>
      </section>

      <section class="block">
        <div class="run">
          <NButton :disabled="!canPreview" :loading="busy === 'preview'" @click="runPreview">
            <template #icon
              ><NIcon><Eye20Regular /></NIcon
            ></template>
            {{ t("sync.preview.action") }}
          </NButton>
          <NButton
            type="primary"
            :disabled="!summary?.changes"
            :loading="busy === 'apply'"
            @click="confirmApply"
          >
            <template #icon
              ><NIcon><ArrowSync24Regular /></NIcon
            ></template>
            {{ t("sync.apply.action") }}
          </NButton>
          <span v-if="!master" class="muted">{{ t("sync.master.none") }}</span>
          <span v-else-if="summary" class="muted">
            {{ summary.changes ? t("sync.preview.summary", summary) : t("sync.preview.noChanges") }}
          </span>
        </div>
        <PreviewList v-if="preview" :preview="preview" :names="names" />
      </section>
    </NSpin>

    <MasterEditor
      :show="editing"
      :options="master?.options ?? []"
      @close="editing = false"
      @saved="(profile: SyncProfile) => (master = profile)"
    />
  </div>
</template>

<style scoped>
.sync-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 1000px;
}

.body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.block {
  margin-bottom: 20px;
}

.alert-body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
}

.section-actions {
  display: flex;
  gap: 12px;
  font-weight: 400;
}

.card {
  padding: 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
}

.master-info {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 24px;
  margin-bottom: 12px;
}

.label {
  margin-right: 8px;
  color: var(--tmcl-text-secondary);
}

.muted {
  color: var(--tmcl-text-secondary);
}

p.muted {
  margin: 0 0 12px;
}

.actions,
.run {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.run {
  margin-bottom: 12px;
}

.source {
  width: 280px;
}

.checks {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 20px;
}

.target-list {
  display: flex;
  flex-direction: column;
}

.target {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 36px;
}

.target + .target {
  border-top: 1px solid var(--tmcl-stroke);
}
</style>
