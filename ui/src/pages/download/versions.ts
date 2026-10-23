// 版本列表的筛选与新实例的默认名称。
import type { GameVersion } from "../../bindings/GameVersion";

/** 版本类型筛选：正式版、快照、远古版本（old_beta / old_alpha）。 */
export type VersionFilter = "release" | "snapshot" | "old";

export const VERSION_FILTERS: VersionFilter[] = ["release", "snapshot", "old"];

/** 版本属于哪个筛选；未知类型（`other`）归入快照。 */
export function filterOf(version: GameVersion): VersionFilter {
  if (version.kind === "release") return "release";
  if (version.kind === "old_beta" || version.kind === "old_alpha") return "old";
  return "snapshot";
}

/** 按类型与搜索词（不区分大小写的子串）筛选，保持原有顺序。 */
export function filterVersions(
  versions: GameVersion[],
  filter: VersionFilter,
  query: string,
): GameVersion[] {
  const needle = query.trim().toLowerCase();
  return versions.filter(
    (v) => filterOf(v) === filter && (!needle || v.id.toLowerCase().includes(needle)),
  );
}

/** 新实例的默认名称：版本号；与已有实例重名时加序号，例如 `1.20.1 (2)`。 */
export function defaultInstanceName(versionId: string, existing: readonly string[]): string {
  const taken = new Set(existing);
  if (!taken.has(versionId)) return versionId;
  for (let n = 2; ; n += 1) {
    const name = `${versionId} (${n})`;
    if (!taken.has(name)) return name;
  }
}
