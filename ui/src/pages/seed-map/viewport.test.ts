import { describe, expect, it } from "vitest";
import { tilesInView, blockAt, zoomAt, jumpPoint } from "./viewport";
const v = { x: 0, z: 0, zoom: 0, w: 512, h: 256 };
describe("seed map viewport", () => {
  it("uses floor at negative edges and excludes a tile ending exactly at the viewport", () => {
    const tiles = tilesInView({ ...v, z: 128 });
    expect(tiles).toEqual([
      { x: -1, z: 0, zoom: 0 },
      { x: 0, z: 0, zoom: 0 },
    ]);
  });
  it("keeps the cursor at the same block while zooming", () => {
    const next = zoomAt(v, 1, 32, 90);
    expect(blockAt(next, 32, 90)).toEqual(blockAt(v, 32, 90));
    expect(zoomAt(v, -1, 20, 20)).toEqual(v);
  });
  it("requests only full tiles in the supported world", () => {
    expect(tilesInView({ ...v, x: 30000000 }).every((t) => t.x * 256 + 255 <= 30000000)).toBe(true);
    expect(tilesInView({ ...v, w: 0 })).toEqual([]);
  });
  it("rejects blank, non-integer and out-of-world coordinate input", () => {
    expect(jumpPoint("-12", "0")).toEqual({ x: -12, z: 0 });
    for (const bad of ["", "1.2", "1e2", "30000001"]) expect(jumpPoint(bad, "0")).toBeNull();
  });
});
