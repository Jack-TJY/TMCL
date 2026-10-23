// `layout_*` 命令封装：主页组件布局（PLAN.md M1-18）。
import type { LayoutSaveArgs } from "../bindings/LayoutSaveArgs";
import type { LayoutSnapshot } from "../bindings/LayoutSnapshot";
import { invoke } from "./invoke";

/** 读取主页布局；从未保存过时 `widgets` 为 `null`。 */
export function layoutGet(): Promise<LayoutSnapshot> {
  return invoke<LayoutSnapshot>("layout_get");
}

/** 整体保存主页布局。 */
export function layoutSave(args: LayoutSaveArgs): Promise<void> {
  return invoke<void>("layout_save", { args });
}
