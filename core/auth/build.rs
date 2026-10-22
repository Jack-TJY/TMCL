//! 构建期注入微软登录 Client ID（AGENTS.md §11，受保护文件）。
//!
//! 来源优先级：环境变量 `TMCL_MS_CLIENT_ID` > 仓库根 `.env.local`。值写入
//! `$OUT_DIR/ms_client_id`，由 `src/config.rs` 用 `include_str!` 读取；不走
//! `cargo:rustc-env`，因为那样值会出现在 `cargo build -vv` 的输出里。
//! 缺失或格式不对时写入空文件，微软登录禁用，构建照常通过。
//! 新建 `.env.local` 后需执行一次 `cargo clean -p tmcl-auth`（见下方 rerun 说明）。
//!
//! 本文件不得以任何形式输出 Client ID 的值，包括 `cargo:warning`。

use std::path::{Path, PathBuf};
use std::{env, fs};

const KEY: &str = "TMCL_MS_CLIENT_ID";

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("core/auth 位于仓库根下两级");
    let env_file = root.join(".env.local");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={KEY}");
    // 文件不存在时 Cargo 会把它当作“已变化”，每次构建都重跑并重编本 crate，
    // 所以只在文件存在时监视。新建 `.env.local` 后需 `cargo clean -p tmcl-auth`。
    if env_file.is_file() {
        println!("cargo:rerun-if-changed={}", env_file.display());
    }

    let id = match env::var(KEY) {
        Ok(v) if !v.trim().is_empty() => Some(v),
        _ => read_env_file(&env_file),
    };
    let id = id.map(|v| v.trim().to_owned()).filter(|v| {
        let ok = is_guid(v);
        if !ok {
            println!("cargo:warning={KEY} 不是 GUID 格式，已忽略，微软登录将禁用");
        }
        ok
    });

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("ms_client_id");
    fs::write(&out, id.unwrap_or_default()).expect("写入 OUT_DIR/ms_client_id");
}

/// 只取 `KEY` 一项，不把 `.env.local` 的其他内容带进构建环境。
fn read_env_file(path: &Path) -> Option<String> {
    let iter = dotenvy::from_path_iter(path).ok()?;
    let mut found = None;
    for item in iter {
        match item {
            Ok((k, v)) if k == KEY => found = Some(v),
            Ok(_) => {}
            Err(_) => {
                // 解析错误信息可能带出行内容，只报告文件有问题。
                println!("cargo:warning=.env.local 解析失败，已忽略 {KEY}");
                return None;
            }
        }
    }
    found.filter(|v| !v.trim().is_empty())
}

/// `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`，十六进制。
fn is_guid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()))
}
