// `settings_*` 命令封装。
import type { SettingsLocale } from "../bindings/SettingsLocale";
import type { SettingsSetLocaleArgs } from "../bindings/SettingsSetLocaleArgs";
import { invoke } from "./invoke";

/** 读取已保存的界面语言；首次启动时 `locale` 为 `null`。 */
export function settingsGetLocale(): Promise<SettingsLocale> {
  return invoke<SettingsLocale>("settings_get_locale");
}

/** 把界面语言写入 `config.json`。 */
export function settingsSetLocale(args: SettingsSetLocaleArgs): Promise<void> {
  return invoke<void>("settings_set_locale", { args });
}
