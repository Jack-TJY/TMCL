import { describe, expect, it } from "vitest";
import type { AppError } from "../../bindings/AppError";
import type { LoaderVersion } from "../../bindings/LoaderVersion";
import { defaultVersion, toState, visibleVersions } from "./loaders";

const v = (version: string, stable = true, recommended = false): LoaderVersion => ({
  kind: "forge",
  game_version: "1.20.1",
  version,
  stable,
  recommended,
});

const error = (code: AppError["code"]): AppError => ({ code, params: {}, detail: null });

describe("toState", () => {
  it("treats empty lists and unsupported versions as incompatible", () => {
    expect(toState({ versions: [] })).toEqual({ status: "unsupported" });
    expect(toState(error("LOADER_UNSUPPORTED_VERSION"))).toEqual({ status: "unsupported" });
  });

  it("keeps other errors for retry", () => {
    const err = error("NET_TIMEOUT");
    expect(toState(err)).toEqual({ status: "error", error: err });
  });

  it("returns versions when available", () => {
    const versions = [v("47.4.0")];
    expect(toState({ versions })).toEqual({ status: "ready", versions });
  });
});

describe("defaultVersion", () => {
  it("prefers the recommended version, then the newest stable one", () => {
    expect(defaultVersion([v("2", false), v("1.5"), v("1", true, true)])?.version).toBe("1");
    expect(defaultVersion([v("2", false), v("1.5"), v("1")])?.version).toBe("1.5");
    expect(defaultVersion([v("2-beta", false)])?.version).toBe("2-beta");
    expect(defaultVersion([])).toBeNull();
  });
});

describe("visibleVersions", () => {
  const versions = [v("0.17.0", false), v("0.16.5", true, true), v("0.16.4")];
  const ids = (list: LoaderVersion[]) => list.map((x) => x.version);

  it("hides beta versions by default", () => {
    expect(ids(visibleVersions(versions, false, "0.16.5"))).toEqual(["0.16.5", "0.16.4"]);
  });

  it("shows everything when asked, when a beta is selected or when nothing is stable", () => {
    expect(visibleVersions(versions, true, null)).toHaveLength(3);
    expect(visibleVersions(versions, false, "0.17.0")).toHaveLength(3);
    expect(visibleVersions([v("1-beta", false)], false, null)).toHaveLength(1);
  });
});
