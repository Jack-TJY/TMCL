// 设置同步页的纯逻辑：类别、手动编辑校验、预览汇总。
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import type { SyncCategory } from "../../bindings/SyncCategory";
import type { SyncOption } from "../../bindings/SyncOption";
import type { SyncPreview } from "../../bindings/SyncPreview";

/** 全部类别，按界面显示顺序。 */
export const SYNC_CATEGORIES: SyncCategory[] = ["keys", "video", "audio", "language", "other"];

/** 默认勾选的类别；“其他”包含大量与实例相关的杂项，默认不同步。 */
export const DEFAULT_CATEGORIES: SyncCategory[] = ["keys", "video", "audio", "language"];

/** 主配置最多包含的选项数，与后端 `MAX_OPTIONS` 一致。 */
export const MAX_OPTIONS = 4096;

/** 一行选项的校验问题。 */
export type RowIssue = "empty" | "colon" | "newline" | "duplicate";

/** 编辑器中的一行；`id` 只用于列表渲染。 */
export interface EditorRow {
  id: number;
  key: string;
  value: string;
}

let nextRowId = 0;

/** 把选项转为可编辑的行。 */
export function toRows(options: SyncOption[]): EditorRow[] {
  return options.map((o) => newRow(o.key, o.value));
}

export function newRow(key = "", value = ""): EditorRow {
  return { id: nextRowId++, key, value };
}

const hasNewline = (s: string) => /[\r\n]/.test(s);

/** 逐行校验，与后端 `sync_save_master` 的规则一致，另外标出重复的键（后写的会覆盖先写的）。 */
export function validateRows(rows: EditorRow[]): (RowIssue | null)[] {
  const seen = new Set<string>();
  return rows.map((row) => {
    const key = row.key.trim();
    if (key === "") return "empty";
    if (key.includes(":")) return "colon";
    if (hasNewline(row.value)) return "newline";
    if (seen.has(key)) return "duplicate";
    seen.add(key);
    return null;
  });
}

/** 行可以保存时返回选项（键去掉首尾空白），否则返回 `null`。 */
export function rowsToOptions(rows: EditorRow[]): SyncOption[] | null {
  if (rows.length > MAX_OPTIONS) return null;
  if (validateRows(rows).some((issue) => issue !== null)) return null;
  return rows.map((row) => ({ key: row.key.trim(), value: row.value }));
}

/** 按键、值或显示名称筛选行（忽略大小写）。 */
export function filterRows(
  rows: EditorRow[],
  query: string,
  labelOf: (key: string) => string | null = () => null,
): EditorRow[] {
  const q = query.trim().toLowerCase();
  if (!q) return rows;
  return rows.filter((r) =>
    [r.key, r.value, labelOf(r.key.trim()) ?? ""].some((s) => s.toLowerCase().includes(q)),
  );
}

/** 可作为来源或目标的实例：排除损坏的实例。 */
export function usableInstances(instances: InstanceSummary[]): InstanceSummary[] {
  return instances.filter((i) => !i.error);
}

/** 预览中的修改总数与需要修改的实例数。 */
export function summarizePreview(preview: SyncPreview): { changes: number; targets: number } {
  const changed = preview.targets.filter((t) => t.changes.length > 0);
  return {
    changes: changed.reduce((sum, t) => sum + t.changes.length, 0),
    targets: changed.length,
  };
}
