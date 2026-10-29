import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import {
  skinFileRead,
  skinLibraryAddTexture,
  skinLibraryList,
  skinSetCape,
  skinTexture,
} from "./skin";

describe("skin commands", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
  });

  it("unwraps the library list", async () => {
    const entry = { id: "a", name: "Steve", variant: "classic", height: 64, added_at: 1 };
    tauriInvoke.mockResolvedValue({ skins: [entry] });
    expect(await skinLibraryList()).toEqual([entry]);
    expect(tauriInvoke).toHaveBeenLastCalledWith("skin_library_list", undefined);
  });

  it("passes args to binary and profile commands", async () => {
    tauriInvoke.mockResolvedValue(new ArrayBuffer(8));
    await skinFileRead({ path: "C:\\skin.png" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("skin_file_read", {
      args: { path: "C:\\skin.png" },
    });
    const url = "http://textures.minecraft.net/texture/ab";
    await skinTexture({ url });
    expect(tauriInvoke).toHaveBeenLastCalledWith("skin_texture", { args: { url } });
    await skinLibraryAddTexture({ url, name: "Steve", variant: "slim" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("skin_library_add_texture", {
      args: { url, name: "Steve", variant: "slim" },
    });
    tauriInvoke.mockResolvedValue({ id: "u", name: "Steve", skins: [], capes: [] });
    await skinSetCape({ account_id: "acc" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("skin_set_cape", { args: { account_id: "acc" } });
  });

  it("rejects with AppError", async () => {
    const error = { code: "SKIN_INVALID_IMAGE", params: {}, detail: null };
    tauriInvoke.mockRejectedValue(error);
    await expect(skinFileRead({ path: "x.png" })).rejects.toEqual(error);
  });
});
