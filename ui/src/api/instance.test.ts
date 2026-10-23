import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import * as api from "./instance";

describe("instance commands", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    tauriInvoke.mockResolvedValue({});
  });

  it("invokes argument-less commands without args", async () => {
    await api.instanceList();
    expect(tauriInvoke).toHaveBeenLastCalledWith("instance_list", undefined);
    await api.instanceRefresh();
    expect(tauriInvoke).toHaveBeenLastCalledWith("instance_refresh", undefined);
  });

  it("wraps arguments in args", async () => {
    const calls: [string, () => Promise<unknown>, unknown][] = [
      ["instance_get", () => api.instanceGet({ id: "a" }), { id: "a" }],
      ["instance_delete", () => api.instanceDelete({ id: "a" }), { id: "a" }],
      ["instance_game_entries", () => api.instanceGameEntries({ id: "a" }), { id: "a" }],
      [
        "instance_create",
        () => api.instanceCreate({ name: "n", game_version: "1.20.1" }),
        { name: "n", game_version: "1.20.1" },
      ],
      ["instance_rename", () => api.instanceRename({ id: "a", name: "b" }), { id: "a", name: "b" }],
      [
        "instance_duplicate",
        () => api.instanceDuplicate({ id: "a", name: "b", include_saves: true }),
        { id: "a", name: "b", include_saves: true },
      ],
      [
        "instance_export",
        () => api.instanceExport({ id: "a", dest: "/x.tmclpack", include: ["mods"] }),
        { id: "a", dest: "/x.tmclpack", include: ["mods"] },
      ],
      [
        "instance_import",
        () => api.instanceImport({ pack: "/x.tmclpack" }),
        { pack: "/x.tmclpack" },
      ],
    ];
    for (const [cmd, call, args] of calls) {
      await call();
      expect(tauriInvoke).toHaveBeenLastCalledWith(cmd, { args });
    }
  });
});
