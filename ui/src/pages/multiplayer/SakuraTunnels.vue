<script setup lang="ts">
// 我的隧道：普通用户同时只能有少量隧道，满了就开不了房，这里列出账号下的全部隧道，可以删掉不用的。
// TMCL 的临时隧道在停止联机时会自动删除，这里主要用于清理在官网手动建的隧道。
import { ArrowClockwise20Regular, Delete20Regular } from "@vicons/fluent";
import {
  NAlert,
  NButton,
  NCollapse,
  NCollapseItem,
  NIcon,
  NTag,
  useDialog,
  useMessage,
} from "naive-ui";
import { onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import {
  SAKURA_TEMP_NOTE,
  sakuraDeleteTunnel,
  sakuraTunnels,
  type SakuraTunnel,
} from "../../api/sakura";
import type { AppError } from "../../bindings/AppError";
import { formatError } from "../../i18n";

const props = defineProps<{ limit: number; reload: number }>();

const { t } = useI18n();
const message = useMessage();
const dialog = useDialog();

const tunnels = ref<SakuraTunnel[]>([]);
const loading = ref(false);
const loadError = ref<AppError | null>(null);
const deleting = ref<number | null>(null);

async function load() {
  loading.value = true;
  loadError.value = null;
  try {
    tunnels.value = await sakuraTunnels();
  } catch (err) {
    loadError.value = toAppError(err);
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(() => props.reload, load);

async function remove(tunnel: SakuraTunnel) {
  deleting.value = tunnel.id;
  try {
    await sakuraDeleteTunnel(tunnel.id);
    message.success(t("multiplayer.sakura.tunnels.deleted", { name: tunnel.name }));
    await load();
  } catch (err) {
    message.error(formatError(toAppError(err)));
  } finally {
    deleting.value = null;
  }
}

function confirmRemove(tunnel: SakuraTunnel) {
  dialog.warning({
    title: t("multiplayer.sakura.tunnels.deleteTitle", { name: tunnel.name }),
    content: t("multiplayer.sakura.tunnels.deleteContent"),
    positiveText: t("multiplayer.sakura.tunnels.delete"),
    negativeText: t("multiplayer.cancel"),
    onPositiveClick: () => void remove(tunnel),
  });
}
</script>

<template>
  <NCollapse class="tunnels" :default-expanded-names="[]">
    <NCollapseItem
      name="tunnels"
      :title="t('multiplayer.sakura.tunnels.title', { count: tunnels.length, limit })"
    >
      <template #header-extra>
        <NButton quaternary size="small" :loading="loading" @click.stop="load">
          <template #icon
            ><NIcon><ArrowClockwise20Regular /></NIcon
          ></template>
        </NButton>
      </template>
      <p class="muted">{{ t("multiplayer.sakura.tunnels.hint") }}</p>
      <NAlert v-if="loadError" type="error">{{ formatError(loadError) }}</NAlert>
      <p v-else-if="!loading && tunnels.length === 0" class="muted">
        {{ t("multiplayer.sakura.tunnels.empty") }}
      </p>
      <div v-for="tunnel in tunnels" :key="tunnel.id" class="tunnel">
        <div class="info">
          <span class="name">{{ tunnel.name }}</span>
          <NTag v-if="tunnel.note === SAKURA_TEMP_NOTE" size="small" :bordered="false">
            {{ t("multiplayer.sakura.tunnels.temporary") }}
          </NTag>
          <NTag size="small" :type="tunnel.online ? 'success' : 'default'" :bordered="false">
            {{
              tunnel.online
                ? t("multiplayer.sakura.tunnels.online")
                : t("multiplayer.sakura.tunnels.offline")
            }}
          </NTag>
          <span class="muted">
            {{
              t("multiplayer.sakura.tunnels.details", {
                type: tunnel.type.toUpperCase(),
                node: tunnel.node,
                local: `${tunnel.local_ip}:${tunnel.local_port}`,
              })
            }}
          </span>
        </div>
        <NButton
          quaternary
          size="small"
          :loading="deleting === tunnel.id"
          @click="confirmRemove(tunnel)"
        >
          <template #icon
            ><NIcon><Delete20Regular /></NIcon
          ></template>
          {{ t("multiplayer.sakura.tunnels.delete") }}
        </NButton>
      </div>
    </NCollapseItem>
  </NCollapse>
</template>

<style scoped>
.tunnels {
  box-sizing: border-box;
  margin-top: 12px;
  padding: 12px 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
}

.muted {
  margin: 0 0 8px;
  color: var(--tmcl-text-secondary);
}

.tunnel {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 0;
  border-top: 1px solid var(--tmcl-stroke);
}

.info {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.info .muted {
  margin: 0;
}

.name {
  font-weight: 600;
}
</style>
