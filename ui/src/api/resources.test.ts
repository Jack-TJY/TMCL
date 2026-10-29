import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import { resourcesImport, resourcesModpackExport } from "./resources";

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
});
