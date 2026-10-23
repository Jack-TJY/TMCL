import { describe, expect, it } from "vitest";
import type { GameVersion } from "../../bindings/GameVersion";
import { defaultInstanceName, filterOf, filterVersions } from "./versions";

const v = (id: string, kind: string): GameVersion => ({ id, kind, release_time: "" });

const VERSIONS = [
  v("23w31a", "snapshot"),
  v("1.20.1", "release"),
  v("1.20", "release"),
  v("1.19.4", "release"),
  v("b1.7.3", "old_beta"),
  v("rd-132211", "old_alpha"),
  v("weird", "other"),
];

describe("filterVersions", () => {
  it("groups version kinds", () => {
    expect(filterOf(v("x", "old_alpha"))).toBe("old");
    expect(filterOf(v("x", "other"))).toBe("snapshot");
  });

  it("filters by kind and keeps order", () => {
    expect(filterVersions(VERSIONS, "release", "").map((x) => x.id)).toEqual([
      "1.20.1",
      "1.20",
      "1.19.4",
    ]);
    expect(filterVersions(VERSIONS, "old", "").map((x) => x.id)).toEqual(["b1.7.3", "rd-132211"]);
  });

  it("searches case-insensitively within the filter", () => {
    expect(filterVersions(VERSIONS, "release", " 1.20 ").map((x) => x.id)).toEqual([
      "1.20.1",
      "1.20",
    ]);
    expect(filterVersions(VERSIONS, "snapshot", "W31").map((x) => x.id)).toEqual(["23w31a"]);
  });
});

describe("defaultInstanceName", () => {
  it("uses the version id or adds a counter", () => {
    expect(defaultInstanceName("1.20.1", [])).toBe("1.20.1");
    expect(defaultInstanceName("1.20.1", ["1.20.1", "1.20.1 (2)"])).toBe("1.20.1 (3)");
  });
});
