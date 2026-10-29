// 路由表（AGENTS.md §5.2 共享文件）：每个页面一个路由块，按路径字母排序。
// 页面尚未实现时使用 PlaceholderPage，对应步骤把 component 换成自己的页面即可。
import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";
import PlaceholderPage from "./layouts/PlaceholderPage.vue";
import AboutPage from "./pages/about/AboutPage.vue";
import AccountsPage from "./pages/accounts/AccountsPage.vue";
import DownloadPage from "./pages/download/DownloadPage.vue";
import HomePage from "./pages/home/HomePage.vue";
import InstancesPage from "./pages/instances/InstancesPage.vue";
import ResourceBrowserPage from "./pages/resources/ResourceBrowserPage.vue";
import ResourcesPage from "./pages/resources/ResourcesPage.vue";
import SettingsPage from "./pages/settings/SettingsPage.vue";
import SkinPage from "./pages/skin/SkinPage.vue";
import SyncPage from "./pages/sync/SyncPage.vue";

declare module "vue-router" {
  interface RouteMeta {
    /** 页面标题的 i18n key。 */
    titleKey?: string;
  }
}

const routes: RouteRecordRaw[] = [
  { path: "/", redirect: "/home" },
  {
    path: "/about",
    name: "about",
    component: AboutPage,
    meta: { titleKey: "about.title" },
  },
  {
    path: "/accounts",
    name: "accounts",
    component: AccountsPage,
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
    component: HomePage,
    meta: { titleKey: "shell.nav.home" },
  },
  {
    path: "/instances",
    name: "instances",
    component: InstancesPage,
    meta: { titleKey: "shell.nav.instances" },
  },
  {
    path: "/instances/:id/resources",
    name: "instance-resources",
    component: ResourcesPage,
    meta: { titleKey: "resources.title" },
  },
  {
    path: "/multiplayer",
    name: "multiplayer",
    component: PlaceholderPage,
    meta: { titleKey: "shell.nav.multiplayer" },
  },
  {
    path: "/resources",
    name: "resources",
    component: ResourceBrowserPage,
    meta: { titleKey: "shell.nav.resources" },
  },
  {
    path: "/settings",
    name: "settings",
    component: SettingsPage,
    meta: { titleKey: "shell.nav.settings" },
  },
  {
    path: "/skin",
    name: "skin",
    component: SkinPage,
    meta: { titleKey: "shell.nav.skin" },
  },
  {
    path: "/sync",
    name: "sync",
    component: SyncPage,
    meta: { titleKey: "sync.title" },
  },
  { path: "/:pathMatch(.*)*", redirect: "/home" },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
