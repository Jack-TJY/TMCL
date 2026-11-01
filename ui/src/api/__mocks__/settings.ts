// 设置页尚无后端的部分（AGENTS.md §6.3）。对应命令就绪后用 `ui/src/api/` 中的真实封装替换调用方并删除本文件：
// - 下载源：M1-02 在 `config.json` 追加 `download.source` 后，由 `settings_get` / `settings_set` 读写。
// 在此之前下载源保存在 WebView 的 localStorage 中，只为让界面可用，不是持久化方案。

/** 下载源策略：`auto` 先镜像后官方，或指定某个源（PLAN.md M1-02）。 */
export type DownloadSource = "auto" | "official" | "bmclapi";

export const DEFAULT_SOURCE: DownloadSource = "auto";

const STORAGE_KEY = "tmcl.mock.downloadSource";

export function isSource(value: unknown): value is DownloadSource {
  return value === "auto" || value === "official" || value === "bmclapi";
}

/** 读取下载源；缺失或无效时取默认值。 */
export async function mockSourceGet(): Promise<DownloadSource> {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return isSource(saved) ? saved : DEFAULT_SOURCE;
  } catch {
    return DEFAULT_SOURCE;
  }
}

/** 保存下载源。 */
export async function mockSourceSave(source: DownloadSource): Promise<void> {
  localStorage.setItem(STORAGE_KEY, source);
}
