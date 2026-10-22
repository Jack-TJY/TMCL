// 校验界面文案（PLAN.md M0-07）：
// 1. 每种语言的模块文件和 key 集合完全一致，同一 key 的插值参数一致；
// 2. errors.json 的 key 与 ui/src/bindings/ErrorCode.ts 中的错误码一一对应；
// 3. 其他 key 都在 ui/src 的源码中以字符串字面量出现（无未使用 key）。
//    动态 key 用模板字符串 `前缀.${…}` 引用时，该前缀下的 key 都视为已使用。
// 任一项失败时以非零码退出，因此 `pnpm check` 与 CI 会失败。
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = join(root, "ui/src");
const localesDir = join(srcDir, "locales");
const errors = [];

/** 把嵌套对象展开为 `{ "a.b.c": "文案" }`。 */
function flatten(value, prefix, out) {
  for (const [key, child] of Object.entries(value)) {
    const path = `${prefix}.${key}`;
    if (typeof child === "string") out.set(path, child);
    else if (child && typeof child === "object" && !Array.isArray(child))
      flatten(child, path, out);
    else errors.push(`${path}: 文案必须是字符串或对象`);
  }
  return out;
}

/** 读取一种语言的全部模块，返回 key → 文案。 */
function loadLocale(locale) {
  const messages = new Map();
  for (const file of readdirSync(join(localesDir, locale)).filter((f) =>
    f.endsWith(".json"),
  )) {
    const path = join(localesDir, locale, file);
    try {
      flatten(
        JSON.parse(readFileSync(path, "utf8")),
        file.slice(0, -5),
        messages,
      );
    } catch (e) {
      errors.push(`${relative(root, path)}: ${e.message}`);
    }
  }
  return messages;
}

/** 文案中的插值参数名集合，例如 `{path}` → path。 */
function params(text) {
  return [...text.matchAll(/\{\s*([\w.]+)\s*\}/g)]
    .map((m) => m[1])
    .sort()
    .join(",");
}

/** ui/src 下除生成物和翻译文件外的所有源码。 */
function sourceFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      return path === localesDir || name === "bindings"
        ? []
        : sourceFiles(path);
    }
    return /\.(ts|vue)$/.test(name) && !/\.test\.ts$/.test(name) ? [path] : [];
  });
}

const locales = readdirSync(localesDir).filter((d) =>
  statSync(join(localesDir, d)).isDirectory(),
);
if (locales.length < 2)
  errors.push(`ui/src/locales 下只有 ${locales.length} 种语言`);
const all = new Map(locales.map((l) => [l, loadLocale(l)]));
const [base, ...others] = locales;

// 1. key 集合与插值参数一致
for (const other of others) {
  const a = all.get(base);
  const b = all.get(other);
  for (const key of a.keys())
    if (!b.has(key)) errors.push(`${other} 缺少 key ${key}（${base} 中有）`);
  for (const key of b.keys())
    if (!a.has(key)) errors.push(`${base} 缺少 key ${key}（${other} 中有）`);
  for (const [key, text] of a) {
    if (b.has(key) && params(text) !== params(b.get(key))) {
      errors.push(`${key}: ${base} 与 ${other} 的插值参数不一致`);
    }
  }
}

// 2. 错误码与 errors.json 一一对应
const errorCodeTs = readFileSync(join(srcDir, "bindings/ErrorCode.ts"), "utf8");
const codes = new Set(
  [...errorCodeTs.matchAll(/"([A-Z0-9_]+)"/g)].map((m) => m[1]),
);
const keys = [...all.get(base).keys()];
for (const code of codes) {
  if (!all.get(base).has(`errors.${code}`))
    errors.push(`错误码 ${code} 缺少文案 errors.${code}`);
}
for (const key of keys.filter((k) => k.startsWith("errors."))) {
  if (!codes.has(key.slice("errors.".length)))
    errors.push(`${key} 不是已定义的错误码`);
}

// 3. 无未使用 key
const source = sourceFiles(srcDir)
  .map((f) => readFileSync(f, "utf8"))
  .join("\n");
const dynamicPrefixes = [...source.matchAll(/`([\w.-]+)\.\$\{/g)].map(
  (m) => `${m[1]}.`,
);
for (const key of keys.filter((k) => !k.startsWith("errors."))) {
  const literal = ['"', "'", "`"].some((q) =>
    source.includes(`${q}${key}${q}`),
  );
  if (!literal && !dynamicPrefixes.some((p) => key.startsWith(p))) {
    errors.push(`未使用的 key ${key}`);
  }
}

if (errors.length > 0) {
  console.error(`i18n 检查失败（${errors.length} 项）：`);
  for (const e of errors) console.error(`  - ${e}`);
  process.exit(1);
}
console.log(
  `i18n 检查通过：${locales.join("、")}，每种语言 ${keys.length} 个 key。`,
);
