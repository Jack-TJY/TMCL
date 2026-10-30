//! 构建期注入 CurseForge API Key（AGENTS.md §11，受保护文件）。
//!
//! 来源优先级：环境变量 `TMCL_CURSEFORGE_API_KEY` > `TMCL_CF_API_KEY` > 仓库根 `.env.local`
//! 中的同名项。值写入 `$OUT_DIR/curseforge_api_key`，由 `src/curseforge/mod.rs` 用
//! `include_str!` 读取；不走 `cargo:rustc-env`，因为那样值会出现在 `cargo build -vv` 的输出里。
//! 缺失或格式不对时写入空文件，CurseForge 来源隐藏，构建照常通过。
//! 新建 `.env.local` 后需执行一次 `cargo clean -p tmcl-resources`（见下方 rerun 说明）。
//!
//! `.env.local` 按字面读取，不做 `$VAR` 替换：CurseForge 的 key 以 `$2a$10$` 开头，
//! 交给 dotenvy 会被当成变量展开而损坏。
//!
//! 本文件不得以任何形式输出 API Key 的值，包括 `cargo:warning`。

use std::path::{Path, PathBuf};
use std::{env, fs};

const KEYS: [&str; 2] = ["TMCL_CURSEFORGE_API_KEY", "TMCL_CF_API_KEY"];

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("modules/resources 位于仓库根下两级");
    let env_file = root.join(".env.local");

    println!("cargo:rerun-if-changed=build.rs");
    for key in KEYS {
        println!("cargo:rerun-if-env-changed={key}");
    }
    // 文件不存在时 Cargo 会把它当作“已变化”，每次构建都重跑并重编本 crate，
    // 所以只在文件存在时监视。新建 `.env.local` 后需 `cargo clean -p tmcl-resources`。
    if env_file.is_file() {
        println!("cargo:rerun-if-changed={}", env_file.display());
    }

    let from_env = KEYS.iter().find_map(|key| match env::var(key) {
        Ok(v) if !v.trim().is_empty() => Some(v),
        _ => None,
    });
    let key = from_env
        .or_else(|| read_env_file(&env_file))
        .map(|v| v.trim().to_owned())
        .filter(|v| {
            let ok = is_plausible_key(v);
            if !ok {
                println!(
                    "cargo:warning=CurseForge API Key 格式不对，已忽略，CurseForge 来源将隐藏"
                );
            }
            ok
        });

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("curseforge_api_key");
    fs::write(&out, key.unwrap_or_default()).expect("写入 OUT_DIR/curseforge_api_key");
}

/// 只取 `KEYS` 中的项（按 `KEYS` 的顺序优先），不把 `.env.local` 的其他内容带进构建环境。
fn read_env_file(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let mut found: [Option<String>; KEYS.len()] = Default::default();
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if let Some(i) = KEYS.iter().position(|k| *k == name.trim()) {
            found[i] = Some(unquote(value.trim()).to_owned());
        }
    }
    found.into_iter().flatten().find(|v| !v.trim().is_empty())
}

/// 去掉成对的单引号或双引号；不处理转义与行内注释。
fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

/// 可打印 ASCII、不含空白与引号，长度合理。
fn is_plausible_key(s: &str) -> bool {
    (16..=256).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\'')
}
