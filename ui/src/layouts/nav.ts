// 左侧导航项（PLAN.md M1-11）：首页 / 实例 / 下载 / 资源（M2-06）/ 账号 / 皮肤 / 同步（M2-10）/ 联机 / 设置。
import {
  Accessibility24Filled,
  Accessibility24Regular,
  ArrowDownload24Filled,
  ArrowDownload24Regular,
  ArrowSync24Filled,
  ArrowSync24Regular,
  Cube24Filled,
  Cube24Regular,
  Home24Filled,
  Home24Regular,
  PeopleCommunity24Filled,
  PeopleCommunity24Regular,
  Person24Filled,
  Person24Regular,
  PuzzlePiece24Filled,
  PuzzlePiece24Regular,
  Settings24Filled,
  Settings24Regular,
} from "@vicons/fluent";
import type { Component } from "vue";

export interface NavItem {
  /** 路由名，同时作为菜单项的 key。 */
  name: string;
  /** 文案 key。 */
  labelKey: string;
  icon: Component;
  /** 选中时的实心图标（Win11 导航栏的做法）。 */
  activeIcon: Component;
}

/** 联机页（PLAN.md M2-15）。 */
export const MULTIPLAYER_ENABLED = true;

const item = (name: string, icon: Component, activeIcon: Component): NavItem => ({
  name,
  labelKey: `shell.nav.${name}`,
  icon,
  activeIcon,
});

/** 主导航，从上到下排列。 */
export const MAIN_NAV: NavItem[] = [
  item("home", Home24Regular, Home24Filled),
  item("instances", Cube24Regular, Cube24Filled),
  item("download", ArrowDownload24Regular, ArrowDownload24Filled),
  item("resources", PuzzlePiece24Regular, PuzzlePiece24Filled),
  item("accounts", Person24Regular, Person24Filled),
  item("skin", Accessibility24Regular, Accessibility24Filled),
  item("sync", ArrowSync24Regular, ArrowSync24Filled),
  ...(MULTIPLAYER_ENABLED
    ? [item("multiplayer", PeopleCommunity24Regular, PeopleCommunity24Filled)]
    : []),
];

/** 固定在导航底部的项。 */
export const FOOTER_NAV: NavItem[] = [item("settings", Settings24Regular, Settings24Filled)];
