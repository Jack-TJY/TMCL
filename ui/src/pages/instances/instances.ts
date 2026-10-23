// 实例页的纯函数：搜索、排序、分组与参数文本转换，便于单元测试。
import type { InstanceSummary } from "../../bindings/InstanceSummary";

/** 列表排序方式：`recent` 保持后端顺序（最近游玩在前），`name` 按名称。 */
export const INSTANCE_SORTS = ["recent", "name"] as const;
export type InstanceSort = (typeof INSTANCE_SORTS)[number];

/** 一个分组；`name` 为 `null` 表示未分组。 */
export interface InstanceGroup {
  name: string | null;
  instances: InstanceSummary[];
}

/** 按名称、版本或加载器搜索（不区分大小写）。 */
export function filterInstances(list: InstanceSummary[], query: string): InstanceSummary[] {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  return list.filter((i) =>
    [i.name, i.game_version, i.loader?.kind ?? "", i.group ?? ""].some((field) =>
      field.toLowerCase().includes(q),
    ),
  );
}

/** 排序；`recent` 依赖后端已按最近游玩、创建时间排好的顺序。 */
export function sortInstances(
  list: InstanceSummary[],
  sort: InstanceSort,
  locale?: string,
): InstanceSummary[] {
  if (sort === "recent") return list;
  return [...list].sort((a, b) => a.name.localeCompare(b.name, locale));
}

/** 按分组归类：有名称的分组按名称排序，未分组的放在最后；组内保持传入顺序。 */
export function groupInstances(list: InstanceSummary[], locale?: string): InstanceGroup[] {
  const groups = new Map<string | null, InstanceSummary[]>();
  for (const instance of list) {
    const name = instance.group?.trim() || null;
    const members = groups.get(name);
    if (members) members.push(instance);
    else groups.set(name, [instance]);
  }
  return [...groups.entries()]
    .map(([name, instances]) => ({ name, instances }))
    .sort((a, b) => {
      if (a.name === null) return 1;
      if (b.name === null) return -1;
      return a.name.localeCompare(b.name, locale);
    });
}
