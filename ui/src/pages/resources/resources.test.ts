import { describe, expect, it } from "vitest";
import type { InstallPlan, InstalledResource, ResourceVersion } from "../../api/resources";
import {
  defaultVersion,
  displayName,
  filterInstalled,
  formatSize,
  kindsFor,
  planBlocked,
  planSize,
} from "./resources";

function version(id: string, channel: ResourceVersion["channel"]): ResourceVersion {
  return {
    id,
    project_id: "p",
    name: id,
    version_number: id,
    channel,
    date_published: "2026-10-01T00:00:00Z",
    downloads: 0,
    game_versions: ["1.20.1"],
    loaders: ["fabric"],
    file_name: `${id}.jar`,
    file_size: 1,
  };
}

function installed(file_name: string, title?: string): InstalledResource {
  return {
    file_name,
    enabled: !file_name.endsWith(".disabled"),
    size: 1,
    sha1: "",
    source: title
      ? { project_id: "p", version_id: "v", version_number: "1", title, icon_url: null }
      : null,
  };
}

const emptyPlan: InstallPlan = { files: [], conflicts: [], unavailable: [] };

describe("resources page helpers", () => {
  it("hides mods for vanilla instances", () => {
    expect(kindsFor(null)).toEqual(["resourcepack", "shader", "datapack"]);
    expect(kindsFor({ kind: "fabric", version: "0.16.0" })[0]).toBe("mod");
  });

  it("prefers the newest release", () => {
    expect(defaultVersion([version("b", "beta"), version("r", "release")])?.id).toBe("r");
    expect(defaultVersion([version("a", "alpha"), version("b", "beta")])?.id).toBe("a");
    expect(defaultVersion([])).toBeNull();
  });

  it("blocks plans with conflicts or missing dependencies", () => {
    expect(planBlocked(emptyPlan)).toBe(false);
    expect(
      planBlocked({
        ...emptyPlan,
        unavailable: [{ project_id: null, title: null, required_by: "p" }],
      }),
    ).toBe(true);
    expect(
      planBlocked({
        ...emptyPlan,
        conflicts: [
          {
            project_id: "a",
            title: "A",
            with_project_id: "b",
            with_title: "B",
            with_installed: true,
          },
        ],
      }),
    ).toBe(true);
  });

  it("sums plan sizes", () => {
    const file = {
      project_id: "p",
      version_id: "v",
      title: "P",
      version_number: "1",
      kind: "mod" as const,
      file_name: "p.jar",
      size: 100,
      required_by: null,
      replaces: null,
    };
    expect(planSize({ ...emptyPlan, files: [file, { ...file, size: 50 }] })).toBe(150);
  });

  it("names and filters installed resources", () => {
    const list = [installed("sodium.jar", "Sodium"), installed("mystery.jar.disabled")];
    expect(displayName(list[1])).toBe("mystery.jar");
    expect(filterInstalled(list, "sod")).toEqual([list[0]]);
    expect(filterInstalled(list, "MYST")).toEqual([list[1]]);
    expect(filterInstalled(list, " ")).toEqual(list);
  });

  it("formats sizes", () => {
    expect(formatSize(512)).toBe("512 B");
    expect(formatSize(1536)).toBe("1.5 KB");
    expect(formatSize(20 * 1024 * 1024)).toBe("20 MB");
  });
});
