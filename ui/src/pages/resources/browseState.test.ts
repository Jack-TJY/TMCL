import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import { initialTarget, useResourceBrowseState } from "./browseState";

describe("resource browse state", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("keeps the selection for the session", () => {
    const state = useResourceBrowseState();
    expect([state.instanceId, state.kind, state.platform]).toEqual([null, "mod", "all"]);
    state.instanceId = "i1";
    state.kind = "shader";
    state.platform = "curseforge";
    const again = useResourceBrowseState();
    expect([again.instanceId, again.kind, again.platform]).toEqual(["i1", "shader", "curseforge"]);
  });

  it("prefers the address over the remembered selection", () => {
    const remembered = { instanceId: "old", kind: "shader" as const };
    expect(initialTarget({}, remembered)).toEqual({ instanceId: "old", kind: "shader" });
    expect(initialTarget({ instance: "new", kind: "resourcepack" }, remembered)).toEqual({
      instanceId: "new",
      kind: "resourcepack",
    });
    // 不是本页的种类、重复的查询参数都被忽略。
    expect(initialTarget({ instance: ["a", "b"], kind: "datapack" }, remembered)).toEqual(
      remembered,
    );
  });
});
