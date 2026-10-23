// 任务中心状态（PLAN.md M1-12）：汇总 `task_list` 与 `task:progress` / `task:finished` 事件，
// 并记住本次运行中由界面发起的任务如何重新开始，用于“失败重试”。
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { onTaskFinished, onTaskProgress } from "../api/events";
import { toAppError } from "../api/invoke";
import { taskCancel, taskList } from "../api/task";
import type { TaskFinished } from "../bindings/TaskFinished";
import type { TaskId } from "../bindings/TaskId";
import type { TaskInfo } from "../bindings/TaskInfo";
import type { TaskProgress } from "../bindings/TaskProgress";
import type { TaskStarted } from "../bindings/TaskStarted";

/** 任务中心的一项：后端的 `TaskInfo` 加上界面附带的信息。 */
export interface TaskEntry extends TaskInfo {
  /** 显示名称，例如实例名；后端列表中读到的任务为 `null`，界面按种类显示。 */
  label: string | null;
  /** 可以重试时为重新开始任务的函数。 */
  restart: (() => Promise<TaskStarted>) | null;
}

/** 由界面发起的任务。 */
export interface TaskRequest {
  kind: string;
  label: string | null;
  /** 调用对应命令并返回任务 ID；重试时会再次调用。 */
  start: () => Promise<TaskStarted>;
}

function blank(taskId: TaskId, kind: string): TaskEntry {
  return {
    task_id: taskId,
    kind,
    status: "running",
    started_at: Date.now(),
    finished_at: null,
    progress: null,
    error: null,
    label: null,
    restart: null,
  };
}

export const useTasksStore = defineStore("tasks", () => {
  const entries = ref<TaskEntry[]>([]);
  /** 事件先于 `start` 返回到达时，暂存的结束事件。 */
  const early = new Map<TaskId, TaskFinished>();
  let initialized: Promise<void> | null = null;

  const running = computed(() => entries.value.filter((e) => e.status === "running"));

  /** 所有运行中任务的总体进度（0–1）；没有可计算的进度时为 `null`。 */
  const overallProgress = computed(() => {
    let done = 0;
    let total = 0;
    for (const { progress } of running.value) {
      if (!progress) continue;
      if (progress.bytes_total > 0) {
        done += progress.bytes_done / progress.bytes_total;
        total += 1;
      } else if (progress.total > 0) {
        done += progress.done / progress.total;
        total += 1;
      }
    }
    return total === 0 ? null : done / total;
  });

  function find(taskId: TaskId): TaskEntry | undefined {
    return entries.value.find((e) => e.task_id === taskId);
  }

  function applyProgress(progress: TaskProgress) {
    let entry = find(progress.task_id);
    if (!entry) {
      entries.value.unshift(blank(progress.task_id, progress.kind));
      entry = entries.value[0];
    }
    if (entry.status === "running") entry.progress = progress;
  }

  function applyFinished(finished: TaskFinished) {
    const entry = find(finished.task_id);
    if (!entry) {
      early.set(finished.task_id, finished);
      return;
    }
    entry.status = finished.state;
    entry.finished_at = Date.now();
    entry.error = finished.error ?? null;
  }

  /** 订阅事件并读取已有任务；重复调用只执行一次。没有后端时保持空列表。 */
  function init(): Promise<void> {
    initialized ??= (async () => {
      try {
        await Promise.all([onTaskProgress(applyProgress), onTaskFinished(applyFinished)]);
        const { tasks } = await taskList();
        const known = new Set(entries.value.map((e) => e.task_id));
        for (const task of tasks) {
          if (!known.has(task.task_id)) entries.value.push({ ...task, label: null, restart: null });
        }
      } catch {
        // 在浏览器中调试时没有后端。
      }
    })();
    return initialized;
  }

  /** 发起任务并加入任务中心；命令本身失败时抛出 AppError，不产生任务。 */
  async function start(request: TaskRequest): Promise<TaskId> {
    await init();
    const { task_id } = await request.start().catch((err: unknown) => {
      throw toAppError(err);
    });
    let entry = find(task_id);
    if (!entry) {
      entries.value.unshift(blank(task_id, request.kind));
      entry = entries.value[0];
    }
    entry.label = request.label;
    entry.restart = request.start;
    const finished = early.get(task_id);
    if (finished) {
      early.delete(task_id);
      applyFinished(finished);
    }
    return task_id;
  }

  /** 重新开始失败或已取消的任务，原记录从列表移除；不能重试时返回 `null`。 */
  async function retry(taskId: TaskId): Promise<TaskId | null> {
    const entry = find(taskId);
    if (!entry?.restart || entry.status === "running") return null;
    const id = await start({ kind: entry.kind, label: entry.label, start: entry.restart });
    dismiss(taskId);
    return id;
  }

  async function cancel(taskId: TaskId): Promise<boolean> {
    const { cancelled } = await taskCancel({ task_id: taskId });
    return cancelled;
  }

  /** 从列表中移除一个已结束的任务。 */
  function dismiss(taskId: TaskId) {
    entries.value = entries.value.filter((e) => e.task_id !== taskId || e.status === "running");
  }

  /** 移除所有已结束的任务。 */
  function clearFinished() {
    entries.value = entries.value.filter((e) => e.status === "running");
  }

  return {
    entries,
    running,
    overallProgress,
    init,
    start,
    retry,
    cancel,
    dismiss,
    clearFinished,
  };
});
