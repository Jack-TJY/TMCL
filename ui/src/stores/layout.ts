// 主页布局（PLAN.md M1-18）：经 `layout_get` / `layout_save` 读写，编辑模式下的修改在点击“完成”时
// 才保存，取消（或按 Esc）时恢复进入编辑前的布局。未登记的组件不显示，但保存时原样写回。
import type { LayoutItem } from "grid-layout-plus";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { toAppError } from "../api/invoke";
import { layoutGet, layoutSave } from "../api/layout";
import type { AppError } from "../bindings/AppError";
import type { WidgetPlacement } from "../bindings/WidgetPlacement";
import { DEFAULT_LAYOUT } from "../widgets/defaults";
import { findWidget } from "../widgets/registry";
import {
  clampSize,
  GRID_COLUMNS,
  normalizeSettings,
  type WidgetDefinition,
  type WidgetSettings,
} from "../widgets/types";

/** 与后端 `MAX_WIDGETS` 一致。 */
export const MAX_WIDGETS = 64;

/** 网格中的一项：`i` 即组件实例的 `uid`。 */
export interface HomeItem extends LayoutItem {
  i: string;
  type: string;
  settings: WidgetSettings;
}

/** 读取布局时需要提示用户的情况。 */
export interface LoadNotice {
  error: AppError | null;
  backup: string | null;
  skipped: number;
}

type Find = (id: string) => WidgetDefinition | undefined;

/** 把保存的组件转成网格项：尺寸限制在组件约束内，设置按 schema 规整。 */
export function toItem(p: WidgetPlacement, def: WidgetDefinition): HomeItem {
  const size = clampSize(def, p);
  return {
    i: p.uid,
    type: p.type,
    x: Math.max(0, Math.min(p.x, GRID_COLUMNS - size.w)),
    y: p.y,
    ...size,
    minW: def.minSize.w,
    minH: def.minSize.h,
    maxW: def.maxSize?.w ?? GRID_COLUMNS,
    maxH: def.maxSize?.h ?? Infinity,
    settings: normalizeSettings(def.settingsSchema, p.settings),
  };
}

export function toPlacement(item: HomeItem): WidgetPlacement {
  const { i, type, x, y, w, h, settings } = item;
  return { uid: i, type, x, y, w, h, settings: { ...settings } };
}

/** 拆分为可显示的网格项与未登记的组件。 */
export function splitLayout(placements: readonly WidgetPlacement[], find: Find = findWidget) {
  const items: HomeItem[] = [];
  const hidden: WidgetPlacement[] = [];
  for (const p of placements) {
    const def = find(p.type);
    if (def) items.push(toItem(p, def));
    else hidden.push(p);
  }
  return { items, hidden };
}

type Rect = Pick<LayoutItem, "x" | "y" | "w" | "h">;

const overlaps = (a: Rect, b: Rect) =>
  a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** 按行优先找到第一个放得下 `w × h` 的空位。 */
export function findSlot(items: readonly Rect[], w: number, h: number): { x: number; y: number } {
  const bottom = items.reduce((max, r) => Math.max(max, r.y + r.h), 0);
  for (let y = 0; y <= bottom; y++) {
    for (let x = 0; x + w <= GRID_COLUMNS; x++) {
      if (!items.some((r) => overlaps(r, { x, y, w, h }))) return { x, y };
    }
  }
  return { x: 0, y: bottom };
}

/** 生成布局内唯一的组件实例 ID。 */
export function newUid(taken: ReadonlySet<string>): string {
  for (;;) {
    const uid = Math.random().toString(36).slice(2, 10);
    if (uid && !taken.has(uid)) return uid;
  }
}

const cloneItems = (items: readonly HomeItem[]) =>
  items.map((item) => ({ ...item, settings: { ...item.settings } }));

export const useLayoutStore = defineStore("layout", () => {
  const items = ref<HomeItem[]>([]);
  let hidden: WidgetPlacement[] = [];
  const loaded = ref(false);
  const editing = ref(false);
  const saving = ref(false);
  let before: { items: HomeItem[]; hidden: WidgetPlacement[] } | null = null;

  const canAdd = computed(() => items.value.length + hidden.length < MAX_WIDGETS);

  /** 默认布局中未登记的组件（尚未实现）直接丢弃；保存的布局中的则原样保留。 */
  function apply(placements: readonly WidgetPlacement[], keepUnknown: boolean) {
    const split = splitLayout(placements);
    items.value = split.items;
    hidden = keepUnknown ? split.hidden : [];
  }

  /** 读取保存的布局；读取失败或从未保存时使用默认布局。 */
  async function load(): Promise<LoadNotice> {
    const notice: LoadNotice = { error: null, backup: null, skipped: 0 };
    let placements: readonly WidgetPlacement[] | null = null;
    try {
      const snapshot = await layoutGet();
      placements = snapshot.widgets;
      notice.backup = snapshot.backup;
      notice.skipped = snapshot.skipped;
    } catch (err) {
      notice.error = toAppError(err);
    }
    if (placements) apply(placements, true);
    else apply(DEFAULT_LAYOUT, false);
    loaded.value = true;
    return notice;
  }

  function startEdit() {
    if (editing.value) return;
    before = { items: cloneItems(items.value), hidden: [...hidden] };
    editing.value = true;
  }

  /** 放弃本次编辑的全部修改。 */
  function cancelEdit() {
    if (!editing.value) return;
    if (before) ({ items: items.value, hidden } = before);
    before = null;
    editing.value = false;
  }

  /** 保存并退出编辑模式；失败时保持编辑状态并抛出 AppError。 */
  async function finishEdit() {
    saving.value = true;
    try {
      await layoutSave({ widgets: [...items.value.map(toPlacement), ...hidden] });
      before = null;
      editing.value = false;
    } catch (err) {
      throw toAppError(err);
    } finally {
      saving.value = false;
    }
  }

  /** 添加一个组件到第一个空位，返回新组件的 uid。 */
  function add(type: string): string | null {
    const def = findWidget(type);
    if (!def || !canAdd.value) return null;
    const uid = newUid(new Set([...items.value.map((i) => i.i), ...hidden.map((h) => h.uid)]));
    const { w, h } = def.defaultSize;
    const slot = findSlot(items.value, w, h);
    items.value.push(toItem({ uid, type, ...slot, w, h, settings: {} }, def));
    return uid;
  }

  function remove(uid: string) {
    items.value = items.value.filter((item) => item.i !== uid);
  }

  function setSettings(uid: string, settings: WidgetSettings) {
    const item = items.value.find((i) => i.i === uid);
    const def = item && findWidget(item.type);
    if (item && def) item.settings = normalizeSettings(def.settingsSchema, settings);
  }

  /** 换成默认布局（仍需点击“完成”才保存）。 */
  function resetToDefault() {
    apply(DEFAULT_LAYOUT, false);
  }

  return {
    items,
    loaded,
    editing,
    saving,
    canAdd,
    load,
    startEdit,
    cancelEdit,
    finishEdit,
    add,
    remove,
    setSettings,
    resetToDefault,
  };
});
