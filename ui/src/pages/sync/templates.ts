// 手动编辑的大版本模板：设置改动较多的几个版本的默认设置（常用项）。
// 值使用主配置的规范格式（1.13 以后），同步到旧版本时由后端转换按键与语言代码；
// 只保留该版本存在的选项（见 versions.ts），个别写法不同的选项按版本单独给值。
import type { SyncOption } from "../../bindings/SyncOption";
import { supportedOn } from "./versions";

/** 一个模板。 */
export interface SyncTemplate {
  /** 版本号，同时作为 ID。 */
  version: string;
  options: SyncOption[];
}

/** 模板说明的文案 key。 */
export function templateNoteKey(version: string): string {
  return `sync.template.note.${version.replace(/\./g, "_")}`;
}

const k = (name: string) => `key.keyboard.${name}`;

/** 原版按键的默认绑定。 */
const BINDINGS: [string, string][] = [
  ["attack", "key.mouse.left"],
  ["use", "key.mouse.right"],
  ["forward", k("w")],
  ["left", k("a")],
  ["back", k("s")],
  ["right", k("d")],
  ["jump", k("space")],
  ["sneak", k("left.shift")],
  ["sprint", k("left.control")],
  ["drop", k("q")],
  ["inventory", k("e")],
  ["chat", k("t")],
  ["playerlist", k("tab")],
  ["pickItem", "key.mouse.middle"],
  ["command", k("slash")],
  ["socialInteractions", k("p")],
  ["screenshot", k("f2")],
  ["togglePerspective", k("f5")],
  ["smoothCamera", k("unknown")],
  ["fullscreen", k("f11")],
  ["spectatorOutlines", k("unknown")],
  ["swapHands", k("f")],
  ["swapOffhand", k("f")],
  ["saveToolbarActivator", k("c")],
  ["loadToolbarActivator", k("x")],
  ["advancements", k("l")],
  ...Array.from({ length: 9 }, (_, i): [string, string] => [`hotbar.${i + 1}`, k(String(i + 1))]),
];

const SOUNDS = [
  "master",
  "music",
  "record",
  "weather",
  "block",
  "hostile",
  "neutral",
  "player",
  "ambient",
  "voice",
];

/** 视频、音频与控制的默认值（新版写法）。 */
const BASE: [string, string][] = [
  ["renderDistance", "12"],
  ["simulationDistance", "12"],
  ["graphicsMode", "1"],
  ["fancyGraphics", "true"],
  ["maxFps", "120"],
  ["enableVsync", "true"],
  ["bobView", "true"],
  ["guiScale", "0"],
  ["attackIndicator", "1"],
  ["gamma", "0.5"],
  ["renderClouds", "true"],
  ["particles", "0"],
  ["mipmapLevels", "4"],
  ["entityShadows", "true"],
  ["entityDistanceScaling", "1.0"],
  ["biomeBlendRadius", "2"],
  ["fov", "0.0"],
  ["fovEffectScale", "1.0"],
  ["screenEffectScale", "1.0"],
  ["prioritizeChunkUpdates", "0"],
  ["useVbo", "true"],
  ...SOUNDS.map((s): [string, string] => [`soundCategory_${s}`, "1.0"]),
  ["showSubtitles", "false"],
  ["directionalAudio", "false"],
  ["mouseSensitivity", "0.5"],
  ["invertYMouse", "false"],
  ["toggleCrouch", "false"],
  ["toggleSprint", "false"],
  ["mainHand", "right"],
];

/** 平滑光照在 1.12.2 及以前为 0/1/2（关/最小/最大）。 */
const LEGACY_AO: [string, string] = ["ao", "2"];

/** 生成某版本的模板；`extra` 为该版本特有的写法。 */
function build(version: string, extra: [string, string][]): SyncOption[] {
  const entries: [string, string][] = [
    ...BINDINGS.map(([name, value]): [string, string] => [`key_key.${name}`, value]),
    ...BASE,
    ...extra,
  ];
  return entries
    .filter(([key]) => supportedOn(key, version) !== false)
    .map(([key, value]) => ({ key, value }));
}

/** 设置改动较多的版本，按版本从旧到新。 */
export const SYNC_TEMPLATES: SyncTemplate[] = [
  { version: "1.8.9", options: build("1.8.9", [LEGACY_AO]) },
  { version: "1.12.2", options: build("1.12.2", [LEGACY_AO]) },
  { version: "1.16.5", options: build("1.16.5", []) },
  { version: "1.20.1", options: build("1.20.1", [["ao", "true"]]) },
  { version: "1.21.1", options: build("1.21.1", [["ao", "true"]]) },
];

/** 把模板合并进现有选项：同名选项改为模板值，其余保留，新选项追加在末尾。 */
export function mergeOptions(current: SyncOption[], template: SyncOption[]): SyncOption[] {
  const values = new Map(template.map((o) => [o.key, o.value]));
  const merged = current.map((o) => ({ key: o.key, value: values.get(o.key) ?? o.value }));
  const existing = new Set(current.map((o) => o.key));
  return [...merged, ...template.filter((o) => !existing.has(o.key))];
}
