import { describe, expect, it } from "vitest";
import type { ResourceHit } from "../../api/resources";
import { hitLoaders, incompatibility, releaseVersions, versionSummary } from "./targets";

function hit(categories: string[], game_versions: string[]): ResourceHit {
  return {
    project_id: "p",
    slug: "p",
    title: "P",
    description: "",
    author: "a",
    icon_url: null,
    downloads: 0,
    follows: 0,
    categories,
    game_versions,
    date_modified: "2026-10-01T00:00:00Z",
  };
}

const fabric = { kind: "fabric" as const, version: "0.16.5" };

describe("game versions", () => {
  it("keeps releases, newest first", () => {
    expect(releaseVersions(["1.20.1", "23w31a", "1.9", "1.21", "26.3", "1.20.1"])).toEqual([
      "26.3",
      "1.21",
      "1.20.1",
      "1.9",
    ]);
  });

  it("summarizes the newest few", () => {
    expect(versionSummary(["1.19.2", "1.20.1", "1.21", "1.21.1"])).toEqual({
      latest: ["1.21.1", "1.21", "1.20.1"],
      total: 4,
    });
    expect(versionSummary(["24w14a"])).toEqual({ latest: ["24w14a"], total: 1 });
    expect(versionSummary([])).toEqual({ latest: [], total: 0 });
  });
});

describe("loaders", () => {
  it("reads loaders from categories by kind", () => {
    const h = hit(["optimization", "fabric", "quilt", "iris"], []);
    expect(hitLoaders(h, "mod")).toEqual(["fabric", "quilt"]);
    expect(hitLoaders(h, "shader")).toEqual(["iris"]);
    expect(hitLoaders(h, "resourcepack")).toEqual([]);
  });
});

describe("incompatibility", () => {
  it("checks the game version first", () => {
    expect(
      incompatibility(hit(["fabric"], ["1.21"]), "mod", { game_version: "1.20.1", loader: fabric }),
    ).toEqual({ reason: "version", version: "1.20.1" });
  });

  it("rejects mods on vanilla or on another loader", () => {
    const h = hit(["forge"], ["1.20.1"]);
    expect(incompatibility(h, "mod", { game_version: "1.20.1", loader: null })).toEqual({
      reason: "noLoader",
    });
    expect(incompatibility(h, "mod", { game_version: "1.20.1", loader: fabric })).toEqual({
      reason: "loader",
      loaders: ["forge"],
    });
  });

  it("lets quilt instances use fabric mods", () => {
    const quilt = { kind: "quilt" as const, version: "0.26.0" };
    expect(
      incompatibility(hit(["fabric"], ["1.20.1"]), "mod", {
        game_version: "1.20.1",
        loader: quilt,
      }),
    ).toBeNull();
  });

  it("does not check loaders for packs and shaders", () => {
    const vanilla = { game_version: "1.20.1", loader: null };
    expect(incompatibility(hit([], ["1.20.1"]), "resourcepack", vanilla)).toBeNull();
    expect(incompatibility(hit(["iris"], []), "shader", vanilla)).toBeNull();
  });
});
