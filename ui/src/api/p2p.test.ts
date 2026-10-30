import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import { isValidNode, p2pInfo } from "./p2p";

describe("p2p api", () => {
  beforeEach(() => tauriInvoke.mockReset());

  it("p2pInfo 调用 p2p_info", async () => {
    tauriInvoke.mockResolvedValue({ easytier_version: "v2.6.4", default_nodes: ["tcp://a:1"] });
    await expect(p2pInfo()).resolves.toEqual({
      easytier_version: "v2.6.4",
      default_nodes: ["tcp://a:1"],
    });
    expect(tauriInvoke).toHaveBeenCalledWith("p2p_info", undefined);
  });

  it("isValidNode 与后端规则一致", () => {
    for (const ok of [
      "tcp://public.example.com:11010",
      "udp://[2001:db8::1]:11010",
      "wss://node.example.com/path",
      "https://etnode.zkitefly.eu.org/node1",
    ]) {
      expect(isValidNode(ok), ok).toBe(true);
    }
    for (const bad of [
      "",
      "example.com:11010",
      "ftp://example.com",
      "tcp://",
      " tcp://a:1",
      "tcp://a b:1",
      "tcp://user:pass@a:1",
    ]) {
      expect(isValidNode(bad), bad).toBe(false);
    }
  });
});
