import { describe, expect, it } from "vitest";
import { filterLines, logFileName, logText } from "./log";
import { appendLines, isActive, MAX_LOG_LINES, type GameSession, type LogLine } from "./sessions";

const lines: LogLine[] = [
  { seq: 0, stream: "stdout", text: "[main/INFO]: Hello" },
  { seq: 1, stream: "stderr", text: "java.lang.Error: boom" },
];

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

describe("log helpers", () => {
  it("filters case-insensitively", () => {
    expect(filterLines(lines, "")).toBe(lines);
    expect(filterLines(lines, " ERROR ").map((l) => l.seq)).toEqual([1]);
  });

  it("joins lines with a trailing newline", () => {
    expect(logText(lines)).toBe("[main/INFO]: Hello\njava.lang.Error: boom\n");
    expect(logText([])).toBe("");
  });

  it("builds a safe, timestamped file name", () => {
    const at = new Date(2026, 9, 3, 4, 5, 6);
    expect(logFileName("生存: 1.20/1", at)).toBe("生存_ 1.20_1-20261003-040506.log");
    expect(logFileName("  ", at)).toBe("game-20261003-040506.log");
  });
});

describe("game sessions", () => {
  it("numbers lines and keeps at most MAX_LOG_LINES", () => {
    const s = session();
    appendLines(s, "stdout", ["a", "b"]);
    expect(s.lines.map((l) => [l.seq, l.text])).toEqual([
      [0, "a"],
      [1, "b"],
    ]);
    appendLines(
      s,
      "stderr",
      Array.from({ length: MAX_LOG_LINES }, (_, i) => `x${i}`),
    );
    expect(s.lines).toHaveLength(MAX_LOG_LINES);
    expect(s.lines[0].text).toBe("x0");
    expect(s.lines.at(-1)?.seq).toBe(MAX_LOG_LINES + 1);
  });

  it("treats starting and running as active", () => {
    expect(isActive({ ...session(), status: "starting" })).toBe(true);
    expect(isActive({ ...session(), status: "running" })).toBe(true);
    expect(isActive({ ...session(), status: "crashed" })).toBe(false);
    expect(isActive(undefined)).toBe(false);
  });
});
