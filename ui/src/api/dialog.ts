// 系统文件对话框（tauri-plugin-dialog）。只返回用户选中的路径，不读写文件。
import { documentDir, downloadDir, homeDir, join } from "@tauri-apps/api/path";
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

/** 选择一个或多个已有文件；取消时为空数组。 */
export async function pickFiles(filters: FileFilter[]): Promise<string[]> {
  const picked = await open({ multiple: true, directory: false, filters });
  if (Array.isArray(picked)) return picked;
  return typeof picked === "string" ? [picked] : [];
}

/** 选择一个文件夹；取消时为 `null`。 */
export async function pickFolder(): Promise<string | null> {
  const picked = await open({ multiple: false, directory: true });
  return typeof picked === "string" ? picked : null;
}

/** 选择保存位置；`defaultPath` 为建议的完整路径或文件名，取消时为 `null`。 */
export async function pickSavePath(
  filters: FileFilter[],
  defaultPath?: string,
): Promise<string | null> {
  return (await save({ filters, defaultPath })) ?? null;
}

/**
 * 把文件名放进用户的“下载”目录，取不到时依次退回“文档”与主目录；都取不到时原样返回文件名。
 * 用作 {@link pickSavePath} 的 `defaultPath`：只给文件名时部分平台会从根目录打开对话框。
 */
export async function inUserDir(fileName: string): Promise<string> {
  for (const dir of [downloadDir, documentDir, homeDir]) {
    try {
      return await join(await dir(), fileName);
    } catch {
      // 该目录在当前平台不存在，换下一个。
    }
  }
  return fileName;
}
