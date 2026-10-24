// 生成“关于”页的第三方许可证列表 ui/src/pages/about/third-party-licenses.json（生成物，禁止手改）。
//
// - Rust：cargo-about（配置 scripts/about.toml），只含 tmcl-app 在三平台上随安装包分发的依赖。
// - 前端：`pnpm licenses list --prod`，许可证原文取自各包目录下的 LICENSE / COPYING 文件。
//
// 依赖（Cargo.lock、pnpm-lock.yaml）变化后运行 `pnpm gen:licenses` 并提交结果。
// 需要先安装 cargo-about：`cargo install cargo-about --locked --features cli`。
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = join(root, "ui/src/pages/about/third-party-licenses.json");

/**
 * 运行命令并返回 stdout；失败时带上 stderr 退出。
 * 参数都是脚本里的固定值，经 shell 运行以便在 Windows 上找到 pnpm.cmd。
 */
function run(args, cwd = root) {
  const command = args.join(" ");
  const result = spawnSync(command, {
    cwd,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
    shell: true,
  });
  if (result.status !== 0) {
    console.error(`${command} 失败：\n${result.stderr || result.error}`);
    process.exit(1);
  }
  return result.stdout;
}

/** 许可证原文去重：相同文本只存一份，包引用其下标。 */
const texts = [];
const textIndex = new Map();
function textId(text) {
  const normalized = text.replace(/\r\n/g, "\n").trim();
  let id = textIndex.get(normalized);
  if (id === undefined) {
    id = texts.length;
    texts.push(normalized);
    textIndex.set(normalized, id);
  }
  return id;
}

function rustPackages() {
  const about = JSON.parse(
    run([
      "cargo",
      "about",
      "generate",
      "--format",
      "json",
      "--locked",
      "--offline",
      "--fail",
      "-c",
      "scripts/about.toml",
      "-m",
      "src-tauri/Cargo.toml",
    ]),
  );
  const textsByCrate = new Map();
  for (const license of about.licenses) {
    const id = textId(license.text);
    for (const { crate } of license.used_by) {
      const key = `${crate.name}@${crate.version}`;
      const ids = textsByCrate.get(key) ?? [];
      if (!ids.includes(id)) ids.push(id);
      textsByCrate.set(key, ids);
    }
  }
  return about.crates.map(({ package: pkg, license }) => ({
    name: pkg.name,
    version: pkg.version,
    source: "cargo",
    license,
    url: pkg.repository ?? pkg.homepage ?? `https://crates.io/crates/${pkg.name}`,
    texts: textsByCrate.get(`${pkg.name}@${pkg.version}`) ?? [],
  }));
}

const LICENSE_FILE = /^(licen[cs]e|copying)(\.|-|$)/i;

function isPlatformPackage(dir) {
  const manifest = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  return Boolean(manifest.os || manifest.cpu);
}

function npmPackages() {
  const byLicense = JSON.parse(
    run(["pnpm", "licenses", "list", "--prod", "--json"], join(root, "ui")),
  );
  const packages = [];
  for (const [license, entries] of Object.entries(byLicense)) {
    for (const entry of entries) {
      // 带 os / cpu 限制的是构建工具的原生二进制（如 rolldown 的平台包），不进安装包，
      // 且随生成脚本所在平台变化，跳过以保证生成结果与平台无关。
      if (entry.paths?.some(isPlatformPackage)) continue;
      const ids = [];
      for (const dir of entry.paths ?? []) {
        for (const file of readdirSync(dir).filter((f) => LICENSE_FILE.test(f)).sort()) {
          const id = textId(readFileSync(join(dir, file), "utf8"));
          if (!ids.includes(id)) ids.push(id);
        }
      }
      packages.push({
        name: entry.name,
        version: entry.versions.join(", "),
        source: "npm",
        license,
        url: entry.homepage ?? `https://www.npmjs.com/package/${entry.name}`,
        texts: ids,
      });
    }
  }
  return packages;
}

const packages = [...rustPackages(), ...npmPackages()].sort(
  (a, b) =>
    a.source.localeCompare(b.source) ||
    a.name.localeCompare(b.name) ||
    a.version.localeCompare(b.version),
);
writeFileSync(output, JSON.stringify({ texts, packages }) + "\n");
console.log(`已写入 ${output}：${packages.length} 个包，${texts.length} 份许可证原文。`);
