// 主页组件契约（docs/contracts/widgets.md）：组件定义、设置项与组件收到的 props。
import type { Component } from "vue";

/** 栅格列数。 */
export const GRID_COLUMNS = 12;

/** 以栅格格数计的尺寸。 */
export interface WidgetSize {
  w: number;
  h: number;
}

interface FieldBase {
  key: string;
  labelKey: string;
}

/** 一个设置项，由外框统一生成设置面板。 */
export type WidgetSettingField =
  | (FieldBase & { type: "boolean"; default: boolean })
  | (FieldBase & { type: "number"; default: number; min: number; max: number; step?: number })
  | (FieldBase & {
      type: "select";
      default: string;
      options: { value: string; labelKey: string }[];
    })
  /** 实例 ID；`null` 表示“最近游玩的实例”。 */
  | (FieldBase & { type: "instance"; default: null });

export type WidgetSettings = Record<string, unknown>;

/** 组件收到的 props。 */
export interface WidgetProps {
  uid: string;
  /** 已按 schema 补齐默认值并修正越界值。 */
  settings: WidgetSettings;
  size: WidgetSize;
  /** 编辑模式下组件不应响应点击。 */
  editing: boolean;
}

export interface WidgetDefinition {
  /** 小写字母、数字、`-`，与目录名一致。 */
  id: string;
  titleKey: string;
  descriptionKey: string;
  icon: Component;
  defaultSize: WidgetSize;
  minSize: WidgetSize;
  /** 省略时不限制。 */
  maxSize?: WidgetSize;
  settingsSchema?: WidgetSettingField[];
  /** 异步加载组件，只有布局中出现时才加载。 */
  component: () => Promise<{ default: Component }>;
}

const WIDGET_ID = /^[a-z0-9-]{1,64}$/;

/** 声明一个组件；ID 或尺寸不合法时在开发期直接抛错。 */
export function defineWidget(def: WidgetDefinition): WidgetDefinition {
  const max = def.maxSize ?? { w: GRID_COLUMNS, h: Infinity };
  const ordered = (a: WidgetSize, b: WidgetSize) => a.w <= b.w && a.h <= b.h;
  if (
    !WIDGET_ID.test(def.id) ||
    def.minSize.w < 1 ||
    def.minSize.h < 1 ||
    max.w > GRID_COLUMNS ||
    !ordered(def.minSize, def.defaultSize) ||
    !ordered(def.defaultSize, max)
  ) {
    throw new Error(`invalid widget definition: ${def.id}`);
  }
  return def;
}

/** 把尺寸限制在组件的 `minSize` / `maxSize` 内，并保证不超出栅格右边界。 */
export function clampSize(def: WidgetDefinition, size: WidgetSize, x = 0): WidgetSize {
  const maxW = Math.min(def.maxSize?.w ?? GRID_COLUMNS, GRID_COLUMNS - x);
  const maxH = def.maxSize?.h ?? Infinity;
  return {
    w: Math.max(Math.min(size.w, maxW), Math.min(def.minSize.w, GRID_COLUMNS)),
    h: Math.max(Math.min(size.h, maxH), def.minSize.h),
  };
}

function isValid(field: WidgetSettingField, value: unknown): boolean {
  switch (field.type) {
    case "boolean":
      return typeof value === "boolean";
    case "number":
      return typeof value === "number" && Number.isFinite(value);
    case "select":
      return field.options.some((o) => o.value === value);
    case "instance":
      return value === null || (typeof value === "string" && value.length > 0);
  }
}

/** 按 schema 规整设置：补齐默认值、丢弃未知 key、数字限制在范围内。 */
export function normalizeSettings(
  schema: readonly WidgetSettingField[] | undefined,
  raw: WidgetSettings | undefined,
): WidgetSettings {
  const out: WidgetSettings = {};
  for (const field of schema ?? []) {
    const value = raw?.[field.key];
    let next = isValid(field, value) ? value : field.default;
    if (field.type === "number") {
      next = Math.min(Math.max(next as number, field.min), field.max);
    }
    out[field.key] = next;
  }
  return out;
}
