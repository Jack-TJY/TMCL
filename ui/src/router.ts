// 路由表（AGENTS.md §5.2 共享文件）：每个页面一个路由块，按路径字母排序。
// 页面尚未实现时使用 PlaceholderPage，对应步骤把 component 换成自己的页面即可。
import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";
import PlaceholderPage from "./layouts/PlaceholderPage.vue";
import DownloadPage from "./pages/download/DownloadPage.vue";
import SettingsPage from "./pages/settings/SettingsPage.vue";

declare module "vue-router" {
  interface RouteMeta {
    /** 页面标题的 i18n key。 */
    titleKey?: string;
  }
}

const routes: RouteRecordRaw[] = [
  { path: "/", redirect: "/home" },
  {
    path: "/accounts",
    name: "accounts",
    component: PlaceholderPage,
    meta: { titleKey: "shell.nav.accounts" },
  },
  {
    path: "/download",
    name: "download",
    component: DownloadPage,
    meta: { titleKey: "shell.nav.download" },
  },
  {
    path: "/home",
    name: "home",
    component: PlaceholderPage,
    meta: { titleKey: "shell.nav.home" },
  },
  {
    path: "/instances",
    name: "instances",
    component: PlaceholderPage,
    meta: { titleKey: "shell.nav.instances" },
  },
  {
    path: "/multiplayer",
    name: "multiplayer",
    component: PlaceholderPage,
    meta: { titleKey: "shell.nav.multiplayer" },
  },
  {
    path: "/settings",
    name: "settings",
    component: SettingsPage,
    meta: { titleKey: "shell.nav.settings" },
  },
  { path: "/:pathMatch(.*)*", redirect: "/home" },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
