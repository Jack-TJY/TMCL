// 窗口材质（PLAN.md M1-11）：Windows 11 可选磨砂玻璃（Acrylic）或 Mica，macOS 用 vibrancy，
// Linux 与 Windows 10 回退为背景图层。窗口在 tauri.conf.json 中设为透明，
// 材质生效时页面底色按 `material_opacity` 半透明叠在材质上，否则使用主题底色。
import { isTauri } from "@tauri-apps/api/core";
import { Effect, getCurrentWindow } from "@tauri-apps/api/window";
import type { WindowMaterial } from "../bindings/WindowMaterial";

/**
 * 当前平台可用的窗口材质：`acrylic` 为 Windows 11 22H2 及以上（磨砂玻璃与 Mica 都可用），
 * `mica` 为更早的 Windows 11（Acrylic 走旧接口，拖动窗口时卡顿，只用 Mica）。
 */
export type MaterialKind = "acrylic" | "mica" | "vibrancy" | "none";

/** Chromium 的 User-Agent Client Hints；WebView2 与 Chrome 提供，其他浏览器没有。 */
interface UserAgentData {
  platform: string;
  getHighEntropyValues(hints: string[]): Promise<{ platformVersion?: string }>;
}

/**
 * 判断平台支持的窗口材质。Windows 上 Client Hints 的 `platformVersion` 主版本号
 * ≥ 13 即为 Windows 11，≥ 15 为 22H2 及以上；Windows 10 的 Acrylic 在拖动窗口时卡顿，因此不启用。
 */
export async function detectMaterial(
  uaData: UserAgentData | undefined = (navigator as { userAgentData?: UserAgentData })
    .userAgentData,
  userAgent: string = navigator.userAgent,
): Promise<MaterialKind> {
  if (uaData?.platform === "Windows") {
    const { platformVersion } = await uaData.getHighEntropyValues(["platformVersion"]);
    const major = Number(platformVersion?.split(".")[0]);
    if (major >= 15) return "acrylic";
    return major >= 13 ? "mica" : "none";
  }
  if (/Macintosh|Mac OS X/.test(userAgent)) return "vibrancy";
  return "none";
}

/**
 * Windows 10 上去掉无边框窗口的阴影。Tauri 为无边框窗口模拟阴影时会保留 1px 的 DWM 边框，
 * 在 Windows 10 上窗口获得焦点后显示为一圈细描边；Windows 11 的阴影与圆角依赖它，保持不变。
 */
export async function removeWindows10Frame(
  uaData: UserAgentData | undefined = (navigator as { userAgentData?: UserAgentData })
    .userAgentData,
): Promise<void> {
  if (!isTauri() || uaData?.platform !== "Windows") return;
  const { platformVersion } = await uaData.getHighEntropyValues(["platformVersion"]);
  if (Number(platformVersion?.split(".")[0]) >= 13) return;
  try {
    await getCurrentWindow().setShadow(false);
  } catch {
    // 去不掉描边不影响使用。
  }
}

/**
 * 应用或清除窗口材质，并让材质的明暗跟随界面主题。返回材质是否生效；
 * 不在 Tauri 中运行或调用失败时返回 `false`，页面应使用纯色背景。
 */
export async function applyMaterial(
  kind: MaterialKind,
  enabled: boolean,
  dark: boolean,
  material: WindowMaterial = "acrylic",
): Promise<boolean> {
  if (!isTauri()) return false;
  const window = getCurrentWindow();
  try {
    await window.setTheme(dark ? "dark" : "light");
    if (!enabled || kind === "none") {
      await window.clearEffects();
      return false;
    }
    await window.setEffects({ effects: materialEffects(kind, material) });
    return true;
  } catch {
    return false;
  }
}

/** 平台与所选材质对应的窗口效果。 */
export function materialEffects(kind: MaterialKind, material: WindowMaterial): Effect[] {
  if (kind === "vibrancy") return [Effect.Sidebar];
  if (kind === "acrylic" && material === "acrylic") return [Effect.Acrylic];
  return [Effect.Mica];
}

/** 是否可以在设置中选择材质类型（磨砂玻璃 / Mica）。 */
export function materialSelectable(kind: MaterialKind): boolean {
  return kind === "acrylic";
}
