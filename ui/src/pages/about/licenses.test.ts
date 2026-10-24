import { describe, expect, it } from "vitest";
import data from "./third-party-licenses.json";
import { filterPackages, type LicenseData, type ThirdPartyPackage } from "./licenses";

const pkg = (name: string, license: string): ThirdPartyPackage => ({
  name,
  version: "1.0.0",
  source: "cargo",
  license,
  url: "",
  texts: [],
});

describe("filterPackages", () => {
  const packages = [pkg("serde", "MIT OR Apache-2.0"), pkg("naive-ui", "MIT"), pkg("ring", "ISC")];

  it("returns everything for an empty query", () => {
    expect(filterPackages(packages, "  ")).toHaveLength(3);
  });

  it("matches names and licenses case-insensitively", () => {
    expect(filterPackages(packages, "SERDE").map((p) => p.name)).toEqual(["serde"]);
    expect(filterPackages(packages, "isc").map((p) => p.name)).toEqual(["ring"]);
    expect(filterPackages(packages, "mit")).toHaveLength(2);
  });
});

describe("third-party-licenses.json", () => {
  const licenses = data as LicenseData;

  it("lists both Rust and frontend dependencies", () => {
    const names = new Set(licenses.packages.map((p) => `${p.source}:${p.name}`));
    expect(names.has("cargo:tauri")).toBe(true);
    expect(names.has("npm:vue")).toBe(true);
  });

  it("only references existing license texts", () => {
    for (const p of licenses.packages) {
      for (const i of p.texts) expect(licenses.texts[i]).toBeTypeOf("string");
    }
  });

  it("does not list the launcher's own crates", () => {
    expect(licenses.packages.some((p) => p.name.startsWith("tmcl"))).toBe(false);
  });
});
