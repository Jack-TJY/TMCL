// 联机页的纯函数（PLAN.md M2-15）：地址与端口校验、节点排序、延迟分级，以及只保存在本机 WebView 中的小偏好。

import type { SakuraNodeInfo } from "../../api/sakura";

/** 联机方式；与页面上的选项卡一致。 */
export type MultiplayerMode = "direct" | "sakura";
/** 房主开房，或成员加入。 */
export type MultiplayerRole = "host" | "member";

/** 游戏端口是否在 1–65535 内。 */
export function isValidPort(port: number | null | undefined): port is number {
  return typeof port === "number" && Number.isInteger(port) && port >= 1 && port <= 65535;
}

const HOST = /^(?=.{1,253}$)[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$/i;

/**
 * 规整房主分享的联机地址：去掉首尾空白，`主机:端口` 合法时返回小写形式，否则返回 `null`。
 * 只接受域名或 IPv4，因为樱花穿透给出的地址都是这两种。
 */
export function normalizeAddress(input: string): string | null {
  const text = input.trim();
  const index = text.lastIndexOf(":");
  if (index <= 0) return null;
  const host = text.slice(0, index);
  const portText = text.slice(index + 1);
  if (!/^\d{1,5}$/.test(portText) || !isValidPort(Number(portText))) return null;
  if (!HOST.test(host)) return null;
  return `${host.toLowerCase()}:${Number(portText)}`;
}

/** 节点能否用于这次联机：在线未满载，公开房间还要求节点允许公开游戏。 */
export function nodeUsable(node: SakuraNodeInfo, isPublic: boolean): boolean {
  return node.available && (!isPublic || node.public_game);
}

/** 节点排序：可用的在前，其次不需要付费用户组的，最后按 ID。 */
export function sortNodes(nodes: SakuraNodeInfo[], isPublic: boolean): SakuraNodeInfo[] {
  const rank = (n: SakuraNodeInfo) => (nodeUsable(n, isPublic) ? 0 : 2) + (n.vip > 0 ? 1 : 0);
  return [...nodes].sort((a, b) => rank(a) - rank(b) || a.id - b.id);
}

/** 延迟分级，用于显示颜色。 */
export type LatencyLevel = "good" | "fair" | "poor" | "unreachable";

export function latencyLevel(ms: number | null): LatencyLevel {
  if (ms === null) return "unreachable";
  if (ms < 80) return "good";
  if (ms < 200) return "fair";
  return "poor";
}

/** 存取 WebView 本地存储；隐私模式或存储被禁用时静默失败。 */
function load(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function save(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // 只是便利功能，存不下也不影响使用。
  }
}

const TERMS_KEY = "tmcl.sakura.termsAccepted";
const PREFS_KEY = "tmcl.multiplayer.prefs";

/** 是否已同意这一版（以更新时间区分）服务条款；条款更新后需要重新同意。 */
export function termsAccepted(updated: string): boolean {
  return load(TERMS_KEY) === updated;
}

export function acceptTerms(updated: string) {
  save(TERMS_KEY, updated);
}

/** 记住的上次选择，只为下次少点几下。 */
export interface MultiplayerPrefs {
  mode: MultiplayerMode;
  role: MultiplayerRole;
  node: number | null;
  port: number | null;
}

export const DEFAULT_PREFS: MultiplayerPrefs = {
  mode: "direct",
  role: "host",
  node: null,
  port: null,
};

/** 解析保存的偏好，缺失或无效的字段取默认值。 */
export function parsePrefs(raw: string | null): MultiplayerPrefs {
  let value: Record<string, unknown> = {};
  try {
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    if (parsed && typeof parsed === "object") value = parsed as Record<string, unknown>;
  } catch {
    // 回退默认值。
  }
  return {
    mode: value.mode === "sakura" || value.mode === "direct" ? value.mode : DEFAULT_PREFS.mode,
    role: value.role === "member" || value.role === "host" ? value.role : DEFAULT_PREFS.role,
    node: typeof value.node === "number" && Number.isInteger(value.node) ? value.node : null,
    port: isValidPort(value.port as number) ? (value.port as number) : null,
  };
}

export function loadPrefs(): MultiplayerPrefs {
  return parsePrefs(load(PREFS_KEY));
}

export function savePrefs(change: Partial<MultiplayerPrefs>) {
  save(PREFS_KEY, JSON.stringify({ ...loadPrefs(), ...change }));
}

/** Scaffolding 房间码字母表，与后端 `room.rs` 一致（去掉了易混的 I、O）。 */
const ROOM_ALPHABET = "0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/**
 * 规整用户输入的房间码 `U/NNNN-NNNN-SSSS-SSSS`：忽略首尾空白与大小写，`U/` 可省略。
 * 格式与校验位（34 进制小端数值是 7 的倍数）都正确时返回完整房间码，否则返回 `null`。
 * 只是前端预检，后端加入时会再解析一次。
 */
export function normalizeRoomCode(input: string): string | null {
  const upper = input.trim().toUpperCase();
  const body = upper.startsWith("U/") ? upper.slice(2) : upper;
  const groups = body.split("-");
  if (groups.length !== 4 || groups.some((g) => g.length !== 4)) return null;
  let value = 0n;
  let weight = 1n;
  for (const ch of groups.join("")) {
    const digit = ROOM_ALPHABET.indexOf(ch);
    if (digit < 0) return null;
    value += BigInt(digit) * weight;
    weight *= 34n;
  }
  return value % 7n === 0n ? `U/${groups.join("-")}` : null;
}

/** 玩家名：去掉首尾空白后 1–64 个字符，与后端限制一致。 */
export function isValidPlayerName(name: string): boolean {
  const length = [...name.trim()].length;
  return length >= 1 && length <= 64;
}
