import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import type { WidgetPlacement } from "../bindings/WidgetPlacement";
import { defineWidget, normalizeSettings, type WidgetSettingField } from "../widgets/types";
import { findSlot, splitLayout, toItem, useLayoutStore } from "./layout";

const def = defineWidget({
  id: "demo",
  titleKey: "widget-demo.title",
  descriptionKey: "widget-demo.description",
  icon: {},
  defaultSize: { w: 4, h: 2 },
  minSize: { w: 2, h: 2 },
  maxSize: { w: 6, h: 4 },
  component: () => Promise.resolve({ default: {} }),
});

const place = (uid: string, type: string, x = 0, y = 0, w = 4, h = 2): WidgetPlacement => ({
  uid,
  type,
  x,
  y,
  w,
  h,
  settings: {},
});

describe("defineWidget", () => {
  it("rejects bad ids and sizes", () => {
    expect(() => defineWidget({ ...def, id: "Bad" })).toThrow();
    expect(() => defineWidget({ ...def, minSize: { w: 5, h: 2 } })).toThrow();
    expect(() => defineWidget({ ...def, maxSize: { w: 13, h: 4 } })).toThrow();
  });
});

describe("normalizeSettings", () => {
  const schema: WidgetSettingField[] = [
    { key: "on", type: "boolean", labelKey: "x", default: true },
    { key: "n", type: "number", labelKey: "x", default: 5, min: 1, max: 10 },
    {
      key: "s",
      type: "select",
      labelKey: "x",
      default: "a",
      options: [
        { value: "a", labelKey: "x" },
        { value: "b", labelKey: "x" },
      ],
    },
    { key: "i", type: "instance", labelKey: "x", default: null },
  ];

  it("fills defaults, clamps numbers and drops unknown keys", () => {
    expect(normalizeSettings(schema, { on: "yes", n: 99, s: "c", i: "", extra: 1 })).toEqual({
      on: true,
      n: 10,
      s: "a",
      i: null,
    });
    expect(normalizeSettings(schema, { on: false, n: 3, s: "b", i: "abc" })).toEqual({
      on: false,
      n: 3,
      s: "b",
      i: "abc",
    });
  });
});

describe("layout helpers", () => {
  it("clamps size and keeps the widget inside the grid", () => {
    const item = toItem(place("a", "demo", 10, 3, 12, 1), def);
    expect(item).toMatchObject({ i: "a", x: 6, y: 3, w: 6, h: 2, minW: 2, maxH: 4 });
  });

  it("hides unregistered widgets", () => {
    const find = (id: string) => (id === "demo" ? def : undefined);
    const { items, hidden } = splitLayout([place("a", "demo"), place("b", "future")], find);
    expect(items.map((i) => i.i)).toEqual(["a"]);
    expect(hidden.map((h) => h.uid)).toEqual(["b"]);
  });

  it("finds the first free slot row by row", () => {
    expect(findSlot([], 4, 2)).toEqual({ x: 0, y: 0 });
    const items = [
      { x: 0, y: 0, w: 8, h: 2 },
      { x: 0, y: 2, w: 12, h: 2 },
    ];
    expect(findSlot(items, 4, 2)).toEqual({ x: 8, y: 0 });
    expect(findSlot(items, 6, 2)).toEqual({ x: 0, y: 4 });
  });
});

describe("layout store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    tauriInvoke.mockReset();
  });

  it("uses the default layout when nothing is saved", async () => {
    tauriInvoke.mockResolvedValueOnce({ widgets: null, skipped: 0, backup: null });
    const store = useLayoutStore();
    const notice = await store.load();
    expect(notice).toEqual({ error: null, backup: null, skipped: 0 });
    expect(store.items.length).toBeGreaterThan(0);
    // 默认布局中尚未登记的组件不会被写入布局文件。
    store.startEdit();
    tauriInvoke.mockResolvedValueOnce(null);
    await store.finishEdit();
    const saved = tauriInvoke.mock.lastCall?.[1].args.widgets as WidgetPlacement[];
    expect(saved.map((w) => w.uid)).toEqual(store.items.map((i) => i.i));
  });

  it("falls back to the default layout when loading fails", async () => {
    tauriInvoke.mockRejectedValueOnce({ code: "COMMON_IO", params: {}, detail: null });
    const store = useLayoutStore();
    expect((await store.load()).error?.code).toBe("COMMON_IO");
    expect(store.loaded).toBe(true);
  });

  it("cancels edits and saves unknown widgets back", async () => {
    const future = place("f", "future-widget", 0, 9);
    tauriInvoke.mockResolvedValueOnce({
      widgets: [place("q", "quick-actions", 0, 0, 4, 2), future],
      skipped: 0,
      backup: null,
    });
    const store = useLayoutStore();
    await store.load();
    expect(store.items.map((i) => i.i)).toEqual(["q"]);

    store.startEdit();
    store.remove("q");
    const uid = store.add("quick-actions");
    expect(store.items.map((i) => i.i)).toEqual([uid]);
    store.cancelEdit();
    expect(store.editing).toBe(false);
    expect(store.items.map((i) => i.i)).toEqual(["q"]);

    store.startEdit();
    store.setSettings("q", { labels: false, junk: 1 });
    tauriInvoke.mockResolvedValueOnce(null);
    await store.finishEdit();
    expect(store.editing).toBe(false);
    expect(tauriInvoke).toHaveBeenLastCalledWith("layout_save", {
      args: {
        widgets: [
          { uid: "q", type: "quick-actions", x: 0, y: 0, w: 4, h: 2, settings: { labels: false } },
          future,
        ],
      },
    });
  });

  it("stays in edit mode when saving fails", async () => {
    tauriInvoke.mockResolvedValueOnce({ widgets: [], skipped: 0, backup: null });
    const store = useLayoutStore();
    await store.load();
    store.startEdit();
    store.add("quick-actions");
    tauriInvoke.mockRejectedValueOnce({ code: "COMMON_IO", params: {}, detail: null });
    await expect(store.finishEdit()).rejects.toMatchObject({ code: "COMMON_IO" });
    expect(store.editing).toBe(true);
    expect(store.items).toHaveLength(1);
  });
});
