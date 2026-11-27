import { Channel } from "@tauri-apps/api/core";
import type { SeedMapArgs } from "../bindings/SeedMapArgs";
import type { SeedMapResult } from "../bindings/SeedMapResult";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** Start a viewport query with an isolated result stream. Subscribe to task
 * events before starting; cancel its task when the viewport is superseded. */
export function querySeedMap(
  args: SeedMapArgs,
  onResult: (message: SeedMapResult) => void,
): Promise<TaskStarted> {
  const results = new Channel<SeedMapResult>();
  results.onmessage = onResult;
  return invoke<TaskStarted>("seed_map_query", { args, results });
}
