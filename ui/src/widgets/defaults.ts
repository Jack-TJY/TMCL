// 默认主页布局：首次启动、布局文件损坏或点击“恢复默认布局”时使用。
// 未在 registry.ts 登记的组件会被跳过。
import type { WidgetPlacement } from "../bindings/WidgetPlacement";

export const DEFAULT_LAYOUT: readonly WidgetPlacement[] = [
  { uid: "start-game", type: "start-game", x: 0, y: 0, w: 8, h: 4, settings: {} },
  { uid: "account", type: "account", x: 8, y: 0, w: 4, h: 2, settings: {} },
  { uid: "quick-actions", type: "quick-actions", x: 8, y: 2, w: 4, h: 2, settings: {} },
  { uid: "instances", type: "instances", x: 0, y: 4, w: 6, h: 4, settings: {} },
  { uid: "downloads", type: "downloads", x: 6, y: 4, w: 6, h: 4, settings: {} },
  { uid: "news", type: "news", x: 0, y: 8, w: 12, h: 3, settings: {} },
];
