<script setup lang="ts">
// 皮肤页（PLAN.md M2-08）：左侧 3D 预览（动作、Steve/Alex、披风、自动旋转），右侧当前账号的
// 皮肤与本地皮肤库。拖入 PNG 即预览，可再加入皮肤库。
import { Add20Regular } from "@vicons/fluent";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  NAlert,
  NButton,
  NIcon,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSpin,
  NSwitch,
  useDialog,
  useMessage,
} from "naive-ui";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { pickFile } from "../../api/dialog";
import { toAppError } from "../../api/invoke";
import {
  pngObjectUrl,
  skinFileRead,
  skinLibraryImport,
  skinLibraryList,
  skinLibraryRead,
  skinLibraryRemove,
  skinTexture,
  type ProfileSkin,
  type SkinEntry,
  type SkinVariant,
} from "../../api/skin";
import { SKIN_ANIMATIONS, type SkinAnimation } from "../../components/skin-viewer/animations";
import SkinViewer from "../../components/skin-viewer/SkinViewer.vue";
import { formatError } from "../../i18n";
import { useAccountsStore } from "../../stores/accounts";
import AccountSkinPanel from "./AccountSkinPanel.vue";
import EditSkinDialog from "./EditSkinDialog.vue";
import SkinLibraryGrid from "./SkinLibraryGrid.vue";
import { fileStem, firstPng } from "./skins";

/** 正在预览的皮肤来自哪里。 */
type Preview =
  | { kind: "library"; entry: SkinEntry }
  | { kind: "file"; path: string; name: string }
  | { kind: "profile"; skin: ProfileSkin };

const PNG_FILTER = [{ name: "PNG", extensions: ["png"] }];

const { t } = useI18n();
const message = useMessage();
const dialog = useDialog();
const accounts = useAccountsStore();

const entries = ref<SkinEntry[]>([]);
const urls = ref<Record<string, string>>({});
const loading = ref(false);

const preview = ref<Preview | null>(null);
/** 拖入文件或档案皮肤的 object URL，由本页负责释放；皮肤库的图片在 `urls` 中。 */
const ownedUrl = ref<string | null>(null);
const variant = ref<SkinVariant>("classic");
const animation = ref<SkinAnimation>("walk");
const autoRotate = ref(false);
const capeUrl = ref<string | null>(null);
const showCape = ref(true);
const viewerError = ref<"webgl" | "load" | null>(null);
const dragging = ref(false);
const editing = ref<SkinEntry | null>(null);
const importing = ref(false);

const skinUrl = computed(() => {
  const current = preview.value;
  if (!current) return null;
  return current.kind === "library" ? (urls.value[current.entry.id] ?? null) : ownedUrl.value;
});
const selected = computed(() => (preview.value?.kind === "library" ? preview.value.entry : null));
const previewName = computed(() => {
  const current = preview.value;
  if (!current) return "";
  if (current.kind === "library") return current.entry.name;
  if (current.kind === "file") return current.name;
  return t("skin.preview.profile");
});
const animationOptions = computed(() =>
  SKIN_ANIMATIONS.map((value) => ({ value, label: t(`skin.animation.${value}`) })),
);

function showError(err: unknown) {
  message.error(formatError(toAppError(err)));
}

function setOwnedUrl(url: string | null) {
  if (ownedUrl.value) URL.revokeObjectURL(ownedUrl.value);
  ownedUrl.value = url;
}

async function loadLibrary() {
  loading.value = true;
  try {
    entries.value = await skinLibraryList();
    const ids = new Set(entries.value.map((e) => e.id));
    for (const [id, url] of Object.entries(urls.value)) {
      if (!ids.has(id)) URL.revokeObjectURL(url);
    }
    const kept = Object.fromEntries(Object.entries(urls.value).filter(([id]) => ids.has(id)));
    urls.value = kept;
    await Promise.all(
      entries.value
        .filter((e) => !kept[e.id])
        .map(async (e) => {
          try {
            urls.value[e.id] = pngObjectUrl(await skinLibraryRead({ id: e.id }));
          } catch {
            // 读不到的图片只是没有缩略图，选中时会再提示。
          }
        }),
    );
  } catch (err) {
    showError(err);
  } finally {
    loading.value = false;
  }
}

