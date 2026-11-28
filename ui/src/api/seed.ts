import { Channel } from "@tauri-apps/api/core";
import type { SeedMapArgs } from "../bindings/SeedMapArgs";
import type { SeedMapResult } from "../bindings/SeedMapResult";
import type { SeedVersionInfo } from "../bindings/SeedVersionInfo";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** Query arguments; dimension defaults to the overworld in this wrapper. */
export type SeedMapQueryArgs = Omit<SeedMapArgs, "dimension"> & {
  dimension?: SeedMapArgs["dimension"];
};

/** Start a viewport query with an isolated result stream. Subscribe to task
 * events before starting; cancel its task when the viewport is superseded. */
/** Releases the pinned engine can predict, and their structure layers. */
export function seedVersions(): Promise<SeedVersionInfo[]> {
  return invoke<SeedVersionInfo[]>("seed_versions");
}

/** Start a viewport query with an isolated result stream. Subscribe to task
 * events before starting; cancel its task when the viewport is superseded. */
export function querySeedMap(
  args: SeedMapQueryArgs,
  onResult: (message: SeedMapResult) => void,
): Promise<TaskStarted> {
  const results = new Channel<SeedMapResult>();
  results.onmessage = onResult;
  return invoke<TaskStarted>("seed_map_query", {
    args: { dimension: "overworld", ...args },
    results,
  });
}
