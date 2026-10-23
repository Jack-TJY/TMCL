//! 启动参数中的 `${name}` 占位符。
//!
//! 名称与官方启动器一致；未知占位符保持原样，便于在日志里发现版本 JSON 用了新占位符。

use std::collections::BTreeMap;

use tmcl_game::{GameFiles, RuleEnv};

use crate::args::{LAUNCHER_NAME, LAUNCHER_VERSION, LaunchSpec};
use crate::classpath::{classpath, join_classpath};

/// 一次启动的占位符取值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholders {
    vars: BTreeMap<&'static str, String>,
}

impl Placeholders {
    /// 按启动输入计算全部占位符。
    pub fn new(files: &GameFiles, spec: &LaunchSpec, env: &RuleEnv) -> Self {
        let version = &spec.version;
        let assets_index = version
            .asset_index
            .as_ref()
            .map(|i| i.id.clone())
            .or_else(|| version.assets.clone())
            .unwrap_or_default();
        let (width, height) = spec.window.unwrap_or((854, 480));
        let account = &spec.account;
        let path = |p: &std::path::Path| p.display().to_string();
        let vars = BTreeMap::from([
            ("assets_index_name", assets_index),
            ("assets_root", path(&files.assets_dir())),
            ("auth_access_token", account.access_token.clone()),
            ("auth_player_name", account.name.clone()),
            ("auth_session", account.access_token.clone()),
            ("auth_uuid", account.uuid.clone()),
            ("auth_xuid", String::new()),
            (
                "classpath",
                join_classpath(&classpath(files, version, env), env),
            ),
            ("classpath_separator", classpath_separator(env).to_owned()),
            ("clientid", String::new()),
            ("game_assets", path(&spec.game_assets_dir)),
            ("game_directory", path(&spec.game_dir)),
            ("launcher_name", LAUNCHER_NAME.to_owned()),
            ("launcher_version", LAUNCHER_VERSION.to_owned()),
            ("library_directory", path(&files.libraries_dir())),
            ("natives_directory", path(&spec.natives_dir)),
            ("resolution_height", height.to_string()),
            ("resolution_width", width.to_string()),
            ("user_properties", "{}".to_owned()),
            ("user_type", account.user_type.clone()),
            ("version_name", version.id.clone()),
            ("version_type", version.kind.clone()),
        ]);
        Self { vars }
    }

    /// 某个占位符的值。
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(String::as_str)
    }

    /// 替换 `input` 中的 `${name}`；未知占位符与未闭合的 `${` 保持原样。
    pub fn apply(&self, input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut rest = input;
        while let Some(start) = rest.find("${") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            match after.find('}') {
                Some(end) => {
                    match self.vars.get(&after[..end]) {
                        Some(value) => out.push_str(value),
                        None => out.push_str(&rest[start..start + 2 + end + 1]),
                    }
                    rest = &after[end + 1..];
                }
                None => {
                    out.push_str(&rest[start..]);
                    rest = "";
                }
            }
        }
        out.push_str(rest);
        out
    }
}

/// classpath 分隔符：Windows 为 `;`，其他系统为 `:`。
pub fn classpath_separator(env: &RuleEnv) -> &'static str {
    if env.os_name == "windows" { ";" } else { ":" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(vars: &[(&'static str, &str)]) -> Placeholders {
        Placeholders {
            vars: vars.iter().map(|(k, v)| (*k, (*v).to_owned())).collect(),
        }
    }

    #[test]
    fn apply_handles_known_unknown_and_unclosed() {
        let p = with(&[("a", "1"), ("b", "two")]);
        assert_eq!(p.apply("${a}-${b}"), "1-two");
        assert_eq!(p.apply("x${c}y"), "x${c}y");
        assert_eq!(p.apply("pre${a"), "pre${a");
        assert_eq!(p.apply("no vars"), "no vars");
        assert_eq!(p.apply("${a}${a}"), "11");
    }

    #[test]
    fn values_are_not_substituted_again() {
        let p = with(&[("a", "${b}"), ("b", "x")]);
        assert_eq!(p.apply("${a}"), "${b}");
    }
}
