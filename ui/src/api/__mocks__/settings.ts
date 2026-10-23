// 设置页的临时数据源（AGENTS.md §6.3）。M1-10 提供 `settings_*`、`java_*` 等命令后，
// 用 `ui/src/api/` 中的真实封装替换本文件的调用方，并删除本文件。
// 在此之前设置保存在 WebView 的 localStorage 中，只为让界面可用，不是持久化方案。

/** 界面主题，与后端 `ThemeMode` 的取值一致。 */
export type ThemeMode = "system" | "light" | "dark";

/** 下载源策略：`auto` 先镜像后官方，或指定某个源（PLAN.md M1-02）。 */
export type DownloadSource = "auto" | "official" | "bmclapi";

/** 设置页编辑的启动器设置（语言单独由 `settings_*_language` 管理）。 */
export interface LauncherSettings {
  theme: ThemeMode;
  /** `#RRGGBB`；`null` 表示跟随系统强调色。 */
  accent: string | null;
  download: { source: DownloadSource; max_concurrency: number };
  /** 新建实例的默认最大内存。 */
  memory: { max_mb: number };
}

/** 一个 Java 运行时，字段与 PLAN.md M1-05 的 `JavaInstall` 一致。 */
export interface JavaInstall {
  path: string;
  major: number;
  version: string;
  arch: string;
  vendor: string;
  managed: boolean;
}

export const DEFAULT_SETTINGS: LauncherSettings = {
  theme: "system",
  accent: null,
  download: { source: "auto", max_concurrency: 16 },
  memory: { max_mb: 4096 },
};

const STORAGE_KEY = "tmcl.mock.settings";

/** 读取设置；缺失或损坏的字段取默认值。 */
export async function mockSettingsGet(): Promise<LauncherSettings> {
  let saved: Partial<LauncherSettings> = {};
  try {
    saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") ?? {};
  } catch {
    // 视同首次启动。
  }
  return {
    ...DEFAULT_SETTINGS,
    ...saved,
    download: { ...DEFAULT_SETTINGS.download, ...saved.download },
    memory: { ...DEFAULT_SETTINGS.memory, ...saved.memory },
  };
}

/** 保存全部设置。 */
export async function mockSettingsSave(settings: LauncherSettings): Promise<void> {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
}

/** 数据目录（PLAN.md 第 3.5 节：启动器安装目录下的 `tmcl-data/`）。 */
export async function mockDataDir(): Promise<string> {
  return "<TMCL>/tmcl-data";
}

/** 已发现的 Java 列表；M1-05 之前为空。 */
export async function mockJavaList(): Promise<JavaInstall[]> {
  return [];
}
