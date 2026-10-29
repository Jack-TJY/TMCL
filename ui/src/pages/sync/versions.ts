// 选项在不同游戏版本中的有无（高低版本差异）。
// 只收录确定会随版本出现或消失的原版选项；不在表中的选项视为各版本通用。
// 区间来自 Minecraft Wiki 与各版本更新日志，按正式版号记录（快照按其所属正式版处理）。

/** 选项存在的版本区间（含两端）；缺省表示不限。 */
export interface OptionSpan {
  since?: string;
  until?: string;
}

/** 按键从键码改为键名（`key.keyboard.*`）的版本。 */
export const NAMED_KEYS_SINCE = "1.13";

export const OPTION_SPANS: Record<string, OptionSpan> = {
  anaglyph3d: { until: "1.12.2" },
  attackIndicator: { since: "1.9" },
  autoJump: { since: "1.11" },
  biomeBlendRadius: { since: "1.13" },
  damageTiltStrength: { since: "1.19.4" },
  darknessEffectScale: { since: "1.19" },
  directionalAudio: { since: "1.19" },
  entityDistanceScaling: { since: "1.16" },
  fancyGraphics: { until: "1.15.2" },
  fovEffectScale: { since: "1.16" },
  fullscreenResolution: { since: "1.14" },
  graphicsMode: { since: "1.16" },
  inactivityFpsLimit: { since: "1.21.2" },
  "key_key.advancements": { since: "1.12" },
  "key_key.loadToolbarActivator": { since: "1.12" },
  "key_key.saveToolbarActivator": { since: "1.12" },
  "key_key.socialInteractions": { since: "1.16.4" },
  "key_key.streamCommercials": { until: "1.8.9" },
  "key_key.streamPauseUnpause": { until: "1.8.9" },
  "key_key.streamStartStop": { until: "1.8.9" },
  "key_key.streamToggleMic": { until: "1.8.9" },
  "key_key.swapHands": { since: "1.9", until: "1.15.2" },
  "key_key.swapOffhand": { since: "1.16" },
  mainHand: { since: "1.9" },
  menuBackgroundBlurriness: { since: "1.20.5" },
  narrator: { since: "1.12" },
  operatorItemsTab: { since: "1.19.3" },
  prioritizeChunkUpdates: { since: "1.19" },
  screenEffectScale: { since: "1.16" },
  showSubtitles: { since: "1.9" },
  simulationDistance: { since: "1.18" },
  toggleCrouch: { since: "1.15" },
  toggleSprint: { since: "1.15" },
  useVbo: { until: "1.14.4" },
};

/** 解析正式版号（`1.20.1`、`1.8`）；快照、预览版等无法比较的版本返回 `null`。 */
export function parseRelease(version: string): number[] | null {
  const m = version.trim().match(/^(\d+)\.(\d+)(?:\.(\d+))?$/);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3] ?? 0)] : null;
}

/** 比较两个正式版号；任一无法解析时返回 `null`。 */
export function compareVersions(a: string, b: string): number | null {
  const pa = parseRelease(a);
  const pb = parseRelease(b);
  if (!pa || !pb) return null;
  for (let i = 0; i < 3; i++) if (pa[i] !== pb[i]) return pa[i] - pb[i];
  return 0;
}

/** 选项在该版本中是否存在；没有版本记录或版本无法比较时返回 `null`（未知）。 */
export function supportedOn(key: string, version: string): boolean | null {
  const span = OPTION_SPANS[key];
  if (!span) return null;
  const after = span.since ? compareVersions(version, span.since) : 1;
  const before = span.until ? compareVersions(version, span.until) : -1;
  if (after === null || before === null) return null;
  return after >= 0 && before <= 0;
}

/** 该版本是否使用旧版数字键码；版本无法比较时返回 `null`。 */
export function usesLegacyKeys(version: string): boolean | null {
  const cmp = compareVersions(version, NAMED_KEYS_SINCE);
  return cmp === null ? null : cmp < 0;
}

/** 区间的简写：`1.16+`、`≤1.15.2`、`1.9–1.15.2`。 */
export function spanText(span: OptionSpan): string {
  if (span.since && span.until) return `${span.since}–${span.until}`;
  if (span.since) return `${span.since}+`;
  return span.until ? `≤${span.until}` : "";
}
