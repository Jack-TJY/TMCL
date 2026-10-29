import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import {
  resourcesCheckUpdates,
  resourcesImport,
  resourcesInstall,
  resourcesInstalled,
  resourcesInstallPlan,
  resourcesModpackExport,
  resourcesModpackInspect,
  resourcesModpackInstall,
  resourcesModpackSearch,
  resourcesModpackVersions,
  resourcesRemove,
  resourcesSearch,
  resourcesSetEnabled,
  resourcesUpdate,
  resourcesVersions,
  resourcesWorlds,
} from "./resources";

describe("resources commands", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    tauriInvoke.mockResolvedValue({ imported: [], skipped: [] });
  });

  it("invokes resources_import with args", async () => {
    const args = { instance_id: "a", kind: "mod" as const, paths: ["/x/a.jar"] };
    await resourcesImport(args);
    expect(tauriInvoke).toHaveBeenLastCalledWith("resources_import", { args });
  });

  it("invokes resources_modpack_export with args", async () => {
    const args = { instance_id: "a", dest: "/x/a.mrpack", include: ["mods"], version: "1.0" };
    await resourcesModpackExport(args);
    expect(tauriInvoke).toHaveBeenLastCalledWith("resources_modpack_export", { args });
  });

  it("passes args through to search, plan, install, installed and remove", async () => {
    const dir = { instance_id: "a", kind: "datapack" as const, world: "w" };
    const install = { instance_id: "a", kind: "mod" as const, version_id: "v" };
    const calls: [string, () => Promise<unknown>, unknown][] = [
      ["resources_search", () => resourcesSearch({ ...dir, query: "x" }), { ...dir, query: "x" }],
      ["resources_install_plan", () => resourcesInstallPlan(install), install],
      ["resources_install", () => resourcesInstall(install), install],
      ["resources_installed", () => resourcesInstalled(dir), dir],
      [
        "resources_remove",
        () => resourcesRemove({ ...dir, file_name: "a.zip" }),
        { ...dir, file_name: "a.zip" },
      ],
    ];
    for (const [command, call, args] of calls) {
      await call();
      expect(tauriInvoke).toHaveBeenLastCalledWith(command, { args });
    }
  });

  it("unwraps list results", async () => {
    tauriInvoke.mockResolvedValueOnce({ versions: [{ id: "v" }] });
    expect(await resourcesVersions({ instance_id: "a", kind: "mod", project_id: "p" })).toEqual([
      { id: "v" },
    ]);
    tauriInvoke.mockResolvedValueOnce({ worlds: ["New World"] });
    expect(await resourcesWorlds({ instance_id: "a" })).toEqual(["New World"]);
    tauriInvoke.mockResolvedValueOnce({ file_name: "a.jar.disabled" });
    expect(
      await resourcesSetEnabled({
        instance_id: "a",
        kind: "mod",
        file_name: "a.jar",
        enabled: false,
      }),
    ).toBe("a.jar.disabled");
  });

  it("covers updates and modpacks", async () => {
    const dir = { instance_id: "a", kind: "mod" as const };
    tauriInvoke.mockResolvedValueOnce({ updates: [{ file_name: "a.jar" }] });
    expect(await resourcesCheckUpdates(dir)).toEqual([{ file_name: "a.jar" }]);
    expect(tauriInvoke).toHaveBeenLastCalledWith("resources_check_updates", { args: dir });
    const update = { ...dir, file_names: ["a.jar"] };
    await resourcesUpdate(update);
    expect(tauriInvoke).toHaveBeenLastCalledWith("resources_update", { args: update });
    await resourcesModpackSearch({ query: "x" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("resources_modpack_search", {
      args: { query: "x" },
    });
    tauriInvoke.mockResolvedValueOnce({ versions: [] });
    expect(await resourcesModpackVersions({ project_id: "p" })).toEqual([]);
    await resourcesModpackInspect({ path: "/a.mrpack" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("resources_modpack_inspect", {
      args: { path: "/a.mrpack" },
    });
    const install = { source: { kind: "modrinth" as const, version_id: "v" }, name: "Pack" };
    await resourcesModpackInstall(install);
    expect(tauriInvoke).toHaveBeenLastCalledWith("resources_modpack_install", { args: install });
  });
});
