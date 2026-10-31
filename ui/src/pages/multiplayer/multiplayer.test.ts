import { describe, expect, it } from "vitest";
import type { SakuraNodeInfo } from "../../api/sakura";
import {
  isValidPlayerName,
  isValidPort,
  latencyLevel,
  nodeUsable,
  normalizeAddress,
  normalizeRoomCode,
  parsePrefs,
  sortNodes,
} from "./multiplayer";

function node(id: number, change: Partial<SakuraNodeInfo> = {}): SakuraNodeInfo {
  return {
    id,
    name: `#${id}`,
    host: `n${id}.test`,
    description: "",
    vip: 0,
    flag: 0,
    available: true,
    public_game: false,
    ...change,
  };
}

describe("isValidPort", () => {
  it("accepts 1-65535 integers only", () => {
    expect(isValidPort(25565)).toBe(true);
    expect(isValidPort(1)).toBe(true);
    expect(isValidPort(65535)).toBe(true);
    expect(isValidPort(0)).toBe(false);
    expect(isValidPort(65536)).toBe(false);
    expect(isValidPort(1.5)).toBe(false);
    expect(isValidPort(null)).toBe(false);
  });
});

describe("normalizeAddress", () => {
  it("accepts domain and IPv4 addresses with a port", () => {
    expect(normalizeAddress("  Frp-Abc.Example.com:10086 ")).toBe("frp-abc.example.com:10086");
    expect(normalizeAddress("1.2.3.4:25565")).toBe("1.2.3.4:25565");
    expect(normalizeAddress("host:080")).toBe("host:80");
  });

  it("rejects missing or invalid parts", () => {
    for (const bad of ["", "example.com", ":25565", "example.com:", "a b.com:1", "x.com:70000"])
      expect(normalizeAddress(bad)).toBeNull();
    expect(normalizeAddress("-bad.com:1")).toBeNull();
    expect(normalizeAddress("http://x.com:1")).toBeNull();
  });
});

describe("nodes", () => {
  it("only allows public-game nodes for public rooms", () => {
    expect(nodeUsable(node(1), false)).toBe(true);
    expect(nodeUsable(node(1), true)).toBe(false);
    expect(nodeUsable(node(1, { public_game: true }), true)).toBe(true);
    expect(nodeUsable(node(1, { available: false, public_game: true }), true)).toBe(false);
  });

  it("puts usable free nodes first", () => {
    const nodes = [
      node(1, { available: false }),
      node(2, { vip: 2 }),
      node(3),
      node(4, { public_game: true }),
    ];
    expect(sortNodes(nodes, false).map((n) => n.id)).toEqual([3, 4, 2, 1]);
    expect(sortNodes(nodes, true).map((n) => n.id)).toEqual([4, 1, 3, 2]);
  });
});

describe("latencyLevel", () => {
  it("grades latency", () => {
    expect(latencyLevel(null)).toBe("unreachable");
    expect(latencyLevel(30)).toBe("good");
    expect(latencyLevel(120)).toBe("fair");
    expect(latencyLevel(400)).toBe("poor");
  });
});

describe("parsePrefs", () => {
  it("falls back to defaults for missing or invalid fields", () => {
    expect(parsePrefs(null)).toEqual({ mode: "direct", role: "host", node: null, port: null });
    expect(parsePrefs("not json")).toEqual(parsePrefs(null));
    expect(parsePrefs('{"mode":"x","role":"member","node":3,"port":70000}')).toEqual({
      mode: "direct",
      role: "member",
      node: 3,
      port: null,
    });
    expect(parsePrefs('{"mode":"sakura","port":25565}').port).toBe(25565);
  });
});

describe("normalizeRoomCode", () => {
  // 后端 room.rs 测试中的规范示例。
  const VALID = "U/YNZE-U61D-2206-HXRG";

  it("accepts valid codes leniently", () => {
    expect(normalizeRoomCode(VALID)).toBe(VALID);
    expect(normalizeRoomCode("  u/ynze-u61d-2206-hxrg ")).toBe(VALID);
    expect(normalizeRoomCode("YNZE-U61D-2206-HXRG")).toBe(VALID);
  });

  it("rejects bad format, letters outside the alphabet and bad checksums", () => {
    expect(normalizeRoomCode("")).toBeNull();
    expect(normalizeRoomCode("U/YNZE-U61D-2206")).toBeNull();
    expect(normalizeRoomCode("U/YNZE-U61D-2206-HXRI")).toBeNull();
    expect(normalizeRoomCode("U/YNZE-U61D-2206-HXRH")).toBeNull();
  });
});

describe("isValidPlayerName", () => {
  it("requires 1-64 characters after trimming", () => {
    expect(isValidPlayerName("Steve")).toBe(true);
    expect(isValidPlayerName("   ")).toBe(false);
    expect(isValidPlayerName("a".repeat(64))).toBe(true);
    expect(isValidPlayerName("a".repeat(65))).toBe(false);
  });
});
