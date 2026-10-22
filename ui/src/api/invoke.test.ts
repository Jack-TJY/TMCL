import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import { appDemoTask } from "./app";
import { invoke, isAppError, toAppError } from "./invoke";

describe("invoke", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
  });

  it("returns the command result", async () => {
    tauriInvoke.mockResolvedValue({ task_id: "t1" });
    await expect(appDemoTask({ steps: 3, step_ms: 10 })).resolves.toEqual({ task_id: "t1" });
    expect(tauriInvoke).toHaveBeenCalledWith("app_demo_task", {
      args: { steps: 3, step_ms: 10 },
    });
  });

  it("passes a backend AppError through unchanged", async () => {
    const err = { code: "COMMON_IO", params: { path: "config.json" }, detail: null };
    tauriInvoke.mockRejectedValue(err);
    await expect(invoke("app_info")).rejects.toEqual(err);
  });

  it("wraps unstructured failures as COMMON_INTERNAL", async () => {
    tauriInvoke.mockRejectedValue("app_info not allowed");
    await expect(invoke("app_info")).rejects.toEqual({
      code: "COMMON_INTERNAL",
      params: {},
      detail: "app_info not allowed",
    });
  });
});

describe("isAppError", () => {
  it("rejects values without code and params", () => {
    expect(isAppError(null)).toBe(false);
    expect(isAppError({ code: "COMMON_IO" })).toBe(false);
    expect(toAppError(new Error("boom")).detail).toBe("boom");
  });
});
