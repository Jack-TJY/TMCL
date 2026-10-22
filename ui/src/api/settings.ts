// `settings_*` 命令封装。
import type { SettingsLanguage } from "../bindings/SettingsLanguage";
import type { SettingsSetLanguageArgs } from "../bindings/SettingsSetLanguageArgs";
import { invoke } from "./invoke";

/** 读取已保存的界面语言；首次启动时 `language` 为 `null`。 */
export function settingsGetLanguage(): Promise<SettingsLanguage> {
  return invoke<SettingsLanguage>("settings_get_language");
}

/** 把界面语言写入 `config.json`。 */
export function settingsSetLanguage(args: SettingsSetLanguageArgs): Promise<void> {
  return invoke<void>("settings_set_language", { args });
}
