import type { SeedChunkBounds as Box } from "../../bindings/SeedChunkBounds";
import type { SeedStructureKind as Kind } from "../../bindings/SeedStructureKind";
import { blockAt, LIMIT, type View } from "./viewport";
export const CELL = 256,
  MAX_REQUESTS = 48,
  CACHE_ENTRIES = 128;
const EDGE = LIMIT / 16;
export interface Region {
  key: string;
  kind: Kind;
  box: Box;
}
/** Test whether a start chunk belongs to an inclusive query rectangle. */
export const contains = (box: Box, x: number, z: number) =>
  x >= box.min_x && x <= box.max_x && z >= box.min_z && z <= box.max_z;
export const intersects = (a: Box, b: Box) =>
  a.min_x <= b.max_x && a.max_x >= b.min_x && a.min_z <= b.max_z && a.max_z >= b.min_z;
/** Plan only a bounded center-first frontier, never materialize all far-view cells. */
export function planRegions(v: View, kinds: Kind[]) {
  const start = blockAt(v, 0, 0),
    end = blockAt(v, v.w, v.h);
  const visible = {
    min_x: Math.max(-EDGE, Math.ceil(start.x / 16)),
    max_x: Math.min(EDGE, Math.ceil(end.x / 16) - 1),
    min_z: Math.max(-EDGE, Math.ceil(start.z / 16)),
    max_z: Math.min(EDGE, Math.ceil(end.z / 16) - 1),
  };
  const jobs: Region[] = [];
  if (
    !kinds.length ||
    v.w <= 0 ||
    v.h <= 0 ||
    visible.max_x < visible.min_x ||
    visible.max_z < visible.min_z
  )
    return { visible, total: 0, jobs };
  const cell = (x: number) => Math.floor(x / CELL);
  const x0 = cell(visible.min_x),
    x1 = cell(visible.max_x),
    z0 = cell(visible.min_z),
    z1 = cell(visible.max_z);
  const total = (x1 - x0 + 1) * (z1 - z0 + 1) * kinds.length;
  const cx = v.x / (16 * CELL),
    cz = v.z / (16 * CELL),
    seen = new Set<string>();
  const frontier: { x: number; z: number; dist: number }[] = [];
  function add(x: number, z: number) {
    const key = x + "," + z;
    if (seen.has(key) || x < x0 || x > x1 || z < z0 || z > z1) return;
    seen.add(key);
    frontier.push({ x, z, dist: (x + 0.5 - cx) ** 2 + (z + 0.5 - cz) ** 2 });
  }
  add(Math.max(x0, Math.min(x1, Math.floor(cx))), Math.max(z0, Math.min(z1, Math.floor(cz))));
  while (frontier.length && jobs.length + kinds.length <= MAX_REQUESTS) {
    frontier.sort((a, b) => a.dist - b.dist || a.z - b.z || a.x - b.x);
    const { x, z } = frontier.shift()!;
    const box = {
      min_x: Math.max(-EDGE, x * CELL),
      max_x: Math.min(EDGE, (x + 1) * CELL - 1),
      min_z: Math.max(-EDGE, z * CELL),
      max_z: Math.min(EDGE, (z + 1) * CELL - 1),
    };
    for (const kind of kinds) jobs.push({ key: [kind, x, z].join("/"), kind, box });
    add(x - 1, z);
    add(x + 1, z);
    add(x, z - 1);
    add(x, z + 1);
  }
  return { visible, total, jobs };
}
