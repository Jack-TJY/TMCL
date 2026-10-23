//! 启动参数模板：新式 `arguments.{game,jvm}`（1.13 起）与旧式 `minecraftArguments`。
//!
//! 这里只按规则展开出带 `${...}` 占位符的模板，占位符替换由启动层负责。

use std::sync::OnceLock;

use serde::Deserialize;

use crate::rules::{Rule, RuleEnv, rules_allow};

/// `arguments` 字段。
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Arguments {
    /// 游戏参数。
    #[serde(default)]
    pub game: Vec<Argument>,
    /// JVM 参数。
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

/// 单个参数：普通字符串，或带规则的一个或多个值。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    /// 无条件参数。
    Plain(String),
    /// 规则满足时才加入的参数。
    Conditional {
        /// 规则列表。
        rules: Vec<Rule>,
        /// 参数值。
        value: ArgumentValue,
    },
}

/// 条件参数的值。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    /// 单个值。
    One(String),
    /// 多个值。
    Many(Vec<String>),
}

impl ArgumentValue {
    /// 以切片形式返回所有值。
    pub fn values(&self) -> &[String] {
        match self {
            Self::One(v) => std::slice::from_ref(v),
            Self::Many(v) => v,
        }
    }
}

/// 按规则展开参数列表。
pub fn expand(args: &[Argument], env: &RuleEnv) -> Vec<String> {
    args.iter()
        .flat_map(|arg| match arg {
            Argument::Plain(s) => std::slice::from_ref(s),
            Argument::Conditional { rules, value } if rules_allow(rules, env) => value.values(),
            Argument::Conditional { .. } => &[],
        })
        .cloned()
        .collect()
}

/// 把旧式 `minecraftArguments`（空格分隔）转成参数列表，并按官方启动器的做法
/// 在启用 `has_custom_resolution` 时追加窗口大小参数。
pub fn from_legacy(minecraft_arguments: &str) -> Vec<Argument> {
    let mut args: Vec<Argument> = minecraft_arguments
        .split_whitespace()
        .map(|s| Argument::Plain(s.to_owned()))
        .collect();
    args.extend(legacy_resolution().iter().cloned());
    args
}

/// 没有 `arguments.jvm` 的旧版本使用的 JVM 参数，与 1.13 起官方版本 JSON 的默认段一致。
pub fn legacy_jvm() -> &'static [Argument] {
    static JVM: OnceLock<Vec<Argument>> = OnceLock::new();
    JVM.get_or_init(|| parse_builtin(LEGACY_JVM))
}

fn legacy_resolution() -> &'static [Argument] {
    static GAME: OnceLock<Vec<Argument>> = OnceLock::new();
    GAME.get_or_init(|| parse_builtin(LEGACY_RESOLUTION))
}

fn parse_builtin(json: &str) -> Vec<Argument> {
    // 内置常量，单元测试保证能解析。
    serde_json::from_str(json).unwrap_or_default()
}

const LEGACY_JVM: &str = r#"[
    {"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]},
    {"rules": [{"action": "allow", "os": {"name": "windows"}}],
     "value": "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump"},
    {"rules": [{"action": "allow", "os": {"name": "windows", "version": "^10\\."}}],
     "value": ["-Dos.name=Windows 10", "-Dos.version=10.0"]},
    {"rules": [{"action": "allow", "os": {"arch": "x86"}}], "value": "-Xss1M"},
    "-Djava.library.path=${natives_directory}",
    "-Dminecraft.launcher.brand=${launcher_name}",
    "-Dminecraft.launcher.version=${launcher_version}",
    "-cp",
    "${classpath}"
]"#;

const LEGACY_RESOLUTION: &str = r#"[
    {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}],
     "value": ["--width", "${resolution_width}", "--height", "${resolution_height}"]}
]"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_templates_parse() {
        assert_eq!(
            serde_json::from_str::<Vec<Argument>>(LEGACY_JVM)
                .unwrap()
                .len(),
            9
        );
        assert_eq!(legacy_jvm().len(), 9);
        assert_eq!(legacy_resolution().len(), 1);
    }

    #[test]
    fn legacy_jvm_by_platform() {
        let win = expand(
            legacy_jvm(),
            &RuleEnv::new("windows", "x86_64").with_os_version("10.0"),
        );
        assert!(win.contains(&"-Dos.name=Windows 10".to_owned()));
        assert!(!win.contains(&"-XstartOnFirstThread".to_owned()));
        assert_eq!(win.last().map(String::as_str), Some("${classpath}"));

        let mac = expand(legacy_jvm(), &RuleEnv::new("osx", "arm64"));
        assert_eq!(mac[0], "-XstartOnFirstThread");
        assert_eq!(mac.len(), 6);

        let x86 = expand(legacy_jvm(), &RuleEnv::new("linux", "x86"));
        assert!(x86.contains(&"-Xss1M".to_owned()));
    }

    #[test]
    fn legacy_game_arguments() {
        let args = from_legacy("${auth_player_name}  ${auth_session} --gameDir ${game_directory}");
        let env = RuleEnv::new("linux", "x86_64");
        assert_eq!(
            expand(&args, &env),
            [
                "${auth_player_name}",
                "${auth_session}",
                "--gameDir",
                "${game_directory}"
            ]
        );
        let sized = expand(&args, &env.with_feature("has_custom_resolution", true));
        assert_eq!(sized.len(), 8);
        assert_eq!(sized[4], "--width");
    }
}
