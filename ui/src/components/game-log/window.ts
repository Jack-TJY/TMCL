// 独立的游戏日志窗口：每个实例一个窗口，可自由拖动和缩放，记住上次关闭前的位置与大小。
// 窗口加载同一个前端，地址为 `index.html#/game-log/<实例 ID>`，`main.ts` 据此挂载日志界面。
import { availableMonitors, getCurrentWindow, type Monitor } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

/** 日志窗口地址中 hash 的前缀。 */
export const LOG_WINDOW_HASH = "#/game-log/";
/** 保存窗口位置与大小的 localStorage key，所有日志窗口共用。 */
export const GEOMETRY_KEY = "tmcl.gameLogWindow";

const DEFAULT_SIZE = { width: 900, height: 560 };
const MIN_SIZE = { width: 480, height: 320 };

/** 逻辑像素下的窗口位置与大小。 */
export interface Geometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** 实例对应的窗口标签；标签只允许字母、数字、`-` 和 `_`。 */
export function logWindowLabel(instanceId: string): string {
  return `game-log-${instanceId.replace(/[^a-zA-Z0-9_-]/g, "_")}`;
}

/** 从 `location.hash` 解析日志窗口的实例 ID；不是日志窗口时为 `null`。 */
export function logWindowInstance(hash: string): string | null {
  if (!hash.startsWith(LOG_WINDOW_HASH)) return null;
  const id = decodeURIComponent(hash.slice(LOG_WINDOW_HASH.length));
  return id || null;
}

/** 读取保存的位置与大小；内容无效时为 `null`。 */
export function parseGeometry(raw: string | null): Geometry | null {
  if (!raw) return null;
  try {
    const g = JSON.parse(raw) as Partial<Geometry>;
    const values = [g.x, g.y, g.width, g.height];
    if (!values.every((v) => typeof v === "number" && Number.isFinite(v))) return null;
    return {
      x: g.x!,
      y: g.y!,
      width: Math.max(MIN_SIZE.width, g.width!),
      height: Math.max(MIN_SIZE.height, g.height!),
    };
  } catch {
    return null;
  }
}

/** 逻辑像素下的显示器区域。 */
export interface Area {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** 窗口标题栏左侧是否落在某个显示器内；拔掉显示器后避免窗口出现在屏幕外。 */
export function isVisible(g: Geometry, areas: Area[]): boolean {
  const px = g.x + 40;
  const py = g.y + 20;
  return areas.some((a) => px >= a.x && px < a.x + a.width && py >= a.y && py < a.y + a.height);
}

function monitorArea(m: Monitor): Area {
  const s = m.scaleFactor;
  return {
    x: m.position.x / s,
    y: m.position.y / s,
    width: m.size.width / s,
    height: m.size.height / s,
  };
}

function readGeometry(): Geometry | null {
  try {
    return parseGeometry(localStorage.getItem(GEOMETRY_KEY));
  } catch {
    return null;
  }
}

/** 打开实例的日志窗口；已打开时还原并聚焦。 */
export async function openLogWindow(instanceId: string, title: string): Promise<void> {
  const label = logWindowLabel(instanceId);
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.unminimize().catch(() => undefined);
    await existing.setFocus();
    return;
  }
  const saved = readGeometry();
  const areas = await availableMonitors()
    .then((ms) => ms.map(monitorArea))
    .catch(() => []);
  const position = saved && isVisible(saved, areas) ? { x: saved.x, y: saved.y } : null;
  const window = new WebviewWindow(label, {
    url: `index.html${LOG_WINDOW_HASH}${encodeURIComponent(instanceId)}`,
    title,
    width: saved?.width ?? DEFAULT_SIZE.width,
    height: saved?.height ?? DEFAULT_SIZE.height,
    minWidth: MIN_SIZE.width,
    minHeight: MIN_SIZE.height,
    ...(position ?? { center: true }),
    decorations: false,
    transparent: true,
    focus: true,
  });
  await new Promise<void>((resolve, reject) => {
    void window.once("tauri://created", () => resolve());
    void window.once<unknown>("tauri://error", (e) => reject(e.payload));
  });
}

/** 日志窗口：移动或缩放后保存位置与大小（最大化、最小化时不保存）。 */
export async function rememberGeometry(): Promise<void> {
  const window = getCurrentWindow();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const save = () => {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        if ((await window.isMaximized()) || (await window.isMinimized())) return;
        const scale = await window.scaleFactor();
        const pos = (await window.outerPosition()).toLogical(scale);
        const size = (await window.innerSize()).toLogical(scale);
        const geometry: Geometry = {
          x: Math.round(pos.x),
          y: Math.round(pos.y),
          width: Math.round(size.width),
          height: Math.round(size.height),
        };
        localStorage.setItem(GEOMETRY_KEY, JSON.stringify(geometry));
      } catch {
        // 记不住位置不影响使用。
      }
    }, 300);
  };
  await Promise.all([window.onMoved(save), window.onResized(save)]);
}
