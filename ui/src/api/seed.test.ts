import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SeedMapArgs } from "../bindings/SeedMapArgs";
import type { SeedMapResult } from "../bindings/SeedMapResult";
const call = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({
  invoke: call,
  Channel: class {
    onmessage?: (m: SeedMapResult) => void;
  },
}));
import { querySeedMap } from "./seed";

describe("seed_map_query", () => {
  beforeEach(() => {
    call.mockReset();
  });
  it("keeps precise seed text and routes early replies only to their request", async () => {
    const tile = { x: -1, z: 0, zoom: 0 };
    const a: SeedMapArgs = {
      version: "26.3",
      seed: "9223372036854775807",
      query: { type: "tiles", tiles: [tile] },
    };
    const result: SeedMapResult = { type: "tile", tile, png: [137, 80, 78, 71] };
    call.mockImplementation((_cmd, { results }) => {
      results.onmessage(result);
      return Promise.resolve({ task_id: "t" });
    });
    const first = vi.fn();
    const second = vi.fn();
    await expect(querySeedMap(a, first)).resolves.toEqual({ task_id: "t" });
    await querySeedMap(a, second);
    expect(call).toHaveBeenCalledWith("seed_map_query", { args: a, results: expect.anything() });
    expect(call.mock.calls[0][1].results).not.toBe(call.mock.calls[1][1].results);
    expect(first).toHaveBeenCalledExactlyOnceWith(result);
    expect(second).toHaveBeenCalledExactlyOnceWith(result);
  });
});
