import { describe, expect, it, vi } from "vitest";

const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler);
    return () => listeners.delete(name);
  }),
}));

import type { FabricApiFailed } from "../bindings/FabricApiFailed";
import { EVENTS, onFabricApiFailed } from "./events";

describe("onFabricApiFailed", () => {
  it("delivers the payload of loader:fabric-api-failed", async () => {
    const seen: FabricApiFailed[] = [];
    const unlisten = await onFabricApiFailed((failed) => seen.push(failed));
    const payload: FabricApiFailed = {
      task_id: "t",
      instance_id: "i",
      instance_name: "Jade",
      error: { code: "NET_CONNECT", params: {}, detail: null },
    };
    listeners.get(EVENTS.loaderFabricApiFailed)?.({ payload });
    expect(EVENTS.loaderFabricApiFailed).toBe("loader:fabric-api-failed");
    expect(seen).toEqual([payload]);
    unlisten();
    expect(listeners.has(EVENTS.loaderFabricApiFailed)).toBe(false);
  });
});
