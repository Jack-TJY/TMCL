import { onTaskFinished } from "../../api/events";
import { querySeedMap } from "../../api/seed";
import { taskCancel } from "../../api/task";
import type { SeedMapArgs } from "../../bindings/SeedMapArgs";
import type { SeedMapResult } from "../../bindings/SeedMapResult";
import type { TaskFinished } from "../../bindings/TaskFinished";
import { useTasksStore } from "../../stores/tasks";
const backend = {
  listen: onTaskFinished,
  cancel: taskCancel,
  start: async (args: SeedMapArgs, apply: (m: SeedMapResult) => void) => ({
    task_id: await useTasksStore().start({
      kind: "seed_map",
      label: null,
      transient: true,
      start: () => querySeedMap(args, apply),
    }),
  }),
};
/** A query completes only after both the requested messages and its success event.
 * The invoke reply, channel and task events are independent transports. */
export function runQuery(
  args: SeedMapArgs,
  signal: AbortSignal,
  apply: (m: SeedMapResult) => void,
  api = backend,
): Promise<void> {
  return new Promise((resolve, reject) => {
    let id: string | undefined,
      off: (() => void) | undefined,
      done = false,
      success = false;
    let remaining = args.query.type === "tiles" ? args.query.tiles.length : 1;
    const early = new Map<string, TaskFinished>();
    const cancel = (task_id: string) => void api.cancel({ task_id }).catch(() => {});
    const settle = (err?: unknown) => {
      if (done) return;
      done = true;
      off?.();
      early.clear();
      signal.removeEventListener("abort", abort);
      if (err === undefined) resolve();
      else reject(err);
    };
    const abort = () => {
      if (id) cancel(id);
      settle();
    };
    const check = () => {
      if (success && remaining === 0) settle();
    };
    const finished = (m: TaskFinished) => {
      if (done) return;
      if (!id) {
        early.set(m.task_id, m);
        return;
      }
      if (m.task_id !== id) return;
      if (m.state !== "succeeded") {
        settle(m.error ?? { code: "COMMON_CANCELLED", params: {}, detail: null });
        return;
      }
      success = true;
      check();
    };
    if (signal.aborted) {
      settle();
      return;
    }
    signal.addEventListener("abort", abort, { once: true });
    void (async () => {
      const stop = await api.listen(finished);
      if (done) {
        stop();
        return;
      }
      off = stop;
      const reply = await api.start(args, (m) => {
        if (done || remaining <= 0) return;
        try {
          apply(m);
          remaining--;
          check();
        } catch (e) {
          if (id) cancel(id);
          settle(e);
        }
      });
      id = reply.task_id;
      if (done) {
        cancel(id);
        return;
      }
      const m = early.get(id);
      early.clear();
      if (m) finished(m);
    })().catch(settle);
  });
}
