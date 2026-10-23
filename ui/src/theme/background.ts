// 窗口背景与透明度（PLAN.md M1-19）：把 `appearance.background` 换算为背景图层样式与卡片 CSS 变量。
// 背景由 components/app-background/ 绘制；窗口材质生效时不画背景，只调整卡片透明度。
import type { BackgroundConfig } from "../bindings/BackgroundConfig";
import type { BackgroundFit } from "../bindings/BackgroundFit";
import darkUrl from "../assets/backgrounds/dark.svg";
import lightUrl from "../assets/backgrounds/light.svg";
import { parseColor } from "./fluent";

/** 与后端 `BackgroundConfig::default()` 一致。 */
export const DEFAULT_BACKGROUND: BackgroundConfig = {
  kind: "default",
  color: "#1F4E79",
  image: null,
  fit: "fill",
  opacity: 100,
  blur: 0,
  mask: 0,
  card_opacity: 85,
};

/** 背景模糊半径上限（px），与后端一致。 */
export const MAX_BLUR = 40;

/** 卡片不透明度低于此值（%）时给文字加描边阴影，保证可读性。 */
export const READABLE_THRESHOLD = 60;

/** 内置背景图。 */
export function defaultBackgroundUrl(dark: boolean): string {
  return dark ? darkUrl : lightUrl;
}

/** 图片填充方式对应的 CSS。 */
export function fitStyle(fit: BackgroundFit): Record<string, string> {
  switch (fit) {
    case "fill":
      return {
        backgroundSize: "cover",
        backgroundRepeat: "no-repeat",
        backgroundPosition: "center",
      };
    case "fit":
      return {
        backgroundSize: "contain",
        backgroundRepeat: "no-repeat",
        backgroundPosition: "center",
      };
    case "center":
      return {
        backgroundSize: "auto",
        backgroundRepeat: "no-repeat",
        backgroundPosition: "center",
      };
    case "tile":
      return { backgroundSize: "auto", backgroundRepeat: "repeat", backgroundPosition: "0 0" };
  }
}

/** 遮罩颜色：负数叠黑、正数叠白；为 0 时返回 `null`（不画遮罩）。 */
export function maskColor(mask: number): string | null {
  if (mask === 0) return null;
  const alpha = Math.min(Math.abs(mask), 100) / 100;
  return mask < 0 ? `rgba(0, 0, 0, ${alpha})` : `rgba(255, 255, 255, ${alpha})`;
}

/** 读取颜色的透明度：`rgba(...)` 取第四个分量，其余视为不透明。 */
function alphaOf(color: string): number {
  const match = color.match(/^rgba\([^)]*,\s*([\d.]+)\s*\)$/i);
  return match ? Number(match[1]) : 1;
}

/** 把颜色的透明度乘以 `factor`（0–1）。 */
export function scaleAlpha(color: string, factor: number): string {
  const rgb = parseColor(color);
  if (!rgb) return color;
  const alpha = Math.round(alphaOf(color) * factor * 1000) / 1000;
  return `rgba(${rgb.join(", ")}, ${alpha})`;
}

/**
 * 按卡片不透明度覆盖主题变量：卡片与内容区底色变为半透明，并给出毛玻璃模糊半径。
 * `themeVars` 为 `fluentCssVars()` 的结果。
 */
export function cardVars(
  themeVars: Record<string, string>,
  cardOpacity: number,
): Record<string, string> {
  const factor = Math.min(Math.max(cardOpacity, 0), 100) / 100;
  return {
    "--tmcl-card": scaleAlpha(themeVars["--tmcl-card"], factor),
    "--tmcl-layer": scaleAlpha(themeVars["--tmcl-layer"], factor),
    "--tmcl-card-blur": factor < 1 ? "20px" : "0px",
  };
}

/** 是否需要给文字加描边阴影。 */
export function needsTextShadow(cardOpacity: number): boolean {
  return cardOpacity < READABLE_THRESHOLD;
}

/** 背景来源（设置页的单选项）：窗口材质与 `background.kind` 合并为一个选择。 */
export type BackgroundSource = BackgroundConfig["kind"] | "material";

/** 当前实际使用的背景来源；平台不支持材质时回退为 `background.kind`。 */
export function backgroundSource(
  windowMaterial: boolean,
  materialSupported: boolean,
  kind: BackgroundConfig["kind"],
): BackgroundSource {
  return windowMaterial && materialSupported ? "material" : kind;
}
