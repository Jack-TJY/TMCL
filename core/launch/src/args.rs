//! 启动命令拼装：classpath、占位符替换、内存与窗口参数。
//!
//! M0-09 覆盖原版新式（`arguments`）与旧式（`minecraftArguments`）两种格式；
//! 启动前检查、自定义 JVM/游戏参数与加载器由 M1-07 补充。

use std::collections::BTreeMap;
use std::path::PathBuf;

use tmcl_common::redact;
use tmcl_game::version::{Argument, VersionJson};
use tmcl_game::{GameFiles, RuleEnv, rules_allow};

/// 启动器名称，填入 `${launcher_name}`。
pub const LAUNCHER_NAME: &str = "TMCL";
/// 启动器版本，填入 `${launcher_version}`。
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 启动时使用的账号信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchAccount {
    /// 玩家名。
    pub name: String,
    /// 不带连字符的 UUID。
    pub uuid: String,
    /// 访问令牌；离线账号可为任意占位值。
    pub access_token: String,
    /// `msa` 或 `legacy`（离线）。
    pub user_type: String,
}

/// 一次启动的输入。
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    /// `java` 可执行文件。
    pub java: PathBuf,
    /// 已补全的版本。
    pub version: VersionJson,
    /// `--gameDir`。
    pub game_dir: PathBuf,
    /// natives 目录（LWJGL 会把本地库解压到这里）。
    pub natives_dir: PathBuf,
    /// 账号。
    pub account: LaunchAccount,
    /// 最小堆内存（MB）。
    pub min_memory_mb: u32,
    /// 最大堆内存（MB）。
    pub max_memory_mb: u32,
    /// 窗口大小，`None` 表示使用游戏默认值。
    pub window: Option<(u32, u32)>,
}

/// 拼装好的启动命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    /// 可执行文件。
    pub program: PathBuf,
    /// 参数。
    pub args: Vec<String>,
    /// 工作目录（即 `--gameDir`）。
    pub cwd: PathBuf,
}

impl LaunchCommand {
    /// 用于日志的命令行，访问令牌已脱敏。
    pub fn redacted(&self) -> String {
        let line = std::iter::once(self.program.display().to_string())
            .chain(self.args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        redact(&line).into_owned()
    }
}

/// 按当前平台拼装启动命令。
pub fn build_command(files: &GameFiles, spec: &LaunchSpec) -> LaunchCommand {
    let mut env = RuleEnv::current();
    if spec.window.is_some() {
        env = env.with_feature("has_custom_resolution", true);
    }
    build_command_for(files, spec, &env)
}

/// [`build_command`] 的可测试版本：显式给出规则环境。
pub fn build_command_for(files: &GameFiles, spec: &LaunchSpec, env: &RuleEnv) -> LaunchCommand {
    let version = &spec.version;
    let vars = placeholders(files, spec, env);
    let subst = |s: &str| substitute(s, &vars);

    let mut args = vec![
        format!("-Xms{}M", spec.min_memory_mb),
        format!("-Xmx{}M", spec.max_memory_mb),
    ];
    match &version.arguments {
        Some(arguments) => args.extend(expand(&arguments.jvm, env).map(subst)),
        None => {
            // 旧版本没有 JVM 参数模板，使用官方启动器的默认值。
            args.push(subst("-Djava.library.path=${natives_directory}"));
            args.push("-cp".to_owned());
            args.push(subst("${classpath}"));
        }
    }
    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = files.log_config(&log.file.id);
        args.push(log.argument.replace("${path}", &path.display().to_string()));
    }
    args.push(version.main_class.clone());
    match (&version.arguments, &version.minecraft_arguments) {
        (Some(arguments), _) => args.extend(expand(&arguments.game, env).map(subst)),
        (None, Some(legacy)) => {
            args.extend(legacy.split_whitespace().map(subst));
            if let Some((w, h)) = spec.window {
                args.extend([
                    "--width".into(),
                    w.to_string(),
                    "--height".into(),
                    h.to_string(),
                ]);
            }
        }
        (None, None) => {}
    }

    LaunchCommand {
        program: spec.java.clone(),
        args,
        cwd: spec.game_dir.clone(),
    }
}

/// 适用于 `env` 的依赖库与客户端 jar，按版本 JSON 中的顺序。
pub fn classpath(files: &GameFiles, version: &VersionJson, env: &RuleEnv) -> Vec<PathBuf> {
    let mut seen = std::collections::BTreeSet::new();
    let mut entries: Vec<PathBuf> = version
        .libraries
        .iter()
        .filter(|l| l.applies(env))
        .filter_map(|l| l.artifact())
        .filter(|a| seen.insert(a.path.clone()))
        .map(|a| files.library(&a.path))
        .collect();
    entries.push(files.client_jar(&version.id));
    entries
}

fn expand<'a>(args: &'a [Argument], env: &'a RuleEnv) -> impl Iterator<Item = &'a str> + 'a {
    args.iter()
        .flat_map(move |arg| match arg {
            Argument::Plain(s) => std::slice::from_ref(s),
            Argument::Conditional { rules, value } if rules_allow(rules, env) => value.values(),
            Argument::Conditional { .. } => &[],
        })
        .map(String::as_str)
}

fn placeholders(
    files: &GameFiles,
    spec: &LaunchSpec,
    env: &RuleEnv,
) -> BTreeMap<&'static str, String> {
    let version = &spec.version;
    let separator = if env.os_name == "windows" { ";" } else { ":" };
    let classpath = classpath(files, version, env)
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(separator);
    let assets_index = version
        .asset_index
        .as_ref()
        .map(|i| i.id.clone())
        .or_else(|| version.assets.clone())
        .unwrap_or_default();
    let (width, height) = spec.window.unwrap_or((854, 480));
    let account = &spec.account;
    BTreeMap::from([
        ("assets_index_name", assets_index),
        ("assets_root", files.assets_dir().display().to_string()),
        ("auth_access_token", account.access_token.clone()),
        ("auth_player_name", account.name.clone()),
        ("auth_session", account.access_token.clone()),
        ("auth_uuid", account.uuid.clone()),
        ("auth_xuid", String::new()),
        ("classpath", classpath),
        ("clientid", String::new()),
        ("game_assets", files.assets_dir().display().to_string()),
        ("game_directory", spec.game_dir.display().to_string()),
        ("launcher_name", LAUNCHER_NAME.to_owned()),
        ("launcher_version", LAUNCHER_VERSION.to_owned()),
        ("natives_directory", spec.natives_dir.display().to_string()),
        ("resolution_height", height.to_string()),
        ("resolution_width", width.to_string()),
        ("user_properties", "{}".to_owned()),
        ("user_type", account.user_type.clone()),
        ("version_name", version.id.clone()),
        ("version_type", version.kind.clone()),
    ])
}

/// 替换 `${name}` 占位符；未知占位符保持原样。
fn substitute(input: &str, vars: &BTreeMap<&'static str, String>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let name = &after[..end];
                match vars.get(name) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_handles_known_unknown_and_unclosed() {
        let vars = BTreeMap::from([("a", "1".to_owned()), ("b", "two".to_owned())]);
        assert_eq!(substitute("${a}-${b}", &vars), "1-two");
        assert_eq!(substitute("x${c}y", &vars), "x${c}y");
        assert_eq!(substitute("pre${a", &vars), "pre${a");
        assert_eq!(substitute("no vars", &vars), "no vars");
    }
}
