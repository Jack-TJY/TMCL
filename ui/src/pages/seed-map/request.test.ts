import { describe, it, expect, vi } from "vitest";
import { runQuery } from "./request";
import type { SeedMapQueryArgs } from "../../api/seed";
import type { TaskFinished } from "../../bindings/TaskFinished";
import type { SeedMapResult } from "../../bindings/SeedMapResult";
const tile = { x: 0, z: 0, zoom: 0 };
const args: SeedMapQueryArgs = {
  seed: "0",
  version: "26.3",
  query: { type: "tiles" as const, tiles: [tile] },
};
const msg: SeedMapResult = { type: "tile", tile, png: [] };
describe("map request lifecycle", () => {
  it("accepts finished before invoke and channel result after finished", async () => {
    let end!: (t: TaskFinished) => void, deliver!: (m: SeedMapResult) => void;
    const off = vi.fn(),
      apply = vi.fn();
    const result = runQuery(args, new AbortController().signal, apply, {
      listen: async (fn) => ((end = fn), off),
      start: async (_, fn) => {
        deliver = fn;
        end({ task_id: "mine", state: "succeeded" });
        return { task_id: "mine" };
      },
      cancel: vi.fn(),
    });
    await vi.waitFor(() => expect(deliver).toBeDefined());
    expect(off).not.toHaveBeenCalled();
    deliver(msg);
    await result;
    expect(apply).toHaveBeenCalledOnce();
    expect(off).toHaveBeenCalledOnce();
  });
  it("ignores stale messages and cancels even when ID arrives after disposal", async () => {
    const controller = new AbortController();
    let resolve!: (r: { task_id: string }) => void, deliver!: (m: SeedMapResult) => void;
    const off = vi.fn(),
      apply = vi.fn(),
      cancel = vi.fn(async () => ({ cancelled: true }));
    const job = runQuery(args, controller.signal, apply, {
      listen: async () => off,
      start: (_, fn) => {
        deliver = fn;
        return new Promise((r) => {
          resolve = r;
        });
      },
      cancel,
    });
    await vi.waitFor(() => expect(resolve).toBeDefined());
    controller.abort();
    await job;
    deliver(msg);
    resolve({ task_id: "late" });
    await vi.waitFor(() => expect(cancel).toHaveBeenCalledWith({ task_id: "late" }));
    expect(apply).not.toHaveBeenCalled();
    expect(off).toHaveBeenCalledOnce();
  });
  it("propagates background failures without waiting for a result", async () => {
    const error = { code: "SEED_INVALID_INPUT" as const, params: {}, detail: null };
    let end!: (t: TaskFinished) => void;
    await expect(
      runQuery(args, new AbortController().signal, vi.fn(), {
        listen: async (fn) => {
          end = fn;
          return () => undefined;
        },
        start: async () => {
          end({ task_id: "x", state: "failed", error });
          return { task_id: "x" };
        },
        cancel: vi.fn(),
      }),
    ).rejects.toEqual(error);
  });
});
