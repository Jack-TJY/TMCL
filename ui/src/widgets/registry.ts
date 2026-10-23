// 主页组件登记表（AGENTS.md §5.2 共享文件）：一行登记一个组件，按组件 ID 字母排序。
// 新增组件：新建 `widgets/<id>/` 目录，在下面两处各加一行。
import account from "./account";
import quickActions from "./quick-actions";
import type { WidgetDefinition } from "./types";

// 保持一行一个组件，便于并行追加（prettier 会把短数组折成一行）。
// prettier-ignore
export const WIDGETS: readonly WidgetDefinition[] = [
  account,
  quickActions,
];

const byId = new Map(WIDGETS.map((w) => [w.id, w]));

/** 按 ID 查找已登记的组件。 */
export function findWidget(id: string): WidgetDefinition | undefined {
  return byId.get(id);
}
