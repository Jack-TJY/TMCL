// 资源浏览的选择在本次运行中保留（beta.1 B4）：离开资源页再回来时，恢复选中的实例、
// 资源种类与平台筛选。只存在内存里，重启应用后回到默认值。
import { defineStore } from "pinia";
import { ref } from "vue";
import type { ResourceKind, ResourcePlatform } from "../../api/resources";
import { BROWSER_KINDS } from "./targets";

/** 平台筛选：`all` 为合并显示。 */
export type PlatformFilter = ResourcePlatform | "all";

export const useResourceBrowseState = defineStore("resourceBrowse", () => {
  /** 资源下载页选中的目标实例。 */
  const instanceId = ref<string | null>(null);
  /** 资源下载页的资源种类。 */
  const kind = ref<ResourceKind>(BROWSER_KINDS[0]);
  /** 浏览面板的平台筛选，资源下载页与实例的资源页共用。 */
  const platform = ref<PlatformFilter>("all");
  return { instanceId, kind, platform };
});

/**
 * 资源下载页打开时的实例与种类：地址中给出的优先（如从实例页跳转过来），否则用上次的选择。
 * 地址中的种类不是该页的种类时忽略。
 */
export function initialTarget(
  query: { instance?: unknown; kind?: unknown },
  remembered: { instanceId: string | null; kind: ResourceKind },
): { instanceId: string | null; kind: ResourceKind } {
  return {
    instanceId: typeof query.instance === "string" ? query.instance : remembered.instanceId,
    kind: BROWSER_KINDS.find((k) => k === query.kind) ?? remembered.kind,
  };
}
