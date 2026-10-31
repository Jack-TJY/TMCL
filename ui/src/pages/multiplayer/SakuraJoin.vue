<script setup lang="ts">
// 樱花穿透成员（PLAN.md M2-15）：成员不需要账号，粘贴房主分享的地址，测一下连通性，再到游戏里直接连接。
import { Copy20Regular, PlugConnected20Regular } from "@vicons/fluent";
import { NAlert, NButton, NIcon, NInput, useMessage, useThemeVars } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { sakuraPing, type SakuraPing } from "../../api/sakura";
import LatencyTag from "./LatencyTag.vue";
import { normalizeAddress } from "./multiplayer";

const { t } = useI18n();
const message = useMessage();
const themeVars = useThemeVars();

const input = ref("");
const testing = ref(false);
/** 最近一次测试的结果；`undefined` 为还没测。 */
const check = ref<SakuraPing | undefined>(undefined);
const latency = computed(() => (check.value ? check.value.latency_ms : undefined));

const address = computed(() => normalizeAddress(input.value));
const invalid = computed(() => input.value.trim() !== "" && address.value === null);

watch(address, () => (check.value = undefined));

async function test() {
  const value = address.value;
  if (!value) return;
  testing.value = true;
  try {
    check.value = await sakuraPing(value).catch(() => ({
      latency_ms: null,
      game_responding: null,
    }));
  } finally {
    testing.value = false;
  }
}

async function copy() {
  if (!address.value) return;
  try {
    await navigator.clipboard.writeText(address.value);
    message.success(t("multiplayer.copied"));
  } catch {
    message.error(t("multiplayer.copyFailed"));
  }
}
</script>

<template>
  <div class="card">
    <ol class="steps">
      <li>{{ t("multiplayer.sakura.join.step1") }}</li>
      <li>{{ t("multiplayer.sakura.join.step2") }}</li>
      <li>{{ t("multiplayer.sakura.join.step3") }}</li>
    </ol>
    <div class="row">
      <NInput
        v-model:value="input"
        class="address"
        clearable
        :status="invalid ? 'error' : undefined"
        :placeholder="t('multiplayer.sakura.join.placeholder')"
        @keyup.enter="test"
      />
      <NButton :disabled="!address" :loading="testing" @click="test">
        <template #icon
          ><NIcon><PlugConnected20Regular /></NIcon
        ></template>
        {{ t("multiplayer.sakura.join.test") }}
      </NButton>
      <NButton secondary :disabled="!address" @click="copy">
        <template #icon
          ><NIcon><Copy20Regular /></NIcon
        ></template>
        {{ t("multiplayer.copy") }}
      </NButton>
      <LatencyTag v-if="latency !== undefined && !testing" :latency="latency" />
    </div>
    <p v-if="invalid" class="error" :style="{ color: themeVars.errorColor }">
      {{ t("multiplayer.sakura.join.invalid") }}
    </p>
    <NAlert v-if="latency === null && !testing" type="warning" class="gap">
      {{ t("multiplayer.sakura.join.unreachable") }}
    </NAlert>
    <NAlert v-else-if="check?.game_responding === false && !testing" type="warning" class="gap">
      {{ t("multiplayer.sakura.join.gameSilent") }}
    </NAlert>
    <NAlert v-else-if="typeof latency === 'number' && !testing" type="success" class="gap">
      {{ t("multiplayer.sakura.join.reachable") }}
    </NAlert>
  </div>
</template>

<style scoped>
.card {
  margin-top: 12px;
  padding: 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
}

.steps {
  margin: 0 0 12px;
  padding-left: 20px;
  line-height: 22px;
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.address {
  width: 320px;
}

.error {
  margin: 4px 0 0;
  font-size: 12px;
}

.gap {
  margin-top: 12px;
}
</style>
