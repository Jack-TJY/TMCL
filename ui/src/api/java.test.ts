import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import { javaList } from "./java";

describe("java commands", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
  });

  it("lists managed and system Java", async () => {
    const list = {
      javas: [
        {
          path: "C:/TMCL/tmcl-data/java/java-runtime-gamma/bin/java.exe",
          major: 17,
          version: "17.0.15",
          arch: "x64",
          vendor: "Microsoft",
          managed: true,
        },
      ],
    };
    tauriInvoke.mockResolvedValue(list);
    await expect(javaList()).resolves.toEqual(list);
    expect(tauriInvoke).toHaveBeenLastCalledWith("java_list", undefined);
  });
});
