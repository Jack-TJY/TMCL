// Windows 11（Fluent）风格主题（PLAN.md M1-11）：用 Naive UI 的 themeOverrides 统一圆角、字体、配色，
// 并导出布局使用的 CSS 变量。亮/暗两套颜色取自 Win11 的 Mica 回退底色与卡片层颜色。
import type { GlobalThemeOverrides } from "naive-ui";

/** 取不到系统强调色时使用的 Win11 默认蓝。 */
export const DEFAULT_ACCENT = "#0067C0";

/** 系统字体栈：Win11 用 Segoe UI Variable，中文回退微软雅黑 / 苹方。 */
export const FONT_FAMILY =
  '"Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI", system-ui, -apple-system, ' +
  '"Microsoft YaHei UI", "Microsoft YaHei", "PingFang SC", "Noto Sans CJK SC", sans-serif';

/** 柔和的悬停与按下过渡，接近 Win11 控件的动画曲线。 */
const EASE = "cubic-bezier(0, 0, 0, 1)";

interface Palette {
  /** 窗口底色（无窗口材质时）。 */
  background: string;
  /** 内容区图层：叠在窗口底色上的半透明层。 */
  layer: string;
  /** 卡片、输入框等控件底色。 */
  card: string;
  /** 输入框、下拉框等控件底色。 */
  control: string;
  /** 弹出层与对话框底色。 */
  popover: string;
  /** 1 px 半透明描边。 */
  stroke: string;
  /** 侧边栏与列表项悬停底色。 */
  subtle: string;
  /** 侧边栏与列表项按下底色。 */
  subtlePressed: string;
  textPrimary: string;
  textSecondary: string;
  textDisabled: string;
  /** 卡片轻阴影。 */
  shadow: string;
}

const LIGHT: Palette = {
  background: "#F3F3F3",
  layer: "rgba(255, 255, 255, 0.5)",
  card: "#FFFFFF",
  control: "rgba(255, 255, 255, 0.7)",
  popover: "#F9F9F9",
  stroke: "rgba(0, 0, 0, 0.0578)",
  subtle: "rgba(0, 0, 0, 0.0373)",
  subtlePressed: "rgba(0, 0, 0, 0.0241)",
  textPrimary: "rgba(0, 0, 0, 0.896)",
  textSecondary: "rgba(0, 0, 0, 0.606)",
  textDisabled: "rgba(0, 0, 0, 0.361)",
  shadow: "0 2px 4px rgba(0, 0, 0, 0.04)",
};

const DARK: Palette = {
  background: "#202020",
  layer: "rgba(58, 58, 58, 0.3)",
  card: "#2B2B2B",
  control: "rgba(255, 255, 255, 0.0605)",
  popover: "#2C2C2C",
  stroke: "rgba(255, 255, 255, 0.0698)",
  subtle: "rgba(255, 255, 255, 0.0605)",
  subtlePressed: "rgba(255, 255, 255, 0.0419)",
  textPrimary: "#FFFFFF",
  textSecondary: "rgba(255, 255, 255, 0.786)",
  textDisabled: "rgba(255, 255, 255, 0.363)",
  shadow: "0 2px 4px rgba(0, 0, 0, 0.26)",
};

/** 解析 `#RRGGBB` 或 `rgb(r, g, b)`；无法解析时返回 `null`。 */
export function parseColor(value: string): [number, number, number] | null {
  const hex = value.trim().match(/^#([0-9a-f]{6})$/i);
  if (hex) {
    const n = parseInt(hex[1], 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  }
  const rgb = value.match(/^rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/i);
  return rgb ? [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])] : null;
}

/** 判断是否为 `#RRGGBB` 形式的颜色。 */
export function isHexColor(value: string): boolean {
  return /^#[0-9a-f]{6}$/i.test(value);
}

function toHex([r, g, b]: [number, number, number]): string {
  return `#${[r, g, b].map((c) => Math.round(c).toString(16).padStart(2, "0")).join("")}`.toUpperCase();
}

/** 把颜色向 `target` 混合 `amount`（0–1）。 */
export function mix(color: string, target: string, amount: number): string {
  const a = parseColor(color) ?? parseColor(DEFAULT_ACCENT)!;
  const b = parseColor(target)!;
  return toHex([0, 1, 2].map((i) => a[i] + (b[i] - a[i]) * amount) as [number, number, number]);
}

