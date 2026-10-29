// 资源页的纯函数：可用的资源种类、版本选择、计划判断与列表过滤，便于单元测试。
import type {
  InstallPlan,
  InstalledResource,
  ResourceKind,
  ResourceSort,
  ResourceVersion,
} from "../../api/resources";
import type { LoaderSpec } from "../../bindings/LoaderSpec";

/** 标签页顺序。 */
export const RESOURCE_KINDS: readonly ResourceKind[] = [
  "mod",
  "resourcepack",
  "shader",
  "datapack",
];

/** 搜索排序选项。 */
export const RESOURCE_SORTS: readonly ResourceSort[] = [
  "relevance",
  "downloads",
  "follows",
  "newest",
  "updated",
];

/** 每页搜索结果条数。 */
export const PAGE_SIZE = 20;

/** 实例能装的资源种类：原版实例不能装 Mod。 */
export function kindsFor(loader: LoaderSpec | null): ResourceKind[] {
  return RESOURCE_KINDS.filter((kind) => kind !== "mod" || loader !== null);
}

/** 默认选中的版本：最新的正式版，没有正式版时取最新版本。 */
export function defaultVersion(versions: ResourceVersion[]): ResourceVersion | null {
  return versions.find((v) => v.channel === "release") ?? versions[0] ?? null;
}

/** 计划中有冲突或缺少前置时不能安装。 */
export function planBlocked(plan: InstallPlan): boolean {
  return plan.conflicts.length > 0 || plan.unavailable.length > 0;
}

/** 计划要下载的总字节数。 */
export function planSize(plan: InstallPlan): number {
  return plan.files.reduce((sum, file) => sum + file.size, 0);
}

/** 已安装资源的显示名称：Modrinth 标题，未知来源时为去掉 `.disabled` 的文件名。 */
export function displayName(resource: InstalledResource): string {
  return resource.source?.title ?? resource.file_name.replace(/\.disabled$/, "");
}

/** 按名称或文件名过滤（不区分大小写）。 */
export function filterInstalled(list: InstalledResource[], query: string): InstalledResource[] {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  return list.filter((r) =>
    [displayName(r), r.file_name].some((field) => field.toLowerCase().includes(q)),
  );
}

/** 字节数的可读形式，例如 `1.5 MB`。 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

/** 下载量的紧凑形式，例如 `1.2M`、`35K`。 */
export function formatCount(count: number, locale?: string): string {
  return new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 1 }).format(
    count,
  );
}
