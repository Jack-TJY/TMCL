import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import type { InstallPlan, ResourceHit, ResourceVersion } from "../../api/resources";
import type { TaskRequest } from "../../stores/tasks";
import { autoInstall } from "./autoInstall";

const hit: ResourceHit = {
  platform: "curseforge",
  project_id: "238222",
  slug: "jei",
  title: "JEI",
  description: "",
  author: "mezz",
  icon_url: null,
  downloads: 0,
  follows: 0,
  categories: [],
  game_versions: ["1.20.1"],
  date_modified: "2026-10-01T00:00:00Z",
  also_on: [],
};

function version(id: string, channel: ResourceVersion["channel"]): ResourceVersion {
  return {
    id,
    project_id: hit.project_id,
    name: id,
    version_number: id,
    channel,
    date_published: "2026-10-01T00:00:00Z",
    downloads: 0,
    game_versions: ["1.20.1"],
    loaders: ["forge"],
    file_name: `${id}.jar`,
    file_size: 1,
  };
}

const okPlan = {
  files: [],
  replaces: [],
  conflicts: [],
  unavailable: [],
} as unknown as InstallPlan;
const target = { hit, kind: "mod" as const, instanceId: "i1", instanceName: "Forge 1.20.1" };

function route(versions: ResourceVersion[], plan: InstallPlan) {
  tauriInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === "resources_versions") return { versions };
    if (cmd === "resources_install_plan") return plan;
    if (cmd === "resources_install") return { task_id: "install" };
    throw new Error(`unexpected ${cmd}`);
  });
}

describe("autoInstall", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
  });

  it("installs the latest release from the hit's platform", async () => {
    route([version("beta", "beta"), version("rel", "release")], okPlan);
    const start = vi.fn(async (req: TaskRequest) => (await req.start()).task_id);
    const result = await autoInstall(target, start);
    expect(result).toEqual({ state: "started", taskId: "install" });
    expect(tauriInvoke).toHaveBeenCalledWith("resources_install", {
      args: { instance_id: "i1", kind: "mod", version_id: "rel", platform: "curseforge" },
    });
    expect(start.mock.calls[0][0]).toMatchObject({ kind: "resources_install", instanceId: "i1" });
  });

  it("asks for the dialog when there is no version or the plan is blocked", async () => {
    const start = vi.fn();
    route([], okPlan);
    expect(await autoInstall(target, start)).toEqual({ state: "needs_dialog" });
    const blocked = { ...okPlan, conflicts: [{}] } as unknown as InstallPlan;
    route([version("rel", "release")], blocked);
    expect(await autoInstall(target, start)).toEqual({ state: "needs_dialog" });
    expect(start).not.toHaveBeenCalled();
  });

  it("reports command errors", async () => {
    tauriInvoke.mockRejectedValue({ code: "NET_TIMEOUT", params: {}, detail: "x" });
    const result = await autoInstall(target, vi.fn());
    expect(result).toMatchObject({ state: "failed", error: { code: "NET_TIMEOUT" } });
  });
});
