import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import { resourcesImport } from "./resources";

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
});
