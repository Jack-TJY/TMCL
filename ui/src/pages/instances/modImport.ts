// 从本地导入资源：选择文件，复制进实例对应的资源目录（默认 mods/），并提示导入与跳过的结果。
import { useMessage } from "naive-ui";
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { pickFiles } from "../../api/dialog";
import { toAppError } from "../../api/invoke";
import { resourcesImport, type ResourceKind } from "../../api/resources";
import { formatError } from "../../i18n";
import { groupSkipped } from "./instances";

const MOD_FILTERS = [{ name: "Mod", extensions: ["jar", "zip"] }];
const ZIP_FILTERS = [{ name: "Zip", extensions: ["zip"] }];

export function useModImport() {
  const { t } = useI18n();
  const message = useMessage();
  /** 正在导入的实例 ID。 */
  const importing = ref<string | null>(null);

  /** 数据包需给出存档 `world`。返回是否导入了至少一个文件；取消选择或正在导入时什么都不做。 */
  async function importMods(
    instanceId: string,
    kind: ResourceKind = "mod",
    world?: string,
  ): Promise<boolean> {
    if (importing.value) return false;
    try {
      const paths = await pickFiles(kind === "mod" ? MOD_FILTERS : ZIP_FILTERS);
      if (paths.length === 0) return false;
      importing.value = instanceId;
      const result = await resourcesImport({ instance_id: instanceId, kind, world, paths });
      if (result.imported.length > 0) {
        const key = kind === "mod" ? "instances.mods.imported" : "resources.imported";
        message.success(t(key, { count: result.imported.length }));
      }
      for (const [reason, names] of groupSkipped(result.skipped)) {
        message.warning(t(`instances.mods.skipped.${reason}`, { names: names.join(", ") }), {
          duration: 6000,
        });
      }
      return result.imported.length > 0;
    } catch (err) {
      message.error(formatError(toAppError(err)));
      return false;
    } finally {
      importing.value = null;
    }
  }

  return { importing, importMods };
}
