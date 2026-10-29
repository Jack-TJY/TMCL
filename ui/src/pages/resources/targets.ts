// 资源下载页（不绑定实例）的纯函数：目标实例与资源是否兼容、支持的游戏版本与加载器标签。
import type { ResourceHit, ResourceKind } from "../../api/resources";
import type { LoaderKind } from "../../bindings/LoaderKind";
import type { LoaderSpec } from "../../bindings/LoaderSpec";

/** 资源下载页的标签页；数据包要选存档，只在实例的资源管理里安装。 */
export const BROWSER_KINDS: readonly ResourceKind[] = ["mod", "resourcepack", "shader"];

/** 搜索结果分类中的 Mod 加载器，按新建实例时的推荐顺序。 */
const MOD_LOADERS: readonly LoaderKind[] = ["fabric", "neoforge", "forge", "quilt"];

/** 光影分类中的加载方式；`vanilla` 指原版核心着色器，不需要额外 Mod。 */
const SHADER_LOADERS = ["iris", "optifine", "canvas"] as const;

/** 加载器名称是专有名词，不翻译。 */
const NAMES: Record<string, string> = {
  fabric: "Fabric",
  quilt: "Quilt",
  forge: "Forge",
  neoforge: "NeoForge",
  iris: "Iris",
  optifine: "OptiFine",
  canvas: "Canvas",
};

/** 加载器的显示名称。 */
export function loaderName(kind: string): string {
  return NAMES[kind] ?? kind;
}

/** 搜索结果需要的加载器（Mod 为 Fabric / Forge 等，光影为 Iris / OptiFine 等）；资源包为空。 */
export function hitLoaders(hit: ResourceHit, kind: ResourceKind): string[] {
  const known: readonly string[] =
    kind === "mod" ? MOD_LOADERS : kind === "shader" ? SHADER_LOADERS : [];
  return known.filter((loader) => hit.categories.includes(loader));
}

const RELEASE = /^\d+\.\d+(\.\d+)?$/;

function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/** 正式版游戏版本（如 `1.20.1`、`26.3`），去重后新版本在前。 */
export function releaseVersions(versions: readonly string[]): string[] {
  return [...new Set(versions.filter((v) => RELEASE.test(v)))].sort((a, b) =>
    compareVersions(b, a),
  );
}

/** 支持的游戏版本摘要：最新的几个正式版与正式版总数；没有正式版时取前几个原始版本号。 */
export function versionSummary(
  versions: readonly string[],
  shown = 3,
): { latest: string[]; total: number } {
  const releases = releaseVersions(versions);
  const list = releases.length > 0 ? releases : [...versions];
  return { latest: list.slice(0, shown), total: list.length };
}

/** 资源不能装到实例上的原因；`null` 表示兼容。 */
export type Incompatibility =
  | { reason: "version"; version: string }
  | { reason: "noLoader" }
  | { reason: "loader"; loaders: string[] };

/** 实例中与兼容性判断有关的字段。 */
export interface TargetInstance {
  game_version: string;
  loader: LoaderSpec | null;
}

/**
 * 按搜索结果中的游戏版本与分类判断能否装到实例上：版本不在支持列表中、
 * Mod 装到原版实例、或 Mod 不支持实例的加载器（Quilt 实例同时接受 Fabric）。
 * 搜索结果没有版本或加载器信息时不据此拒绝，交给安装对话框按版本列表判断。
 */
export function incompatibility(
  hit: ResourceHit,
  kind: ResourceKind,
  instance: TargetInstance,
): Incompatibility | null {
  if (hit.game_versions.length > 0 && !hit.game_versions.includes(instance.game_version)) {
    return { reason: "version", version: instance.game_version };
  }
  if (kind !== "mod") return null;
  if (!instance.loader) return { reason: "noLoader" };
  const loaders = hitLoaders(hit, kind);
  const accepted =
    instance.loader.kind === "quilt" ? ["quilt", "fabric"] : [instance.loader.kind as string];
  if (loaders.length > 0 && !loaders.some((l) => accepted.includes(l))) {
    return { reason: "loader", loaders };
  }
  return null;
}
