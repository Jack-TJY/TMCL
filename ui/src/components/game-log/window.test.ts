import { describe, expect, it } from "vitest";
import { applyUpdate, linesSince } from "./bridge";
import { appendLines, type GameSession } from "./sessions";
import { isVisible, logWindowInstance, logWindowLabel, parseGeometry } from "./window";

function session(): GameSession {
  return {
    instanceId: "a",
    name: "a",
    status: "running",
    pid: 1,
    exitCode: null,
    error: null,
    lines: [],
    nextSeq: 0,
  };
}

describe("log window helpers", () => {
  it("builds a label from any instance ID", () => {
    expect(logWindowLabel("0b9c-41_x")).toBe("game-log-0b9c-41_x");
    expect(logWindowLabel("生存 1.20")).toBe("game-log-___1_20");
  });

  it("parses the instance ID from the hash", () => {
    expect(logWindowInstance("#/game-log/a%20b")).toBe("a b");
    expect(logWindowInstance("#/game-log/")).toBeNull();
    expect(logWindowInstance("#/home")).toBeNull();
  });

  it("validates saved geometry and enforces the minimum size", () => {
    expect(parseGeometry(null)).toBeNull();
    expect(parseGeometry("not json")).toBeNull();
    expect(parseGeometry('{"x":1,"y":2,"width":"3","height":4}')).toBeNull();
    expect(parseGeometry('{"x":-5,"y":2,"width":100,"height":900}')).toEqual({
      x: -5,
      y: 2,
      width: 480,
      height: 900,
    });
  });

  it("only restores positions that are on a monitor", () => {
    const monitors = [
      { x: 0, y: 0, width: 1920, height: 1080 },
      { x: -1280, y: 0, width: 1280, height: 1024 },
    ];
    const at = (x: number, y: number) => ({ x, y, width: 800, height: 600 });
    expect(isVisible(at(100, 100), monitors)).toBe(true);
    expect(isVisible(at(-1200, 50), monitors)).toBe(true);
    expect(isVisible(at(2500, 100), monitors)).toBe(false);
    expect(isVisible(at(100, 100), [])).toBe(false);
  });
});

describe("log window bridge", () => {
  it("returns the lines appended since a sequence number", () => {
    const s = session();
    appendLines(s, "stdout", ["a", "b", "c"]);
    expect(linesSince(s, 1).map((l) => l.text)).toEqual(["b", "c"]);
    expect(linesSince(s, 3)).toEqual([]);
    expect(linesSince(s, 0)).toHaveLength(3);
  });

  it("merges updates into the mirrored session without duplicates", () => {
    const source = session();
    appendLines(source, "stdout", ["a", "b"]);
    const mirror = structuredClone(source);
    appendLines(source, "stderr", ["c"]);
    applyUpdate(mirror, {
      meta: { name: "a", status: "crashed", pid: 1, exitCode: 1, error: null },
      lines: linesSince(source, 1),
    });
    expect(mirror.lines.map((l) => l.text)).toEqual(["a", "b", "c"]);
    expect(mirror.nextSeq).toBe(3);
    expect(mirror.status).toBe("crashed");
    expect(mirror.exitCode).toBe(1);
  });
});
