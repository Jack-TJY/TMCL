<script setup lang="ts">
// 已安装列表：图标、名称、版本与文件名；开关启用/禁用，删除前确认。列表由资源页读取并传入。
import { ArrowClockwise20Regular, Delete20Regular, Search20Regular } from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NEmpty,
  NIcon,
  NInput,
  NSpin,
  NSwitch,
  NTooltip,
  useDialog,
  useMessage,
} from "naive-ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import {
  resourcesRemove,
  resourcesSetEnabled,
  type InstalledList,
  type InstalledResource,
  type ResourceKind,
} from "../../api/resources";
import type { AppError } from "../../bindings/AppError";
import { formatError } from "../../i18n";
import ResourceIcon from "./ResourceIcon.vue";
import { displayName, filterInstalled, formatSize } from "./resources";

const props = defineProps<{
  instanceId: string;
  kind: ResourceKind;
  world: string | null;
  list: InstalledList | null;
  loading: boolean;
  error: AppError | null;
}>();
const emit = defineEmits<{ reload: [] }>();

const { t } = useI18n();
const message = useMessage();
const dialog = useDialog();

const query = ref("");
/** 正在改名或删除的文件，期间禁用该行的操作。 */
const pending = ref(new Set<string>());

const resources = computed(() => props.list?.resources ?? []);
const shown = computed(() => filterInstalled(resources.value, query.value));
const enabledCount = computed(() => resources.value.filter((r) => r.enabled).length);

function target() {
  return { instance_id: props.instanceId, kind: props.kind, world: props.world ?? undefined };
}

async function withPending(resource: InstalledResource, run: () => Promise<void>) {
  pending.value.add(resource.file_name);
  try {
    await run();
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    pending.value.delete(resource.file_name);
    emit("reload");
  }
}

function toggle(resource: InstalledResource, enabled: boolean) {
  void withPending(resource, async () => {
    await resourcesSetEnabled({ ...target(), file_name: resource.file_name, enabled });
  });
}

function confirmRemove(resource: InstalledResource) {
  const name = displayName(resource);
  dialog.warning({
    title: t("resources.installed.deleteTitle", { name }),
    content: t("resources.installed.deleteContent", { file: resource.file_name }),
    positiveText: t("resources.installed.delete"),
    negativeText: t("resources.installed.cancel"),
    onPositiveClick: () =>
      withPending(resource, async () => {
        await resourcesRemove({ ...target(), file_name: resource.file_name });
        message.success(t("resources.installed.deleted", { name }));
      }),
  });
}
</script>

<template>
  <div class="installed">
    <div class="toolbar">
      <NInput
        v-model:value="query"
        class="search"
        clearable
        :placeholder="t('resources.installed.filter')"
      >
        <template #prefix
          ><NIcon><Search20Regular /></NIcon
        ></template>
      </NInput>
      <span v-if="resources.length" class="count">
        {{ t("resources.installed.count", { count: resources.length, enabled: enabledCount }) }}
      </span>
      <div class="spacer" />
      <slot name="actions" />
      <NButton quaternary :loading="loading" @click="emit('reload')">
        <template #icon
          ><NIcon><ArrowClockwise20Regular /></NIcon
        ></template>
        {{ t("resources.installed.refresh") }}
      </NButton>
    </div>

    <NAlert v-if="error" type="error" :title="t('resources.installed.loadFailed')">
      {{ formatError(error) }}
    </NAlert>
    <NAlert
      v-else-if="list?.lookup_error"
      type="warning"
      :title="t('resources.installed.lookupFailed')"
    >
      {{ formatError(list.lookup_error) }}
    </NAlert>
    <slot name="notice" />

    <NSpin :show="loading && !list" class="list">
      <NEmpty
        v-if="list && shown.length === 0"
        class="empty"
        :description="
          resources.length === 0 ? t('resources.installed.empty') : t('resources.installed.noMatch')
        "
      />
      <div
        v-for="resource in shown"
        :key="resource.file_name"
        class="row"
        :class="{ disabled: !resource.enabled }"
      >
        <ResourceIcon :url="resource.source?.icon_url ?? null" :size="36" />
        <div class="row-body">
          <div class="row-title">
            {{ displayName(resource) }}
            <span class="row-version">
              {{ resource.source?.version_number ?? t("resources.installed.unknownSource") }}
            </span>
          </div>
          <div class="row-file">{{ resource.file_name }} · {{ formatSize(resource.size) }}</div>
        </div>
        <slot name="row" :resource="resource" />
        <NTooltip>
          <template #trigger>
            <NSwitch
              :value="resource.enabled"
              :loading="pending.has(resource.file_name)"
              :disabled="pending.has(resource.file_name)"
              @update:value="(value: boolean) => toggle(resource, value)"
            />
          </template>
          {{
            resource.enabled ? t("resources.installed.disable") : t("resources.installed.enable")
          }}
        </NTooltip>
        <NButton
          quaternary
          circle
          size="small"
          :aria-label="t('resources.installed.delete')"
          :disabled="pending.has(resource.file_name)"
          @click="confirmRemove(resource)"
        >
          <template #icon
            ><NIcon><Delete20Regular /></NIcon
          ></template>
        </NButton>
      </div>
    </NSpin>
  </div>
</template>

<style scoped>
.installed {
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 100%;
  min-height: 0;
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.search {
  flex: 1;
  max-width: 280px;
}

.count {
  font-size: 13px;
  color: var(--tmcl-text-secondary);
}

.spacer {
  flex: 1;
}

.list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.empty {
  padding: 48px 0;
}

.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-radius: 8px;
}

.row:hover {
  background: var(--tmcl-subtle);
}

.row.disabled .row-body,
.row.disabled :deep(.icon) {
  opacity: 0.55;
}

.row-body {
  flex: 1;
  min-width: 0;
}

.row-title {
  overflow: hidden;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.row-version {
  margin-left: 6px;
  font-weight: 400;
  color: var(--tmcl-text-secondary);
}

.row-file {
  overflow: hidden;
  font-size: 12px;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--tmcl-text-secondary);
}
</style>
