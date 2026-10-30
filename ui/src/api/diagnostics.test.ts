import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import { diagnosticsExport } from "./diagnostics";

describe("diagnostics commands", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    tauriInvoke.mockResolvedValue({ files: ["system.json"] });
  });

  it("invokes diagnostics_export with args", async () => {
    const result = await diagnosticsExport({ dest: "/tmp/diag.zip" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("diagnostics_export", {
      args: { dest: "/tmp/diag.zip" },
    });
    expect(result.files).toEqual(["system.json"]);
  });
});
