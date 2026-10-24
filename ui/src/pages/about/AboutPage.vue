<script setup lang="ts">
// 关于页（PLAN.md M1-16）：版本、GPL-3.0 声明与第三方许可证列表。
// 列表由 scripts/gen-licenses.mjs 生成，约 700 KB，按需加载为单独的代码块。
import { Search24Regular } from "@vicons/fluent";
import { NButton, NEmpty, NIcon, NInput, NModal, NSpin, NTag } from "naive-ui";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { appInfo } from "../../api/app";
import type { AppInfo } from "../../bindings/AppInfo";
import logo from "./logo.png";
import { filterPackages, type LicenseData, type ThirdPartyPackage } from "./licenses";

const { t } = useI18n();

const info = ref<AppInfo | null>(null);
const data = ref<LicenseData | null>(null);
const query = ref("");
const selected = ref<ThirdPartyPackage | null>(null);
// 关闭时只收起弹窗、保留 selected，避免淡出动画期间内容变空。
const showLicense = ref(false);

function openLicense(pkg: ThirdPartyPackage) {
  selected.value = pkg;
  showLicense.value = true;
}

onMounted(async () => {
  // 浏览器中调试时没有后端，版本号显示为占位符。
  info.value = await appInfo().catch(() => null);
  data.value = (await import("./third-party-licenses.json")).default as LicenseData;
});

const packages = computed(() =>
  data.value ? filterPackages(data.value.packages, query.value) : [],
);
const selectedTexts = computed(() =>
  selected.value && data.value ? selected.value.texts.map((i) => data.value!.texts[i]) : [],
);
</script>

<template>
  <div class="about-page">
    <h1 class="page-title">{{ t("about.title") }}</h1>

    <section class="hero tmcl-card">
      <img :src="logo" alt="" class="logo" width="64" height="64" />
      <div>
        <div class="product">TMCL</div>
        <div class="secondary">
          {{
            info
              ? t("about.version", { version: info.version, os: info.os, arch: info.arch })
              : t("about.versionUnknown")
          }}
        </div>
        <div class="secondary">https://github.com/Jack-TJY/TMCL</div>
      </div>
    </section>

    <section class="notice">
      <p>{{ t("about.disclaimer") }}</p>
      <p>{{ t("about.license") }}</p>
    </section>

    <section>
      <h2 class="section-title">{{ t("about.thirdParty.title") }}</h2>
      <p class="secondary">{{ t("about.thirdParty.description") }}</p>
      <NSpin v-if="!data" class="loading" />
      <template v-else>
        <NInput v-model:value="query" clearable :placeholder="t('about.thirdParty.search')">
          <template #prefix>
            <NIcon><Search24Regular /></NIcon>
          </template>
        </NInput>
        <p class="secondary count">
          {{ t("about.thirdParty.count", { count: packages.length }) }}
        </p>
        <NEmpty v-if="packages.length === 0" :description="t('about.thirdParty.noMatch')" />
        <ul v-else class="packages">
          <li v-for="pkg in packages" :key="`${pkg.source}:${pkg.name}@${pkg.version}`">
            <button type="button" class="package" @click="openLicense(pkg)">
              <span class="name">{{ pkg.name }}</span>
              <span class="secondary version">{{ pkg.version }}</span>
              <NTag size="small" :bordered="false">{{ t(`about.source.${pkg.source}`) }}</NTag>
              <span class="secondary license">{{ pkg.license }}</span>
            </button>
          </li>
        </ul>
      </template>
    </section>

    <NModal
      v-model:show="showLicense"
      preset="card"
      class="license-modal"
      :title="selected ? `${selected.name} ${selected.version}` : ''"
    >
      <template v-if="selected">
        <p class="secondary">{{ selected.license }} · {{ selected.url }}</p>
        <pre v-for="(text, index) in selectedTexts" :key="index" class="license-text">{{
          text
        }}</pre>
        <p v-if="selectedTexts.length === 0" class="secondary">
          {{ t("about.thirdParty.noText") }}
        </p>
      </template>
      <template #footer>
        <NButton @click="showLicense = false">{{ t("about.close") }}</NButton>
      </template>
    </NModal>
  </div>
</template>

<style scoped>
.about-page {
  max-width: 1000px;
}

section + section {
  margin-top: 24px;
}

.hero {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
}

.logo {
  border-radius: 12px;
}

.product {
  font-size: 20px;
  font-weight: 600;
  line-height: 28px;
}

.secondary {
  color: var(--tmcl-text-secondary);
  font-size: 12px;
  line-height: 18px;
}

.notice p {
  margin: 8px 0;
}

.section-title {
  margin: 0 0 4px;
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
}

.loading {
  display: block;
  margin: 24px auto;
}

.count {
  margin: 8px 0;
}

.packages {
  margin: 0;
  padding: 0;
  list-style: none;
  border: 1px solid var(--tmcl-stroke);
  border-radius: 4px;
  background: var(--tmcl-card);
}

.packages li + li {
  border-top: 1px solid var(--tmcl-stroke);
}

.package {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 16px;
  border: none;
  background: none;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.package:hover {
  background: var(--tmcl-subtle);
}

.name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.license {
  margin-left: auto;
  text-align: right;
}

.license-text {
  max-height: 50vh;
  overflow: auto;
  padding: 12px;
  border-radius: 4px;
  background: var(--tmcl-layer);
  font-size: 12px;
  white-space: pre-wrap;
}
</style>

<style>
.license-modal {
  width: min(720px, 90vw);
}
</style>
