// 加载器选择组件的纯逻辑：加载状态、默认版本与可见版本，便于单元测试。
import type { AppError } from "../../bindings/AppError";
import type { LoaderKind } from "../../bindings/LoaderKind";
import type { LoaderVersion } from "../../bindings/LoaderVersion";

/** 界面中的加载器顺序。 */
export const LOADER_KINDS: readonly LoaderKind[] = ["fabric", "quilt", "forge", "neoforge"];

/** 版本列表经 BMCLAPI 镜像获取的加载器（Quilt 未被收录，始终走官方源）。 */
export const MIRRORED_KINDS: readonly LoaderKind[] = ["fabric", "forge", "neoforge"];

/** 某个加载器在当前游戏版本上的版本列表状态。 */
export type LoaderState =
  | { status: "loading" }
  | { status: "ready"; versions: LoaderVersion[] }
  | { status: "unsupported" }
  | { status: "error"; error: AppError };

/** 把 `loader_list` 的结果规整为状态：空列表与 `LOADER_UNSUPPORTED_VERSION` 都视为不兼容。 */
export function toState(result: { versions: LoaderVersion[] } | AppError): LoaderState {
  if ("versions" in result) {
    return result.versions.length > 0
      ? { status: "ready", versions: result.versions }
      : { status: "unsupported" };
  }
  return result.code === "LOADER_UNSUPPORTED_VERSION"
    ? { status: "unsupported" }
    : { status: "error", error: result };
}

/** 默认选中的版本：推荐版本，其次最新正式版，再次最新版本。 */
export function defaultVersion(versions: readonly LoaderVersion[]): LoaderVersion | null {
  return (
    versions.find((v) => v.recommended) ?? versions.find((v) => v.stable) ?? versions[0] ?? null
  );
}

/**
 * 下拉框中显示的版本：默认只显示正式版；没有正式版、勾选“显示测试版”或当前选中的是测试版时显示全部。
 */
export function visibleVersions(
  versions: readonly LoaderVersion[],
  showUnstable: boolean,
  selected: string | null,
): LoaderVersion[] {
  const stable = versions.filter((v) => v.stable);
  const selectedUnstable = versions.some((v) => v.version === selected && !v.stable);
  return showUnstable || stable.length === 0 || selectedUnstable ? [...versions] : stable;
}
