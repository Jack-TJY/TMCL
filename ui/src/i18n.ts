// 界面国际化（PLAN.md M0-07）：文案按模块拆分为 locales/<语言>/<模块>.json，
// 文件名即 key 的第一段，例如 common.json 中的 `language.label` 用 `common.language.label` 引用。
import { createI18n, type Composer, type LocaleMessageValue } from "vue-i18n";
import { settingsGetLanguage, settingsSetLanguage } from "./api/settings";
import type { AppError } from "./bindings/AppError";

/** 支持的界面语言；新增语言时同时新增 `locales/<语言>/` 目录。 */
export const SUPPORTED_LOCALES = ["zh-CN", "en-US"] as const;
export type Locale = (typeof SUPPORTED_LOCALES)[number];

/** 语言选择器中显示的名称，始终用该语言自身书写，因此不进入翻译文件。 */
export const LOCALE_NAMES: Record<Locale, string> = {
  "zh-CN": "简体中文",
  "en-US": "English",
};

/** 系统语言不受支持时使用的语言。 */
export const FALLBACK_LOCALE: Locale = "en-US";

type Messages = Record<string, LocaleMessageValue>;

/** 把 `./locales/<语言>/<模块>.json` 组装成 `{ <语言>: { <模块>: {...} } }`。 */
function loadMessages(): Record<Locale, Messages> {
  const files = import.meta.glob<LocaleMessageValue>("./locales/*/*.json", {
    eager: true,
    import: "default",
  });
  const messages = Object.fromEntries(SUPPORTED_LOCALES.map((l) => [l, {}])) as Record<
    Locale,
    Messages
  >;
  for (const [path, content] of Object.entries(files)) {
    const [, locale, module] = path.match(/\.\/locales\/([^/]+)\/([^/]+)\.json$/) ?? [];
    if (isLocale(locale)) messages[locale][module] = content;
  }
  return messages;
}

export const i18n = createI18n({
  legacy: false,
  locale: FALLBACK_LOCALE,
  fallbackLocale: FALLBACK_LOCALE,
  messages: loadMessages(),
});

/** Composition 模式的全局实例；文案从 JSON 动态加载，因此不做 key 的静态类型推导。 */
const composer = i18n.global as unknown as Composer;

/** 判断是否为支持的语言标签。 */
export function isLocale(value: unknown): value is Locale {
  return SUPPORTED_LOCALES.includes(value as Locale);
}

/** 按偏好顺序（如 `navigator.languages`）选出支持的语言：先精确匹配，再按主语言匹配。 */
export function resolveLocale(preferred: readonly string[]): Locale {
  for (const tag of preferred) {
    const exact = SUPPORTED_LOCALES.find((l) => l.toLowerCase() === tag.toLowerCase());
    if (exact) return exact;
    const primary = tag.split("-")[0].toLowerCase();
    const sameLanguage = SUPPORTED_LOCALES.find((l) => l.split("-")[0].toLowerCase() === primary);
    if (sameLanguage) return sameLanguage;
  }
  return FALLBACK_LOCALE;
}

/** 当前界面语言。 */
export function currentLocale(): Locale {
  return composer.locale.value as Locale;
}

function apply(locale: Locale) {
  composer.locale.value = locale;
  // 单元测试运行在 Node 环境，没有 document。
  if (typeof document !== "undefined") document.documentElement.lang = locale;
}

/**
 * 启动时确定界面语言：优先使用 `config.json` 中保存的值；首次启动按系统语言选择并写回。
 * 后端不可用（如在浏览器中调试）时只按系统语言选择。
 */
export async function initLocale(systemLocales: readonly string[] = navigator.languages) {
  let saved: string | null;
  try {
    saved = (await settingsGetLanguage()).language;
  } catch {
    apply(resolveLocale(systemLocales));
    return;
  }
  if (isLocale(saved)) {
    apply(saved);
    return;
  }
  const locale = resolveLocale(saved ? [saved, ...systemLocales] : systemLocales);
  apply(locale);
  // 写回失败不影响使用，下次启动会再次按系统语言选择。
  await settingsSetLanguage({ language: locale }).catch(() => undefined);
}

/** 切换界面语言：界面立即更新，并写入 `config.json`。写入失败时抛出 AppError。 */
export async function setLocale(locale: Locale) {
  apply(locale);
  await settingsSetLanguage({ language: locale });
}

/** 把后端返回的 AppError 渲染为当前语言的文案（key 为 `errors.<CODE>`）。 */
export function formatError(error: AppError): string {
  return composer.t(`errors.${error.code}`, error.params);
}
