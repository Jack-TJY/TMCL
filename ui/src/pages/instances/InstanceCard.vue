<script setup lang="ts">
// 实例卡片：名称、版本与加载器、最近游玩时间；右上角菜单提供实例操作。
import {
  Apps20Regular,
  Copy20Regular,
  Delete20Regular,
  Edit20Regular,
  FolderOpen20Regular,
  MoreHorizontal20Regular,
  PuzzlePiece20Regular,
  Settings20Regular,
  Share20Regular,
} from "@vicons/fluent";
import { NButton, NDropdown, NIcon, NTag, type DropdownOption } from "naive-ui";
import { computed, h, type Component } from "vue";
import { useI18n } from "vue-i18n";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import { canImportMods } from "./instances";

export type CardAction =
  "details" | "resources" | "open" | "importMods" | "rename" | "duplicate" | "export" | "delete";

const props = defineProps<{ instance: InstanceSummary }>();
const emit = defineEmits<{ action: [action: CardAction] }>();

const { t, locale } = useI18n();

const broken = computed(() => props.instance.error !== null);

function icon(component: Component) {
  return () => h(NIcon, null, { default: () => h(component) });
}

const menu = computed<DropdownOption[]>(() => {
  const items: [CardAction, Component][] = broken.value
    ? [
        ["open", FolderOpen20Regular],
        ["delete", Delete20Regular],
      ]
    : [
        ["details", Settings20Regular],
        ["resources", Apps20Regular],
        ["open", FolderOpen20Regular],
        ...(canImportMods(props.instance)
          ? [["importMods", PuzzlePiece20Regular] as [CardAction, Component]]
          : []),
        ["rename", Edit20Regular],
        ["duplicate", Copy20Regular],
        ["export", Share20Regular],
        ["delete", Delete20Regular],
      ];
  return items.map(([key, component]) => ({
    key,
    label: t(`instances.action.${key}`),
    icon: icon(component),
  }));
});

const lastPlayed = computed(() => {
  const value = props.instance.last_played_at;
  if (!value) return t("instances.neverPlayed");
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? ""
    : t("instances.lastPlayed", { date: date.toLocaleDateString(locale.value) });
});

function open() {
  if (!broken.value) emit("action", "details");
}
</script>

<template>
  <div
    class="instance-card"
    :class="{ broken }"
    role="button"
    tabindex="0"
    @click="open"
    @keyup.enter="open"
  >
    <div class="card-head">
      <div class="card-name" :title="instance.name">{{ instance.name }}</div>
      <NDropdown trigger="click" :options="menu" @select="(key: CardAction) => emit('action', key)">
        <NButton quaternary circle size="small" :aria-label="t('instances.more')" @click.stop>
          <template #icon
            ><NIcon><MoreHorizontal20Regular /></NIcon
          ></template>
        </NButton>
      </NDropdown>
    </div>
    <div class="card-tags">
      <template v-if="broken">
        <NTag size="small" type="error" :bordered="false">{{ t("instances.broken") }}</NTag>
      </template>
      <template v-else>
        <NTag size="small" :bordered="false">{{ instance.game_version }}</NTag>
        <NTag size="small" :bordered="false" :type="instance.loader ? 'info' : 'default'">
          {{
            instance.loader
              ? `${t(`instances.loader.${instance.loader.kind}`)} ${instance.loader.version}`
              : t("instances.loader.vanilla")
          }}
        </NTag>
      </template>
    </div>
    <div class="card-foot">
      <slot name="actions" />
      <span class="card-meta">{{ broken ? t("instances.brokenHint") : lastPlayed }}</span>
    </div>
  </div>
</template>

<style scoped>
.instance-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 12px 12px 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 8px;
  background: var(--tmcl-card);
  cursor: pointer;
  transition: background-color 0.15s var(--tmcl-ease);
}

.instance-card:not(.broken):hover {
  background: var(--tmcl-subtle);
}

.instance-card.broken {
  cursor: default;
}

.card-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.card-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.card-foot {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 28px;
}

.card-meta {
  /* 靠右放，与左侧的启动、日志按钮拉开距离 */
  margin-left: auto;
  min-width: 0;
  text-align: right;
  font-size: 12px;
  color: var(--tmcl-text-secondary);
}
</style>
