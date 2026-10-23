<script setup lang="ts">
// 应用框架（PLAN.md M1-11）：自绘标题栏 + Win11 NavigationView 式左侧导航 + 内容图层。
import { Navigation24Regular } from "@vicons/fluent";
import { NIcon, NTooltip } from "naive-ui";
import { onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { FOOTER_NAV, MAIN_NAV, type NavItem } from "./nav";
import TitleBar from "./TitleBar.vue";

const { t } = useI18n();
const route = useRoute();

/** 与 Win11 NavigationView 一致：窗口宽度小于 1008 px 时收起为图标栏，用户仍可手动展开。 */
const narrow = window.matchMedia("(max-width: 1007px)");
const collapsed = ref(narrow.matches);
const onNarrowChange = (e: MediaQueryListEvent) => (collapsed.value = e.matches);
narrow.addEventListener("change", onNarrowChange);
onBeforeUnmount(() => narrow.removeEventListener("change", onNarrowChange));

function isActive(item: NavItem): boolean {
  return route.name === item.name || route.path.startsWith(`/${item.name}/`);
}
</script>

<template>
  <div class="app-layout">
    <TitleBar title="TMCL">
      <template #leading>
        <button
          class="nav-toggle"
          :aria-label="collapsed ? t('shell.nav.expand') : t('shell.nav.collapse')"
          :aria-expanded="!collapsed"
          @click="collapsed = !collapsed"
        >
          <NIcon :size="16"><Navigation24Regular /></NIcon>
        </button>
      </template>
    </TitleBar>
    <div class="app-body">
      <nav class="nav-pane" :class="{ collapsed }" :aria-label="t('shell.nav.label')">
        <template v-for="(group, index) in [MAIN_NAV, FOOTER_NAV]" :key="index">
          <div class="nav-group" :class="{ 'nav-footer': index === 1 }">
            <NTooltip
              v-for="item in group"
              :key="item.name"
              placement="right"
              :disabled="!collapsed"
            >
              <template #trigger>
                <RouterLink
                  :to="{ name: item.name }"
                  class="nav-item"
                  :class="{ active: isActive(item) }"
                  :aria-current="isActive(item) ? 'page' : undefined"
                >
                  <NIcon :size="20" class="nav-icon">
                    <component :is="isActive(item) ? item.activeIcon : item.icon" />
                  </NIcon>
                  <span v-if="!collapsed" class="nav-label">{{ t(item.labelKey) }}</span>
                </RouterLink>
              </template>
              {{ t(item.labelKey) }}
            </NTooltip>
          </div>
        </template>
      </nav>
      <main class="content-layer">
        <RouterView />
      </main>
    </div>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 960px;
  min-height: 600px;
  color: var(--tmcl-text);
}

.nav-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 32px;
  margin: 0 4px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--tmcl-text);
  transition: background-color 83ms var(--tmcl-ease);
}

.nav-toggle:hover,
.nav-item:hover {
  background: var(--tmcl-subtle);
}

.nav-toggle:active,
.nav-item:active {
  background: var(--tmcl-subtle-pressed);
}

.app-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.nav-pane {
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  width: 240px;
  flex-shrink: 0;
  padding: 4px;
  transition: width 167ms var(--tmcl-ease);
}

.nav-pane.collapsed {
  width: 48px;
}

.collapsed .nav-item {
  padding: 0 10px;
}

.nav-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-footer {
  margin-top: auto;
}

.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 16px;
  height: 36px;
  padding: 0 12px;
  border-radius: 4px;
  color: var(--tmcl-text);
  text-decoration: none;
  white-space: nowrap;
  overflow: hidden;
  transition: background-color 83ms var(--tmcl-ease);
}

.nav-item.active {
  background: var(--tmcl-subtle);
}

/* Win11 选中指示条：左侧 3 px 宽的强调色圆角竖条。 */
.nav-item.active::before {
  content: "";
  position: absolute;
  left: 0;
  top: 50%;
  width: 3px;
  height: 16px;
  margin-top: -8px;
  border-radius: 2px;
  background: var(--tmcl-accent);
}

.nav-icon {
  flex-shrink: 0;
}

.nav-label {
  font-size: 14px;
}

.content-layer {
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding: 24px 36px;
  border-top: 1px solid var(--tmcl-stroke);
  border-left: 1px solid var(--tmcl-stroke);
  border-top-left-radius: 8px;
  background: var(--tmcl-layer);
}

.content-layer :deep(.page-title) {
  margin: 0 0 24px;
  font-size: 28px;
  font-weight: 600;
  line-height: 36px;
}
</style>
