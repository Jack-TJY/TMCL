import type { SeedTile } from "../../bindings/SeedTile";
export const LIMIT = 30_000_000;
export const SIZE = 256;
export interface View {
  x: number;
  z: number;
  zoom: number;
  w: number;
  h: number;
}
export const clamp = (n: number) => Math.max(-LIMIT, Math.min(LIMIT, n));
/** World coordinates at a CSS pixel offset from the viewport's top left. */
export function blockAt(v: View, px: number, py: number) {
  const step = 2 ** v.zoom;
  return { x: v.x + (px - v.w / 2) * step, z: v.z + (py - v.h / 2) * step };
}
/** Change sampling scale around an anchor; zoom 0 is one block per pixel. */
export function zoomAt(v: View, delta: number, px: number, py: number): View {
  const zoom = Math.max(0, Math.min(12, v.zoom + delta));
  const point = blockAt(v, px, py);
  return {
    ...v,
    zoom,
    x: clamp(point.x - (px - v.w / 2) * 2 ** zoom),
    z: clamp(point.z - (py - v.h / 2) * 2 ** zoom),
  };
}
export const tileKey = (t: SeedTile) => [t.zoom, t.x, t.z].join("/");
/** Enumerate the half-open screen rect, excluding edge tiles rejected by the backend. */
export function tilesInView(v: View): SeedTile[] {
  if (v.w <= 0 || v.h <= 0) return [];
  const step = 2 ** v.zoom,
    span = SIZE * step;
  const start = blockAt(v, 0, 0),
    end = blockAt(v, v.w, v.h);
  const lo = Math.ceil(-LIMIT / span),
    hi = Math.floor((LIMIT - (SIZE - 1) * step) / span);
  const tiles: SeedTile[] = [];
  for (
    let z = Math.max(lo, Math.floor(start.z / span));
    z <= Math.min(hi, Math.ceil(end.z / span) - 1);
    z++
  )
    for (
      let x = Math.max(lo, Math.floor(start.x / span));
      x <= Math.min(hi, Math.ceil(end.x / span) - 1);
      x++
    )
      tiles.push({ x: x || 0, z: z || 0, zoom: v.zoom });
  return tiles;
}
/** Accept only signed decimal block coordinates inside the Java world. */
export function jumpPoint(xs: string, zs: string) {
  const parse = (s: string) => (/^[+-]?\d+$/.test(s.trim()) ? Number(s) : NaN);
  const x = parse(xs),
    z = parse(zs);
  return Number.isSafeInteger(x) &&
    Number.isSafeInteger(z) &&
    Math.abs(x) <= LIMIT &&
    Math.abs(z) <= LIMIT
    ? { x, z }
    : null;
}
