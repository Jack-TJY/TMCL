// 实例页的纯函数：搜索、排序、分组与参数文本转换，便于单元测试。
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import type { ResourceImportSkip } from "../../bindings/ResourceImportSkip";
import type { ResourceImportSkipReason } from "../../bindings/ResourceImportSkipReason";

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

/** 已有的分组名，供详情页选择。 */
export function groupNames(list: InstanceSummary[], locale?: string): string[] {
  const names = new Set(list.map((i) => i.group?.trim()).filter((g): g is string => !!g));
  return [...names].sort((a, b) => a.localeCompare(b, locale));
}

/** 最近可启动的实例（列表中第一个未损坏的实例），没有时为 `null`。 */
export function mostRecent(list: InstanceSummary[]): InstanceSummary | null {
  return list.find((i) => i.error === null) ?? null;
}

/**
 * 把一行参数文本拆成参数数组：按空白分隔，双引号内的空白保留，引号本身去掉。
 * 例如 `-Dfoo="a b" -Xss2m` → `["-Dfoo=a b", "-Xss2m"]`。
 */
export function parseArgs(text: string): string[] {
  const args: string[] = [];
  let current = "";
  let quoted = false;
  let started = false;
  for (const ch of text) {
    if (ch === '"') {
      quoted = !quoted;
      started = true;
    } else if (!quoted && /\s/.test(ch)) {
      if (started) args.push(current);
      current = "";
      started = false;
    } else {
      current += ch;
      started = true;
    }
  }
  if (started) args.push(current);
  return args;
}

/** `parseArgs` 的逆操作：含空白或为空的参数加双引号。 */
export function formatArgs(args: string[]): string {
  return args.map((a) => (a === "" || /\s/.test(a) ? `"${a}"` : a)).join(" ");
}

/** 复制实例时建议的新名称，例如 `生存` → `生存 (2)`，避开已有名称。 */
export function copyName(name: string, existing: string[]): string {
  const taken = new Set(existing);
  for (let n = 2; ; n += 1) {
    const candidate = `${name} (${n})`;
    if (!taken.has(candidate)) return candidate;
  }
}

/** 导出格式：TMCL 实例包，或 Modrinth 整合包。 */
export type PackFormat = "tmclpack" | "mrpack" | "curseforge";

/** 各格式的文件扩展名；CurseForge 整合包是普通 zip。 */
export const PACK_EXTENSIONS: Record<PackFormat, string> = {
  tmclpack: "tmclpack",
  mrpack: "mrpack",
  curseforge: "zip",
};

/** 默认不勾选的条目：日志与缓存通常没有分享价值；整合包还不带存档。 */
const UNCHECKED: Record<PackFormat, readonly string[]> = {
  tmclpack: ["logs", "crash-reports", ".cache", "screenshots"],
  mrpack: ["logs", "crash-reports", ".cache", "screenshots", "saves"],
  curseforge: ["logs", "crash-reports", ".cache", "screenshots", "saves"],
};

/** 导出时默认勾选的 `game/` 条目。 */
export function defaultInclude(entries: string[], format: PackFormat): string[] {
  return entries.filter((e) => !UNCHECKED[format].includes(e));
}

/** 导出文件名：去掉文件名中不允许的字符。 */
export function packFileName(name: string, format: PackFormat = "tmclpack"): string {
  const safe = name.replace(/[\\/:*?"<>|]/g, "_").trim();
  return `${safe || "instance"}.${PACK_EXTENSIONS[format]}`;
}

/** 把路径的 `.tmclpack` / `.mrpack` / `.zip` 扩展名换成 `format` 的；没有这些扩展名时追加。 */
export function withPackExtension(path: string, format: PackFormat): string {
  const base = path.replace(/\.(tmclpack|mrpack|zip)$/i, "");
  return base ? `${base}.${PACK_EXTENSIONS[format]}` : base;
}

/** 在目录后拼接文件名，沿用目录中的路径分隔符。 */
export function joinPath(dir: string, file: string): string {
  const sep = dir.includes("\\") ? "\\" : "/";
  return dir.endsWith(sep) ? `${dir}${file}` : `${dir}${sep}${file}`;
}

/** 装有加载器（Forge、NeoForge、Fabric、Quilt）且能正常读取的实例才能导入 Mod。 */
export function canImportMods(instance: Pick<InstanceSummary, "loader" | "error">): boolean {
  return instance.error === null && instance.loader !== null;
}

/** 把导入时跳过的文件按原因分组，保持出现顺序。 */
export function groupSkipped(
  skipped: ResourceImportSkip[],
): [ResourceImportSkipReason, string[]][] {
  const groups = new Map<ResourceImportSkipReason, string[]>();
  for (const s of skipped) {
    const names = groups.get(s.reason) ?? [];
    names.push(s.file_name);
    groups.set(s.reason, names);
  }
  return [...groups];
}