/**
 * 读取系统强调色：WebView 支持 CSS 系统颜色 `AccentColor` 时取其计算值，否则返回 `null`。
 */
export function systemAccent(): string | null {
  if (typeof document === "undefined" || !globalThis.CSS?.supports("color", "AccentColor")) {
    return null;
  }
  const probe = document.createElement("span");
  probe.style.color = "AccentColor";
  probe.style.display = "none";
  document.body.appendChild(probe);
  const rgb = parseColor(getComputedStyle(probe).color);
  probe.remove();
  return rgb ? toHex(rgb) : null;
}

/** 暗色主题下 Win11 使用更亮的强调色变体，保证在深色底上的对比度。 */
function accentFor(accent: string, dark: boolean) {
  const base = dark ? mix(accent, "#FFFFFF", 0.4) : accent;
  const toward = dark ? "#000000" : "#FFFFFF";
  return {
    base,
    hover: mix(base, toward, 0.1),
    pressed: mix(base, toward, 0.2),
    // 强调色按钮上的文字：暗色主题下强调色较亮，用黑字。
    onAccent: dark ? "#000000" : "#FFFFFF",
  };
}

/** 生成 Naive UI 的主题覆盖。 */
export function fluentOverrides(accent: string, dark: boolean): GlobalThemeOverrides {
  const p = dark ? DARK : LIGHT;
  const a = accentFor(accent, dark);
  return {
    common: {
      fontFamily: FONT_FAMILY,
      fontSize: "14px",
      fontSizeMedium: "14px",
      borderRadius: "4px",
      borderRadiusSmall: "4px",
      cubicBezierEaseInOut: EASE,
      cubicBezierEaseOut: EASE,
      primaryColor: a.base,
      primaryColorHover: a.hover,
      primaryColorPressed: a.pressed,
      primaryColorSuppl: a.hover,
      bodyColor: p.background,
      cardColor: p.card,
      modalColor: p.popover,
      popoverColor: p.popover,
      tableColor: p.card,
      inputColor: p.control,
      textColorBase: p.textPrimary,
      textColor1: p.textPrimary,
      textColor2: p.textPrimary,
      textColor3: p.textSecondary,
      textColorDisabled: p.textDisabled,
      placeholderColor: p.textSecondary,
      borderColor: p.stroke,
      dividerColor: p.stroke,
      hoverColor: p.subtle,
      pressedColor: p.subtlePressed,
      boxShadow1: p.shadow,
    },
    Button: {
      textColorPrimary: a.onAccent,
      textColorHoverPrimary: a.onAccent,
      textColorPressedPrimary: a.onAccent,
      textColorFocusPrimary: a.onAccent,
    },
    Card: {
      borderRadius: "8px",
      borderColor: p.stroke,
      color: p.card,
    },
    Dialog: { borderRadius: "8px" },
    Modal: { boxShadow: p.shadow },
    Menu: {
      borderRadius: "4px",
      itemHeight: "36px",
      itemColorHover: p.subtle,
      itemColorActive: p.subtle,
      itemColorActiveHover: p.subtle,
      itemColorActiveCollapsed: p.subtle,
      itemTextColorActive: p.textPrimary,
      itemTextColorActiveHover: p.textPrimary,
      itemIconColorActive: p.textPrimary,
      itemIconColorActiveHover: p.textPrimary,
      itemIconColorCollapsed: p.textPrimary,
    },
  };
}

/** 布局使用的 CSS 变量，挂在根元素上。 */
export function fluentCssVars(accent: string, dark: boolean): Record<string, string> {
  const p = dark ? DARK : LIGHT;
  return {
    "--tmcl-bg": p.background,
    "--tmcl-layer": p.layer,
    "--tmcl-card": p.card,
    "--tmcl-stroke": p.stroke,
    "--tmcl-subtle": p.subtle,
    "--tmcl-subtle-pressed": p.subtlePressed,
    "--tmcl-text": p.textPrimary,
    "--tmcl-text-secondary": p.textSecondary,
    "--tmcl-accent": accentFor(accent, dark).base,
    "--tmcl-shadow": p.shadow,
    "--tmcl-font": FONT_FAMILY,
    "--tmcl-ease": EASE,
  };
}
