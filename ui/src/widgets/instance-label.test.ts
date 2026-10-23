import { describe, expect, it } from "vitest";
import { versionLabel } from "./instance-label";

describe("versionLabel", () => {
  it("adds the loader with its proper name", () => {
    expect(versionLabel({ game_version: "1.20.1", loader: null })).toBe("1.20.1");
    const loader = { kind: "neoforge" as const, version: "20.4.80" };
    expect(versionLabel({ game_version: "1.20.4", loader })).toBe("1.20.4 · NeoForge 20.4.80");
  });
});
