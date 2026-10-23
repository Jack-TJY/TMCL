import { describe, expect, it } from "vitest";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import { filterInstances, groupInstances, sortInstances } from "./instances";

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
  });
});
