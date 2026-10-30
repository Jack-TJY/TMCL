import { describe, expect, it } from "vitest";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import {
  canImportMods,
  copyName,
  filterInstances,
  formatArgs,
  groupInstances,
  groupNames,
  groupSkipped,
  joinPath,
  mostRecent,
  defaultInclude,
  packFileName,
  withPackExtension,
  parseArgs,
  sortInstances,
} from "./instances";

function instance(id: string, extra: Partial<InstanceSummary> = {}): InstanceSummary {
  return {
    id,
    name: id,
    game_version: "1.20.1",
    loader: null,
    group: null,
    created_at: "2026-10-22T00:00:00Z",
    last_played_at: null,
    error: null,
    ...extra,
  };
}

describe("instance list helpers", () => {
  const list = [
    instance("b", { group: "Mods", loader: { kind: "fabric", version: "0.16.5" } }),
    instance("a"),
    instance("c", { group: " Mods ", game_version: "1.12.2" }),
    instance("d", { group: "Adventure" }),
  ];

  it("filters by name, version, loader or group", () => {
    expect(filterInstances(list, "").map((i) => i.id)).toEqual(["b", "a", "c", "d"]);
    expect(filterInstances(list, "FABRIC").map((i) => i.id)).toEqual(["b"]);
    expect(filterInstances(list, "1.12").map((i) => i.id)).toEqual(["c"]);
    expect(filterInstances(list, "advent").map((i) => i.id)).toEqual(["d"]);
  });

  it("keeps backend order for recent and sorts by name otherwise", () => {
    expect(sortInstances(list, "recent")).toBe(list);
    expect(sortInstances(list, "name").map((i) => i.id)).toEqual(["a", "b", "c", "d"]);
  });

  it("groups by trimmed name with ungrouped last", () => {
    const groups = groupInstances(list);
    expect(groups.map((g) => g.name)).toEqual(["Adventure", "Mods", null]);
    expect(groups[1].instances.map((i) => i.id)).toEqual(["b", "c"]);
    expect(groupNames(list)).toEqual(["Adventure", "Mods"]);
  });
});

describe("most recent", () => {
  it("skips broken instances", () => {
    const broken = instance("x", { error: "INSTANCE_CORRUPTED" });
    expect(mostRecent([broken, instance("a"), instance("b")])?.id).toBe("a");
    expect(mostRecent([broken])).toBeNull();
  });
});

describe("argument text", () => {
  it("splits on whitespace and honours quotes", () => {
    expect(parseArgs('  -Xss2m   -Dfoo="a b" ""  ')).toEqual(["-Xss2m", "-Dfoo=a b", ""]);
    expect(parseArgs("")).toEqual([]);
  });

  it("round-trips through formatArgs", () => {
    const args = ["-XX:+UseG1GC", "-Dname=a b", ""];
    expect(formatArgs(args)).toBe('-XX:+UseG1GC "-Dname=a b" ""');
    expect(parseArgs(formatArgs(args))).toEqual(args);
  });
});

describe("names and paths", () => {
  it("suggests a free copy name", () => {
    expect(copyName("生存", ["生存"])).toBe("生存 (2)");
    expect(copyName("a", ["a", "a (2)"])).toBe("a (3)");
  });

  it("makes a safe pack file name", () => {
    expect(packFileName('a/b:c*"d')).toBe("a_b_c__d.tmclpack");
    expect(packFileName("  ")).toBe("instance.tmclpack");
    expect(packFileName("生存", "mrpack")).toBe("生存.mrpack");
    expect(packFileName("生存", "curseforge")).toBe("生存.zip");
  });

  it("swaps the pack extension", () => {
    expect(withPackExtension("C:\\out\\a.tmclpack", "mrpack")).toBe("C:\\out\\a.mrpack");
    expect(withPackExtension("/out/a.MRPACK", "tmclpack")).toBe("/out/a.tmclpack");
    expect(withPackExtension("/out/a", "mrpack")).toBe("/out/a.mrpack");
    expect(withPackExtension("", "mrpack")).toBe("");
    expect(withPackExtension("/out/a.mrpack", "curseforge")).toBe("/out/a.zip");
    expect(withPackExtension("/out/a.zip", "mrpack")).toBe("/out/a.mrpack");
  });

  it("leaves saves out of modpacks by default", () => {
    const entries = ["config", "logs", "mods", "saves", "screenshots"];
    expect(defaultInclude(entries, "tmclpack")).toEqual(["config", "mods", "saves"]);
    expect(defaultInclude(entries, "mrpack")).toEqual(["config", "mods"]);
    expect(defaultInclude(entries, "curseforge")).toEqual(["config", "mods"]);
  });

  it("joins with the directory's separator", () => {
    expect(joinPath("C:\\TMCL\\tmcl-data", "a.tmclpack")).toBe("C:\\TMCL\\tmcl-data\\a.tmclpack");
    expect(joinPath("/opt/tmcl-data/", "a.tmclpack")).toBe("/opt/tmcl-data/a.tmclpack");
  });
});

describe("mod import", () => {
  it("is offered only for readable instances with a loader", () => {
    const fabric = { kind: "fabric" as const, version: "0.16.5" };
    expect(canImportMods(instance("a", { loader: fabric }))).toBe(true);
    expect(canImportMods(instance("b"))).toBe(false);
    expect(
      canImportMods(
        instance("c", {
          loader: fabric,
          error: { code: "COMMON_IO", params: {}, detail: null } as never,
        }),
      ),
    ).toBe(false);
  });

  it("groups skipped files by reason in order", () => {
    const skip = (file_name: string, reason: "exists" | "unsupported" | "not_found") => ({
      path: `/x/${file_name}`,
      file_name,
      reason,
    });
    expect(
      groupSkipped([
        skip("a.jar", "exists"),
        skip("b.txt", "unsupported"),
        skip("c.jar", "exists"),
      ]),
    ).toEqual([
      ["exists", ["a.jar", "c.jar"]],
      ["unsupported", ["b.txt"]],
    ]);
  });
});
