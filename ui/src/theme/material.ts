// 窗口材质（PLAN.md M1-11）：Windows 11 用 Mica（不可用时 Acrylic），macOS 用 vibrancy，
// Linux 与 Windows 10 回退为纯色背景。窗口在 tauri.conf.json 中设为透明，
// 材质生效时页面底色透明，否则使用主题底色。
import { isTauri } from "@tauri-apps/api/core";
import { Effect, getCurrentWindow } from "@tauri-apps/api/window";

/** 当前平台可用的窗口材质。 */
export type MaterialKind = "mica" | "vibrancy" | "none";

/** Chromium 的 User-Agent Client Hints；WebView2 与 Chrome 提供，其他浏览器没有。 */
interface UserAgentData {
  platform: string;
  getHighEntropyValues(hints: string[]): Promise<{ platformVersion?: string }>;
}

/**
 * 判断平台是否支持窗口材质。Windows 上 Client Hints 的 `platformVersion` 主版本号
 * ≥ 13 即为 Windows 11；Windows 10 的 Acrylic 在拖动窗口时卡顿，因此不启用。
 */
export async function detectMaterial(
  uaData: UserAgentData | undefined = (navigator as { userAgentData?: UserAgentData })
    .userAgentData,
  userAgent: string = navigator.userAgent,
): Promise<MaterialKind> {
  if (uaData?.platform === "Windows") {
    const { platformVersion } = await uaData.getHighEntropyValues(["platformVersion"]);
    return Number(platformVersion?.split(".")[0]) >= 13 ? "mica" : "none";
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
): Promise<boolean> {
  if (!isTauri()) return false;
  const window = getCurrentWindow();
  try {
    await window.setTheme(dark ? "dark" : "light");
    if (!enabled || kind === "none") {
      await window.clearEffects();
      return false;
    }
    const effects = kind === "mica" ? [Effect.Mica, Effect.Acrylic] : [Effect.Sidebar];
    await window.setEffects({ effects });
    return true;
  } catch {
    return false;
  }
}
