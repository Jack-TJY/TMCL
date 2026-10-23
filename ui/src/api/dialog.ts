// 系统文件对话框（tauri-plugin-dialog）。只返回用户选中的路径，不读写文件。
import { open, save } from "@tauri-apps/plugin-dialog";

/** 文件类型筛选；`extensions` 不带点。 */
export interface FileFilter {
  name: string;
  extensions: string[];
}

/** 选择一个已有文件；取消时为 `null`。 */
export async function pickFile(filters: FileFilter[]): Promise<string | null> {
  const picked = await open({ multiple: false, directory: false, filters });
  return typeof picked === "string" ? picked : null;
}

/** 选择保存位置；`defaultPath` 为建议的完整路径或文件名，取消时为 `null`。 */
export async function pickSavePath(
  filters: FileFilter[],
  defaultPath?: string,
): Promise<string | null> {
  return (await save({ filters, defaultPath })) ?? null;
}