function selectEntry(entry: SkinEntry) {
  viewerError.value = null;
  setOwnedUrl(null);
  preview.value = { kind: "library", entry };
  variant.value = entry.variant;
  if (!urls.value[entry.id]) message.error(t("skin.preview.unreadable"));
}

async function previewProfileSkin(skin: ProfileSkin) {
  try {
    const url = pngObjectUrl(await skinTexture({ url: skin.url }));
    viewerError.value = null;
    setOwnedUrl(url);
    preview.value = { kind: "profile", skin };
    variant.value = skin.variant;
  } catch (err) {
    showError(err);
  }
}

function onActiveSkin(skin: ProfileSkin | null) {
  if (skin && preview.value === null) void previewProfileSkin(skin);
}

async function onCape(url: string | null) {
  if (capeUrl.value) URL.revokeObjectURL(capeUrl.value);
  capeUrl.value = null;
  if (!url) return;
  try {
    capeUrl.value = pngObjectUrl(await skinTexture({ url }));
  } catch (err) {
    showError(err);
  }
}

async function previewFile(path: string) {
  try {
    const url = pngObjectUrl(await skinFileRead({ path }));
    viewerError.value = null;
    setOwnedUrl(url);
    preview.value = { kind: "file", path, name: fileStem(path) };
  } catch (err) {
    showError(err);
  }
}

async function importPath(path: string) {
  importing.value = true;
  try {
    const entry = await skinLibraryImport({ path, variant: variant.value });
    await loadLibrary();
    selectEntry(entries.value.find((e) => e.id === entry.id) ?? entry);
    message.success(t("skin.library.imported", { name: entry.name }));
  } catch (err) {
    showError(err);
  } finally {
    importing.value = false;
  }
}

async function onSavedCurrent(entry: SkinEntry) {
  await loadLibrary();
  selectEntry(entries.value.find((e) => e.id === entry.id) ?? entry);
  message.success(t("skin.library.imported", { name: entry.name }));
}

async function pickAndImport() {
  const path = await pickFile(PNG_FILTER);
  if (path) await importPath(path);
}

function confirmRemove(entry: SkinEntry) {
  dialog.warning({
    title: t("skin.remove.title", { name: entry.name }),
    content: t("skin.remove.content"),
    positiveText: t("skin.remove.confirm"),
    negativeText: t("skin.cancel"),
    onPositiveClick: async () => {
      try {
        await skinLibraryRemove({ id: entry.id });
        if (selected.value?.id === entry.id) preview.value = null;
        await loadLibrary();
      } catch (err) {
        showError(err);
      }
    },
  });
}

function onSaved(entry: SkinEntry) {
  entries.value = entries.value.map((e) => (e.id === entry.id ? entry : e));
  if (selected.value?.id === entry.id) {
    preview.value = { kind: "library", entry };
    variant.value = entry.variant;
  }
}

let unlistenDrop: (() => void) | null = null;
let unmounted = false;

onMounted(async () => {
  void loadLibrary();
  accounts.ensureLoaded().catch(showError);
  try {
    const unlisten = await getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter" || payload.type === "over") dragging.value = true;
      else if (payload.type === "leave") dragging.value = false;
      else {
        dragging.value = false;
        const path = firstPng(payload.paths);
        if (path) void previewFile(path);
        else message.warning(t("skin.drop.notPng"));
      }
    });
    if (unmounted) unlisten();
    else unlistenDrop = unlisten;
  } catch {
    // 不在 Tauri 中（例如浏览器调试）时没有拖放事件。
  }
});

onBeforeUnmount(() => {
  unmounted = true;
  unlistenDrop?.();
  setOwnedUrl(null);
  if (capeUrl.value) URL.revokeObjectURL(capeUrl.value);
  Object.values(urls.value).forEach((url) => URL.revokeObjectURL(url));
});
</script>

