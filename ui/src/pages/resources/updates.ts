// 检查更新与更新：按文件名记下可更新的资源，单个或全部更新以 resources_update 任务进行。
import { useMessage } from "naive-ui";
import { ref, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toAppError } from "../../api/invoke";
import {
  resourcesCheckUpdates,
  resourcesUpdate,
  RESOURCES_UPDATE_KIND,
  type ResourceKind,
  type ResourceUpdate,
} from "../../api/resources";
import type { TaskId } from "../../bindings/TaskId";
import { formatError } from "../../i18n";
import { useTasksStore } from "../../stores/tasks";

/** 当前检查的目标：实例、种类与数据包所在的存档。 */
export interface UpdateTarget {
  instanceId: string;
  instanceName: string;
  kind: ResourceKind;
  world: string | null;
}

export function useResourceUpdates(target: Ref<UpdateTarget | null>) {
  const { t } = useI18n();
  const message = useMessage();
  const tasks = useTasksStore();

  /** 文件名 → 可更新到的版本；`null` 表示还没检查。 */
  const updates = ref<Map<string, ResourceUpdate> | null>(null);
  const checking = ref(false);
  let seq = 0;

  function args(current: UpdateTarget) {
    return {
      instance_id: current.instanceId,
      kind: current.kind,
      world: current.world ?? undefined,
    };
  }

  function reset() {
    seq += 1;
    updates.value = null;
    checking.value = false;
  }

  async function check() {
    const current = target.value;
    if (!current) return;
    const id = ++seq;
    checking.value = true;
    try {
      const list = await resourcesCheckUpdates(args(current));
      if (id === seq) updates.value = new Map(list.map((u) => [u.file_name, u]));
    } catch (err) {
      if (id === seq) message.error(formatError(toAppError(err)));
    } finally {
      if (id === seq) checking.value = false;
    }
  }

  /** 更新指定的文件，省略时更新全部；返回任务 ID，失败时为 `null`。 */
  async function update(fileNames?: string[]): Promise<TaskId | null> {
    const current = target.value;
    const pending = updates.value;
    if (!current || !pending || pending.size === 0) return null;
    const names = fileNames ?? [...pending.keys()];
    const label =
      names.length === 1
        ? `${pending.get(names[0])?.title ?? names[0]} → ${current.instanceName}`
        : `${names.length} × ${t(`resources.kind.${current.kind}`)} → ${current.instanceName}`;
    try {
      const taskId = await tasks.start({
        kind: RESOURCES_UPDATE_KIND,
        label,
        start: () => resourcesUpdate({ ...args(current), file_names: fileNames }),
        instanceId: current.instanceId,
      });
      // 已交给任务的文件不再显示更新按钮，任务结束后由页面重新读取列表。
      const rest = new Map(pending);
      for (const name of names) rest.delete(name);
      updates.value = rest;
      message.success(t("resources.updates.started", { count: names.length }));
      return taskId;
    } catch (err) {
      message.error(formatError(toAppError(err)));
      return null;
    }
  }

  return { updates, checking, check, update, reset };
}
