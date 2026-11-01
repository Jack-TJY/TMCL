import { beforeEach, describe, expect, it, vi } from "vitest";

const path = vi.hoisted(() => ({
  downloadDir: vi.fn(),
  documentDir: vi.fn(),
  homeDir: vi.fn(),
  join: vi.fn(async (...parts: string[]) => parts.join("/")),
}));
vi.mock("@tauri-apps/api/path", () => path);
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

import { inUserDir } from "./dialog";

describe("inUserDir", () => {
  beforeEach(() => {
    path.downloadDir.mockReset();
    path.documentDir.mockReset();
    path.homeDir.mockReset();
  });

  it("prefers the Downloads directory", async () => {
    path.downloadDir.mockResolvedValue("C:/Users/me/Downloads");
    await expect(inUserDir("a.zip")).resolves.toBe("C:/Users/me/Downloads/a.zip");
  });

  it("falls back to Documents, then home", async () => {
    path.downloadDir.mockRejectedValue(new Error("unknown path"));
    path.documentDir.mockResolvedValue("/home/me/Documents");
    await expect(inUserDir("a.zip")).resolves.toBe("/home/me/Documents/a.zip");

    path.documentDir.mockRejectedValue(new Error("unknown path"));
    path.homeDir.mockResolvedValue("/home/me");
    await expect(inUserDir("a.zip")).resolves.toBe("/home/me/a.zip");
  });

  it("returns the bare file name when no directory is known", async () => {
    for (const dir of [path.downloadDir, path.documentDir, path.homeDir]) {
      dir.mockRejectedValue(new Error("unknown path"));
    }
    await expect(inUserDir("a.zip")).resolves.toBe("a.zip");
  });
});