<template>
  <div class="skin-page">
    <h1 class="page-title">{{ t("shell.nav.skin") }}</h1>

    <div class="body">
      <section class="preview" :class="{ dragging }">
        <div class="stage">
          <NAlert v-if="viewerError === 'webgl'" type="warning" class="stage-alert">
            {{ t("skin.preview.webgl") }}
          </NAlert>
          <template v-else>
            <SkinViewer
              :skin="skinUrl"
              :variant="variant"
              :cape="showCape ? capeUrl : null"
              :animation="animation"
              :auto-rotate="autoRotate"
              @error="(kind) => (viewerError = kind)"
            />
            <div v-if="!skinUrl" class="stage-hint">{{ t("skin.preview.empty") }}</div>
          </template>
          <div v-if="dragging" class="drop-overlay">{{ t("skin.drop.hint") }}</div>
        </div>

        <div class="preview-bar">
          <span class="preview-name" :title="previewName">{{ previewName }}</span>
          <NButton
            v-if="preview?.kind === 'file'"
            size="small"
            type="primary"
            :loading="importing"
            @click="importPath(preview.path)"
          >
            {{ t("skin.library.add") }}
          </NButton>
        </div>
        <NAlert v-if="viewerError === 'load'" type="error" :show-icon="false">
          {{ t("skin.preview.loadFailed") }}
        </NAlert>

        <div class="controls">
          <div class="control">
            <span class="label">{{ t("skin.model.label") }}</span>
            <NRadioGroup v-model:value="variant" size="small">
              <NRadioButton value="classic">{{ t("skin.model.steve") }}</NRadioButton>
              <NRadioButton value="slim">{{ t("skin.model.alex") }}</NRadioButton>
            </NRadioGroup>
          </div>
          <div class="control">
            <span class="label">{{ t("skin.animation.label") }}</span>
            <NSelect
              v-model:value="animation"
              class="animation-select"
              size="small"
              :options="animationOptions"
            />
          </div>
          <div class="control">
            <span class="label">{{ t("skin.preview.cape") }}</span>
            <NSwitch v-model:value="showCape" size="small" :disabled="!capeUrl" />
            <span class="label spaced">{{ t("skin.preview.autoRotate") }}</span>
            <NSwitch v-model:value="autoRotate" size="small" />
          </div>
        </div>
      </section>

      <div class="side">
        <AccountSkinPanel
          :account="accounts.current"
          :selected="selected"
          @preview-skin="previewProfileSkin"
          @active-skin="onActiveSkin"
          @cape="onCape"
          @saved="onSavedCurrent"
        />

        <section class="library">
          <div class="library-header">
            <h2 class="section-title">{{ t("skin.library.title") }}</h2>
            <span class="hint">{{ t("skin.drop.tip") }}</span>
            <div class="spacer" />
            <NButton size="small" :loading="importing" @click="pickAndImport">
              <template #icon
                ><NIcon><Add20Regular /></NIcon
              ></template>
              {{ t("skin.library.import") }}
            </NButton>
          </div>
          <NSpin :show="loading && entries.length === 0" class="library-list">
            <SkinLibraryGrid
              :entries="entries"
              :urls="urls"
              :selected-id="selected?.id ?? null"
              @select="selectEntry"
              @edit="(entry) => (editing = entry)"
              @remove="confirmRemove"
            />
          </NSpin>
        </section>
      </div>
    </div>

    <EditSkinDialog :entry="editing" @close="editing = null" @saved="onSaved" />
  </div>
</template>

<style scoped>
.skin-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 1200px;
}

.body {
  display: flex;
  flex: 1;
  gap: 16px;
  min-height: 0;
}

.preview {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: 8px;
  width: 300px;
}

.stage {
  position: relative;
  flex: 1;
  min-height: 240px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 8px;
  background: var(--tmcl-card);
  overflow: hidden;
}

.preview.dragging .stage {
  border-color: var(--tmcl-accent);
}

.stage-hint,
.drop-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  text-align: center;
  color: var(--tmcl-text-secondary);
  pointer-events: none;
}

.drop-overlay {
  background: color-mix(in srgb, var(--tmcl-accent) 12%, transparent);
  color: var(--tmcl-accent);
  font-weight: 600;
}

.stage-alert {
  margin: 12px;
}

.preview-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 28px;
}

.preview-name {
  flex: 1;
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.controls {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.control {
  display: flex;
  align-items: center;
  gap: 8px;
}

.label {
  min-width: 40px;
  color: var(--tmcl-text-secondary);
}

.label.spaced {
  margin-left: 16px;
}

.animation-select {
  flex: 1;
}

.side {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
}

.library {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}

.library-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.section-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}

.hint {
  color: var(--tmcl-text-secondary);
  font-size: 12px;
}

.spacer {
  flex: 1;
}

.library-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
</style>
