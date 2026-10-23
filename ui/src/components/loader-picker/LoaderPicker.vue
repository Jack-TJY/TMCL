<script setup lang="ts">
// 加载器选择组件（PLAN.md M1-15）：按游戏版本列出四种加载器及其版本，默认选中推荐版本，
// 不兼容的加载器灰显。`v-model` 为 `LoaderSpec`，`null` 表示原版。
import { NButton, NCheckbox, NSelect, NTag, type SelectOption } from "naive-ui";
import { computed, h, ref, toRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { LoaderKind } from "../../bindings/LoaderKind";
import type { LoaderSpec } from "../../bindings/LoaderSpec";
import { formatError } from "../../i18n";
import { useSettingsStore } from "../../stores/settings";
import { defaultVersion, LOADER_KINDS, MIRRORED_KINDS, visibleVersions } from "./loaders";
import { useLoaderVersions } from "./useLoaderVersions";

const props = defineProps<{ gameVersion: string | null; disabled?: boolean }>();
const model = defineModel<LoaderSpec | null>({ required: true });

const { t } = useI18n();
const settings = useSettingsStore();
const { states, retry } = useLoaderVersions(toRef(props, "gameVersion"));
const showUnstable = ref(false);

/** 加载器的项目名称是专有名词，不翻译。 */
const NAMES: Record<LoaderKind, string> = {
  fabric: "Fabric",
  quilt: "Quilt",
  forge: "Forge",
  neoforge: "NeoForge",
};

function status(kind: LoaderKind): string {
  const state = states[kind];
  switch (state.status) {
    case "loading":
      return t("loaderPicker.loading");
    case "unsupported":
      return t("loaderPicker.unsupported");
    case "error":
      return t("loaderPicker.failed");
    case "ready": {
      const recommended = defaultVersion(state.versions);
      return recommended ? recommended.version : "";
    }
  }
}

function select(kind: LoaderKind | null) {
  if (props.disabled) return;
  if (kind === null) {
    model.value = null;
    return;
  }
  const state = states[kind];
  if (state.status !== "ready") return;
  const version = defaultVersion(state.versions);
  if (version) model.value = { kind, version: version.version };
}

// 游戏版本改变或列表重新获取后，选中的加载器不再可用时退回原版，版本不存在时改为推荐版本。
watch(
  () => (model.value ? states[model.value.kind] : null),
  (state) => {
    const current = model.value;
    if (!current || !state || state.status === "loading") return;
    if (state.status !== "ready") {
      model.value = null;
    } else if (!state.versions.some((v) => v.version === current.version)) {
      select(current.kind);
    }
  },
);

const versionOptions = computed<SelectOption[]>(() => {
  const current = model.value;
  const state = current ? states[current.kind] : null;
  if (!current || state?.status !== "ready") return [];
  return visibleVersions(state.versions, showUnstable.value, current.version).map((v) => ({
    label: v.version,
    value: v.version,
    recommended: v.recommended,
    stable: v.stable,
  }));
});

const hasUnstable = computed(() => {
  const state = model.value ? states[model.value.kind] : null;
  return state?.status === "ready" && state.versions.some((v) => !v.stable);
});

function renderOption({ option }: { option: SelectOption }) {
  const tags = [];
  if (option.recommended) {
    tags.push(
      h(NTag, { size: "tiny", type: "success", bordered: false }, () =>
        t("loaderPicker.recommended"),
      ),
    );
  }
  if (!option.stable) {
    tags.push(
      h(NTag, { size: "tiny", type: "warning", bordered: false }, () => t("loaderPicker.unstable")),
    );
  }
  return h("div", { class: "version-option" }, [h("span", option.label as string), ...tags]);
}

const errorText = computed(() => {
  const current = model.value;
  return LOADER_KINDS.flatMap((kind) => {
    const state = states[kind];
    return state.status === "error" && current?.kind !== kind
      ? [{ kind, text: formatError(state.error) }]
      : [];
  });
});

/** 选中的加载器经 BMCLAPI 获取时标明来源（BMCLAPI 使用协议要求）。 */
const sourceNote = computed(() => {
  const kind = model.value?.kind;
  const source = settings.settings.download.source;
  if (!kind || source === "official" || !MIRRORED_KINDS.includes(kind)) return null;
  return t("loaderPicker.bmclapi", { name: NAMES[kind] });
});
</script>

<template>
  <div class="loader-picker">
    <div class="kinds" role="radiogroup" :aria-label="t('loaderPicker.label')">
      <button
        type="button"
        role="radio"
        class="kind"
        :class="{ active: model === null }"
        :aria-checked="model === null"
        :disabled="disabled"
        @click="select(null)"
      >
        <span class="kind-name">{{ t("loaderPicker.vanilla") }}</span>
        <span class="kind-status">{{ gameVersion ?? "" }}</span>
      </button>
      <button
        v-for="kind in LOADER_KINDS"
        :key="kind"
        type="button"
        role="radio"
        class="kind"
        :class="{ active: model?.kind === kind, failed: states[kind].status === 'error' }"
        :aria-checked="model?.kind === kind"
        :disabled="disabled || states[kind].status !== 'ready'"
        :title="states[kind].status === 'unsupported' ? t('loaderPicker.unsupportedHint') : ''"
        @click="select(kind)"
      >
        <span class="kind-name">{{ NAMES[kind] }}</span>
        <span class="kind-status">{{ status(kind) }}</span>
      </button>
    </div>

    <div v-if="model" class="version-row">
      <NSelect
        :value="model.version"
        :options="versionOptions"
        :render-label="(option: SelectOption) => renderOption({ option })"
        :disabled="disabled"
        filterable
        size="small"
        @update:value="(version: string) => model && (model = { kind: model.kind, version })"
      />
      <NCheckbox
        v-if="hasUnstable"
        v-model:checked="showUnstable"
        size="small"
        :disabled="disabled"
      >
        {{ t("loaderPicker.showUnstable") }}
      </NCheckbox>
    </div>

    <div v-for="item in errorText" :key="item.kind" class="note error">
      <span>{{
        t("loaderPicker.failedDetail", { name: NAMES[item.kind], error: item.text })
      }}</span>
      <NButton text size="tiny" type="primary" @click="retry(item.kind)">
        {{ t("loaderPicker.retry") }}
      </NButton>
    </div>
    <p v-if="sourceNote" class="note">{{ sourceNote }}</p>
  </div>
</template>

<style scoped>
.loader-picker {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
}

.kinds {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 6px;
}

.kind {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  min-width: 0;
  padding: 6px 8px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
  color: var(--tmcl-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition:
    background-color 0.15s var(--tmcl-ease),
    border-color 0.15s var(--tmcl-ease);
}

.kind:hover:not(:disabled) {
  background: var(--tmcl-subtle);
}

.kind:active:not(:disabled) {
  background: var(--tmcl-subtle-pressed);
}

.kind.active {
  border-color: var(--tmcl-accent);
  box-shadow: inset 0 0 0 1px var(--tmcl-accent);
}

.kind:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.kind.failed .kind-status {
  color: var(--n-error-color, #c42b1c);
}

.kind-name {
  font-size: 13px;
  font-weight: 600;
}

.kind-status {
  max-width: 100%;
  overflow: hidden;
  font-size: 11px;
  color: var(--tmcl-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.version-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.version-row > :deep(.n-select) {
  flex: 1;
}

.version-row :deep(.n-checkbox) {
  white-space: nowrap;
}

.note {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}

.note.error span {
  overflow-wrap: anywhere;
}

:global(.version-option) {
  display: flex;
  align-items: center;
  gap: 6px;
}
</style>
