import { describe, it, expect } from "vitest";
import { planRegions, MAX_REQUESTS, CELL, contains } from "./regions";
const base = { x: 176, z: -176, w: 800, h: 420, zoom: 3 };
describe("structure query planning", () => {
  for (const w of [800, 1280])
    it("keeps the known camp through zooms at width " + w, () => {
      for (const scale of [4, 8, 16, 8, 4]) {
        const p = planRegions({ ...base, w, zoom: Math.log2(scale) }, ["abandoned_camp"]);
        expect(p.total).toBe(p.jobs.length);
        expect(p.jobs.filter((j) => contains(j.box, 11, -11))).toHaveLength(1);
        expect(p.jobs[0].box).toMatchObject({ min_x: 0, max_z: -1 });
        for (const { box } of p.jobs) {
          expect(box.max_x - box.min_x).toBeLessThan(256);
          expect(box.max_z - box.min_z).toBeLessThan(256);
        }
      }
    });
  it("covers negative seams exactly once and clips the world edge", () => {
    const p = planRegions({ ...base, x: -CELL * 16, z: 0 }, ["village"]);
    for (const x of [-CELL - 1, -CELL, -CELL + 1])
      for (const z of [-1, 0, 1])
        expect(p.jobs.filter((j) => contains(j.box, x, z))).toHaveLength(1);
    for (const { box } of planRegions({ ...base, x: -30000000, z: 30000000 }, ["village"]).jobs) {
      expect(box.min_x).toBeGreaterThanOrEqual(-1875000);
      expect(box.max_z).toBeLessThanOrEqual(1875000);
    }
  });
  it("bounds far-view work for multiple layers, starting at the center", () => {
    const p = planRegions({ ...base, zoom: 12, w: 3840, h: 2160 }, ["village", "stronghold"]);
    expect(p.total).toBeGreaterThan(1000000);
    expect(p.jobs.length).toBe(MAX_REQUESTS);
    expect(p.jobs[0].kind).toBe("village");
    expect(p.jobs[1].kind).toBe("stronghold");
    expect(new Set(p.jobs.map((j) => j.key)).size).toBe(MAX_REQUESTS);
  });
});
