<script setup lang="ts">
// 联机页（PLAN.md M2-15）：“直连”与“樱花穿透”两种方式，各分房主与成员两种流程。
// 上次选的方式与角色记在本机 WebView 中。
import { NRadioButton, NRadioGroup, NTab, NTabs } from "naive-ui";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import DirectPanel from "./DirectPanel.vue";
import SakuraHost from "./SakuraHost.vue";
import SakuraJoin from "./SakuraJoin.vue";
import { loadPrefs, savePrefs, type MultiplayerMode, type MultiplayerRole } from "./multiplayer";

const { t } = useI18n();

const prefs = loadPrefs();
const mode = ref<MultiplayerMode>(prefs.mode);
const role = ref<MultiplayerRole>(prefs.role);

watch([mode, role], ([m, r]) => savePrefs({ mode: m, role: r }));
</script>

<template>
  <div class="multiplayer-page">
    <h1 class="page-title">{{ t("shell.nav.multiplayer") }}</h1>
    <div class="body">
      <NTabs v-model:value="mode" type="line">
        <NTab name="direct">{{ t("multiplayer.mode.direct") }}</NTab>
        <NTab name="sakura">{{ t("multiplayer.mode.sakura") }}</NTab>
      </NTabs>
      <p class="muted">
        {{
          mode === "direct" ? t("multiplayer.mode.directHint") : t("multiplayer.mode.sakuraHint")
        }}
      </p>
      <NRadioGroup v-model:value="role">
        <NRadioButton value="host">{{ t("multiplayer.role.host") }}</NRadioButton>
        <NRadioButton value="member">{{ t("multiplayer.role.member") }}</NRadioButton>
      </NRadioGroup>

      <DirectPanel v-if="mode === 'direct'" :role="role" @use-sakura="mode = 'sakura'" />
      <template v-else>
        <SakuraHost v-if="role === 'host'" />
        <SakuraJoin v-else />
        <p class="muted credit">{{ t("multiplayer.sakura.credit") }}</p>
      </template>
    </div>
  </div>
</template>

<style scoped>
.multiplayer-page {
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

.muted {
  margin: 8px 0 12px;
  color: var(--tmcl-text-secondary);
}

.credit {
  font-size: 12px;
}
</style>
