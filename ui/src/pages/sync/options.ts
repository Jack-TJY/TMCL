// 把 options.txt 的键与值显示为游戏内的名称（跟随界面语言）。
// 文案在 locales/<语言>/sync.json 的 `option` / `binding` / `sound` / `modelPart` / `keyName` / `value` 下；
// 没有收录的选项（如模组按键）返回 `null`，界面退回显示原始键名。
// 旧版（1.12.2 及以前）按键值是 LWJGL2 键码，借用 M2-09 的映射表转回键名再显示。
import keymap from "../../../../modules/sync/data/keymap-lwjgl2-glfw.json";

/** `useI18n()` 中用到的两个函数。 */
export interface Translator {
  t: (key: string, params?: Record<string, unknown>) => string;
  te: (key: string) => boolean;
}

const BINDING_PREFIX = "key_key.";
const SOUND_PREFIX = "soundCategory_";
const MODEL_PREFIX = "modelPart_";
const KEYBOARD_PREFIX = "key.keyboard.";
const MOUSE_PREFIX = "key.mouse.";
/** LWJGL2 鼠标键码：`-100 + 按钮序号`。 */
const MOUSE_BASE = -100;

const CODE_TO_KEY = new Map(Object.entries(keymap.keyboard).map(([name, code]) => [code, name]));

function lookup(tr: Translator, key: string, params?: Record<string, unknown>): string | null {
  return tr.te(key) ? tr.t(key, params) : null;
}

/** 选项在游戏设置界面中的名称；未收录时为 `null`。 */
export function optionLabel(key: string, tr: Translator): string | null {
  if (key.startsWith(BINDING_PREFIX)) {
    const name = key.slice(BINDING_PREFIX.length);
    const hotbar = name.match(/^hotbar\.(\d)$/);
    if (hotbar) return tr.t("sync.binding.hotbar", { n: hotbar[1] });
    return name.includes(".") ? null : lookup(tr, `sync.binding.${name}`);
  }
  if (key.startsWith(SOUND_PREFIX))
    return lookup(tr, `sync.sound.${key.slice(SOUND_PREFIX.length)}`);
  if (key.startsWith(MODEL_PREFIX))
    return lookup(tr, `sync.modelPart.${key.slice(MODEL_PREFIX.length)}`);
  return /^\w+$/.test(key) ? lookup(tr, `sync.option.${key}`) : null;
}

/** 把旧版 LWJGL2 键码转为 1.13 以后的键名；不认识时返回 `null`。 */
export function legacyCodeToKey(code: number): string | null {
  if (code >= MOUSE_BASE && code < MOUSE_BASE + 16) {
    const button = code - MOUSE_BASE;
    return MOUSE_PREFIX + (["left", "right", "middle"][button] ?? String(button + 1));
  }
  return CODE_TO_KEY.get(code) ?? null;
}

/** 与语言无关的按键名：字母、数字、功能键与符号。 */
const SYMBOLS: Record<string, string> = {
  apostrophe: "'",
  backslash: "\\",
  comma: ",",
  equal: "=",
  "grave.accent": "`",
  "left.bracket": "[",
  minus: "-",
  period: ".",
  "right.bracket": "]",
  semicolon: ";",
  slash: "/",
};

/** 按键名（`key.keyboard.w`、`key.mouse.left` 或旧版键码）在游戏中的显示。 */
export function keyLabel(value: string, tr: Translator): string | null {
  let name = value;
  if (/^-?\d+$/.test(value)) {
    const converted = legacyCodeToKey(Number(value));
    if (!converted) return null;
    name = converted;
  }
  if (name.startsWith(MOUSE_PREFIX)) {
    const button = name.slice(MOUSE_PREFIX.length);
    return /^\d+$/.test(button)
      ? tr.t("sync.keyName.mouseButton", { n: button })
      : lookup(tr, `sync.keyName.mouse_${button}`);
  }
  if (!name.startsWith(KEYBOARD_PREFIX)) return null;
  const key = name.slice(KEYBOARD_PREFIX.length);
  if (/^[a-z0-9]$/.test(key) || /^f\d{1,2}$/.test(key)) return key.toUpperCase();
  if (SYMBOLS[key]) return SYMBOLS[key];
  const keypad = key.match(/^keypad\.(.+)$/);
  if (keypad) {
    const part = keypad[1];
    const sign = { add: "+", subtract: "-", multiply: "*", divide: "/", decimal: ".", equal: "=" }[
      part
    ];
    const label = /^\d$/.test(part) ? part : (sign ?? lookup(tr, `sync.keyName.${part}`));
    return label ? tr.t("sync.keyName.keypad", { key: label }) : null;
  }
  return lookup(tr, `sync.keyName.${key.replace(/\./g, "_")}`);
}

/** 自带名称的语言，与 LOCALE_NAMES 一样不进入翻译文件。 */
const LANGUAGES: Record<string, string> = {
  de_de: "Deutsch",
  en_gb: "English (UK)",
  en_us: "English (US)",
  es_es: "Español",
  fr_fr: "Français",
  ja_jp: "日本語",
  ko_kr: "한국어",
  pt_br: "Português (Brasil)",
  ru_ru: "Русский",
  zh_cn: "简体中文",
  zh_hk: "繁體中文（香港）",
  zh_tw: "繁體中文（台灣）",
};

/** 值为 0–1、游戏中以百分比显示的选项（倍数）。 */
const PERCENT: Record<string, number> = {
  chatOpacity: 100,
  chatScale: 100,
  chatWidth: 100,
  damageTiltStrength: 100,
  darknessEffectScale: 100,
  fovEffectScale: 100,
  gamma: 100,
  glintSpeed: 100,
  glintStrength: 100,
  menuBackgroundBlurriness: 100,
  mouseSensitivity: 200,
  screenEffectScale: 100,
  textBackgroundOpacity: 100,
};

/** 取值为固定几项的选项，文案在 `sync.value.<键>.<值>`。 */
const ENUMS = new Set([
  "ao",
  "attackIndicator",
  "chatVisibility",
  "graphicsMode",
  "inactivityFpsLimit",
  "mainHand",
  "narrator",
  "particles",
  "prioritizeChunkUpdates",
  "renderClouds",
  "toggleCrouch",
  "toggleSprint",
]);

function percent(value: string, scale: number): string | null {
  const n = Number(value);
  return Number.isFinite(n) ? `${Math.round(n * scale)}%` : null;
}

/** 值在游戏中的显示；无法解释时为 `null`，界面只显示原值。 */
export function valueLabel(key: string, value: string, tr: Translator): string | null {
  if (key.startsWith("key_")) return keyLabel(value, tr);
  if (key.startsWith(SOUND_PREFIX)) return percent(value, 100);
  if (ENUMS.has(key)) return lookup(tr, `sync.value.${key}.${value}`);
  if (PERCENT[key]) return percent(value, PERCENT[key]);
  const n = Number(value);
  switch (key) {
    case "lang":
      return LANGUAGES[value.toLowerCase()] ?? null;
    case "fov":
      return Number.isFinite(n) ? String(Math.round(70 + 40 * n)) : null;
    case "guiScale":
      return value === "0" ? tr.t("sync.value.auto") : null;
    case "maxFps":
      return n >= 260 ? tr.t("sync.value.unlimited") : null;
    case "renderDistance":
    case "simulationDistance":
      return Number.isInteger(n) ? tr.t("sync.value.chunks", { n }) : null;
  }
  if (value === "true") return tr.t("sync.value.on");
  if (value === "false") return tr.t("sync.value.off");
  return null;
}
