// 手动编辑中每个选项使用的控件：开关、下拉、滑块加数值、按键录入，其余退回文本框。
// 数值控件以游戏中的单位显示（百分比、度、区块），保存时换算回 options.txt 中的值。
import { LANGUAGES } from "./options";

export type Control =
  | { kind: "switch" }
  | { kind: "select"; values: string[] }
  | {
      kind: "number";
      min: number;
      max: number;
      step: number;
      /** 显示值 = 存储值 × scale + offset。 */
      scale: number;
      offset: number;
      /** 单位文案 key（`sync.unit.<unit>`），无单位时省略。 */
      unit?: "percent" | "degrees" | "chunks" | "fps";
      /** 存储值是否为整数。 */
      integer: boolean;
    }
  | { kind: "key" }
  | { kind: "text" };

const BOOLEANS = new Set([
  "advancedItemTooltips",
  "allowServerListing",
  "anaglyph3d",
  "autoJump",
  "autoSuggestions",
  "backgroundForChatOnly",
  "bobView",
  "chatColors",
  "chatLinks",
  "chatLinksPrompt",
  "darkMojangStudiosBackground",
  "directionalAudio",
  "discrete_mouse_scroll",
  "enableVsync",
  "entityShadows",
  "fancyGraphics",
  "forceUnicodeFont",
  "fullscreen",
  "hideLightningFlashes",
  "hideMatchedNames",
  "hideSplashTexts",
  "highContrast",
  "invertYMouse",
  "onlyShowSecureChat",
  "operatorItemsTab",
  "pauseOnLostFocus",
  "rawMouseInput",
  "realmsNotifications",
  "reducedDebugInfo",
  "showAutosaveIndicator",
  "showSubtitles",
  "useVbo",
]);

/** 取值固定的选项，值与 `sync.value.<键>.<值>` 文案对应。 */
const SELECTS: Record<string, string[]> = {
  attackIndicator: ["0", "1", "2"],
  chatVisibility: ["0", "1", "2"],
  graphicsMode: ["0", "1", "2"],
  inactivityFpsLimit: ["afk", "minimized"],
  mainHand: ["left", "right"],
  narrator: ["0", "1", "2", "3"],
  particles: ["0", "1", "2"],
  prioritizeChunkUpdates: ["0", "1", "2"],
  renderClouds: ["true", "fast", "false"],
  toggleCrouch: ["false", "true"],
  toggleSprint: ["false", "true"],
};

const num = (
  min: number,
  max: number,
  step: number,
  scale = 1,
  unit?: "percent" | "degrees" | "chunks" | "fps",
  offset = 0,
): Control => ({ kind: "number", min, max, step, scale, offset, unit, integer: scale === 1 });

/** 0–1 的值以百分比显示。 */
const pct = (max = 100) => num(0, max, 1, 100, "percent");

const NUMBERS: Record<string, Control> = {
  biomeBlendRadius: num(0, 7, 1),
  chatLineSpacing: pct(),
  chatOpacity: pct(),
  chatScale: pct(),
  chatWidth: pct(),
  damageTiltStrength: pct(),
  darknessEffectScale: pct(),
  entityDistanceScaling: num(50, 500, 25, 100, "percent"),
  fov: num(30, 110, 1, 40, "degrees", 70),
  fovEffectScale: pct(),
  gamma: pct(),
  glintSpeed: pct(),
  glintStrength: pct(),
  guiScale: num(0, 8, 1),
  maxFps: num(10, 260, 10, 1, "fps"),
  menuBackgroundBlurriness: num(0, 10, 1),
  mipmapLevels: num(0, 4, 1),
  mouseSensitivity: num(0, 200, 1, 200, "percent"),
  mouseWheelSensitivity: num(1, 10, 0.01, 1),
  renderDistance: num(2, 32, 1, 1, "chunks"),
  screenEffectScale: pct(),
  simulationDistance: num(5, 32, 1, 1, "chunks"),
  textBackgroundOpacity: pct(),
};

/** 选项使用的控件；`value` 用于区分写法随版本变化的选项（如平滑光照）。 */
export function controlFor(key: string, value: string): Control {
  if (key.startsWith("key_")) return { kind: "key" };
  if (key.startsWith("soundCategory_")) return pct();
  if (key.startsWith("modelPart_")) return { kind: "switch" };
  if (key === "ao") {
    // 1.12.2 及以前为 0/1/2，新版为开关。
    return /^\d$/.test(value) ? { kind: "select", values: ["0", "1", "2"] } : { kind: "switch" };
  }
  if (key === "lang") return { kind: "select", values: Object.keys(LANGUAGES) };
  if (BOOLEANS.has(key)) return { kind: "switch" };
  if (SELECTS[key]) return { kind: "select", values: SELECTS[key] };
  if (NUMBERS[key]) return NUMBERS[key];
  if (value === "true" || value === "false") return { kind: "switch" };
  return { kind: "text" };
}

