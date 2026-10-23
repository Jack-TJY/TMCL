import { describe, expect, it } from "vitest";
import type { TaskProgress } from "../../bindings/TaskProgress";
import { errorDetails, formatBytes, progressPercent } from "./format";

const base: TaskProgress = {
  task_id: "t",
  kind: "game_install",
  stage: "assets",
  done: 0,
  total: 0,
  bytes_done: 0,
  bytes_total: 0,
  speed_bps: 0,
};

describe("formatBytes", () => {
  it("picks a binary unit", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(150 * 1024 * 1024)).toBe("150 MB");
    expect(formatBytes(3 * 1024 ** 3)).toBe("3.0 GB");
  });
});

describe("progressPercent", () => {
  it("prefers bytes, then items", () => {
    expect(progressPercent({ ...base, bytes_done: 25, bytes_total: 100, done: 9, total: 10 })).toBe(
      25,
    );
    expect(progressPercent({ ...base, done: 1, total: 4 })).toBe(25);
  });

  it("is null when the total is unknown", () => {
    expect(progressPercent(null)).toBeNull();
    expect(progressPercent(base)).toBeNull();
  });
});

describe("errorDetails", () => {
  it("includes code, params and detail", () => {
    const text = errorDetails("连接失败", {
      code: "NET_CONNECT",
      params: { host: "example.com" },
      detail: "dns error",
    });
    expect(text).toBe(
      '连接失败\ncode: NET_CONNECT\nparams: {"host":"example.com"}\ndetail: dns error',
    );
  });

  it("omits empty parts", () => {
    expect(errorDetails("x", { code: "NET_TIMEOUT", params: {}, detail: null })).toBe(
      "x\ncode: NET_TIMEOUT",
    );
  });
});
