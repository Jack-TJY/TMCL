// 组件共用：实例版本的显示文字，例如“1.20.1 · Fabric 0.16.5”。
import type { InstanceSummary } from "../bindings/InstanceSummary";

/** 加载器名称是专有名词，不翻译。 */
const LOADER_NAMES: Record<string, string> = {
  fabric: "Fabric",
  forge: "Forge",
  neoforge: "NeoForge",
  quilt: "Quilt",
};

export function versionLabel(instance: Pick<InstanceSummary, "game_version" | "loader">): string {
  const { game_version, loader } = instance;
  if (!loader) return game_version;
  return `${game_version} · ${LOADER_NAMES[loader.kind] ?? loader.kind} ${loader.version}`;
}