/** 存储值 → 控件显示值；无法解析时为 `null`。 */
export function toDisplay(control: Control, value: string): number | null {
  if (control.kind !== "number") return null;
  const n = Number(value);
  if (value.trim() === "" || !Number.isFinite(n)) return null;
  return Math.round((n * control.scale + control.offset) * 100) / 100;
}

/** 控件显示值 → 存储值（限制在范围内）。 */
export function fromDisplay(control: Control, display: number): string {
  if (control.kind !== "number") return String(display);
  const clamped = Math.min(control.max, Math.max(control.min, display));
  const stored = (clamped - control.offset) / control.scale;
  if (control.integer) return String(Math.round(stored));
  // 去掉浮点误差，保留足够精度。
  return String(Math.round(stored * 10000) / 10000);
}

/** 已收录的选项键（供“添加选项”列表使用），不含按键与音量。 */
export const KNOWN_SETTINGS: string[] = [
  ...BOOLEANS,
  ...Object.keys(SELECTS),
  ...Object.keys(NUMBERS),
  "ao",
  "lang",
].sort();

/** 浏览器 `KeyboardEvent.code` → 游戏的键名后缀（`key.keyboard.<名>`）。 */
const CODE_NAMES: Record<string, string> = {
  AltLeft: "left.alt",
  AltRight: "right.alt",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  Backquote: "grave.accent",
  Backslash: "backslash",
  Backspace: "backspace",
  BracketLeft: "left.bracket",
  BracketRight: "right.bracket",
  CapsLock: "caps.lock",
  Comma: "comma",
  ContextMenu: "menu",
  ControlLeft: "left.control",
  ControlRight: "right.control",
  Delete: "delete",
  End: "end",
  Enter: "enter",
  Equal: "equal",
  Home: "home",
  Insert: "insert",
  MetaLeft: "left.win",
  MetaRight: "right.win",
  Minus: "minus",
  NumLock: "num.lock",
  NumpadAdd: "keypad.add",
  NumpadDecimal: "keypad.decimal",
  NumpadDivide: "keypad.divide",
  NumpadEnter: "keypad.enter",
  NumpadEqual: "keypad.equal",
  NumpadMultiply: "keypad.multiply",
  NumpadSubtract: "keypad.subtract",
  PageDown: "page.down",
  PageUp: "page.up",
  Pause: "pause",
  Period: "period",
  PrintScreen: "print.screen",
  Quote: "apostrophe",
  ScrollLock: "scroll.lock",
  Semicolon: "semicolon",
  ShiftLeft: "left.shift",
  ShiftRight: "right.shift",
  Slash: "slash",
  Space: "space",
  Tab: "tab",
};

/** 按下的键 → 游戏键名；Esc 与游戏中一样表示“未绑定”，不认识的键为 `null`。 */
export function keyFromEvent(code: string): string | null {
  if (code === "Escape") return "key.keyboard.unknown";
  const letter = code.match(/^Key([A-Z])$/);
  if (letter) return `key.keyboard.${letter[1].toLowerCase()}`;
  const digit = code.match(/^Digit(\d)$/);
  if (digit) return `key.keyboard.${digit[1]}`;
  const numpad = code.match(/^Numpad(\d)$/);
  if (numpad) return `key.keyboard.keypad.${numpad[1]}`;
  const fn = code.match(/^F(\d{1,2})$/);
  if (fn && Number(fn[1]) <= 25) return `key.keyboard.f${fn[1]}`;
  return CODE_NAMES[code] ? `key.keyboard.${CODE_NAMES[code]}` : null;
}

/** 鼠标按钮（`MouseEvent.button`）→ 游戏键名。 */
export function keyFromMouse(button: number): string {
  return (
    ["key.mouse.left", "key.mouse.middle", "key.mouse.right"][button] ?? `key.mouse.${button + 1}`
  );
}

/** 新添加选项时的初始值（取游戏默认值，未收录的用控件的最小值）。 */
const DEFAULTS: Record<string, string> = {
  ao: "true",
  biomeBlendRadius: "2",
  entityDistanceScaling: "1.0",
  fov: "0.0",
  gamma: "0.5",
  lang: "zh_cn",
  mainHand: "right",
  maxFps: "120",
  mipmapLevels: "4",
  mouseSensitivity: "0.5",
  mouseWheelSensitivity: "1.0",
  renderClouds: "true",
  renderDistance: "12",
  simulationDistance: "12",
  inactivityFpsLimit: "afk",
};

export function defaultValue(key: string): string {
  if (DEFAULTS[key]) return DEFAULTS[key];
  const control = controlFor(key, "");
  switch (control.kind) {
    case "switch":
      return "false";
    case "select":
      return control.values[0];
    case "number":
      return control.unit === "percent" && control.scale === 100
        ? "1.0"
        : fromDisplay(control, control.min);
    case "key":
      return "key.keyboard.unknown";
    default:
      return "";
  }
}
