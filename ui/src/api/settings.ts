// `settings_*` 命令封装。
import type { LauncherSettings } from "../bindings/LauncherSettings";
import type { SettingsLanguage } from "../bindings/SettingsLanguage";
import type { SettingsSetArgs } from "../bindings/SettingsSetArgs";
import type { SettingsSetLanguageArgs } from "../bindings/SettingsSetLanguageArgs";
import type { SettingsSnapshot } from "../bindings/SettingsSnapshot";
import { invoke } from "./invoke";

/** 读取已保存的界面语言；首次启动时 `language` 为 `null`。 */
export function settingsGetLanguage(): Promise<SettingsLanguage> {
  return invoke<SettingsLanguage>("settings_get_language");
}

/** 把界面语言写入 `config.json`。 */
export function settingsSetLanguage(args: SettingsSetLanguageArgs): Promise<void> {
  return invoke<void>("settings_set_language", { args });
}

/** 读取启动器设置与数据目录。 */
export function settingsGet(): Promise<SettingsSnapshot> {
  return invoke<SettingsSnapshot>("settings_get");
}

/** 保存启动器设置（界面语言不变），返回保存后的设置。 */
export function settingsSet(args: SettingsSetArgs): Promise<LauncherSettings> {
  return invoke<LauncherSettings>("settings_set", { args });
}

/** 在系统文件管理器中打开数据目录。 */
export function settingsOpenDataDir(): Promise<void> {
  return invoke<void>("settings_open_data_dir");
}
