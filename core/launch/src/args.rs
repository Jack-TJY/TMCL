//! 启动命令拼装：内存、版本 JSON 的参数模板、日志配置、自定义参数与窗口。
//!
//! 参数模板由 [`VersionJson::jvm_arguments`] / [`VersionJson::game_arguments`] 按规则展开
//! （新式 `arguments`、旧式 `minecraftArguments` 与加载器追加的参数都在其中），
//! 占位符见 [`crate::placeholders`]，classpath 见 [`crate::classpath`]。

use std::path::PathBuf;

use tmcl_common::redact;
use tmcl_game::{GameFiles, RuleEnv, VersionJson};

use crate::placeholders::Placeholders;

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
    /// 访问令牌；离线账号为占位值。
    pub access_token: String,
    /// `msa` 或 `legacy`（离线）。
    pub user_type: String,
}

/// 离线账号使用的访问令牌占位值。
pub const OFFLINE_ACCESS_TOKEN: &str = "0";

impl LaunchAccount {
    /// 离线账号：令牌为占位值，`user_type` 为 `legacy`。离线账号仅限本地测试。
    pub fn offline(name: &str, uuid: &str) -> Self {
        Self {
            name: name.to_owned(),
            uuid: uuid.to_owned(),
            access_token: OFFLINE_ACCESS_TOKEN.to_owned(),
            user_type: "legacy".to_owned(),
        }
    }
}

/// 一次启动的输入。
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    /// `java` 可执行文件。
    pub java: PathBuf,
    /// 已补全并经 [`tmcl_game::resolve`] 合并的版本。
    pub version: VersionJson,
    /// `--gameDir`，也是进程的工作目录。
    pub game_dir: PathBuf,
    /// `${game_assets}`：旧版本为虚拟资源目录，见 [`tmcl_game::game_assets_dir`]。
    pub game_assets_dir: PathBuf,
    /// natives 目录（每次启动新建，见 [`tmcl_game::new_natives_dir`]）。
    pub natives_dir: PathBuf,
    /// 账号。
    pub account: LaunchAccount,
    /// 最小堆内存（MB）。
    pub min_memory_mb: u32,
    /// 最大堆内存（MB）。
    pub max_memory_mb: u32,
    /// 用户自定义 JVM 参数，排在版本模板之后、主类之前，可覆盖模板中的同名参数。
    pub jvm_args: Vec<String>,
    /// 用户自定义游戏参数，排在最后。
    pub game_args: Vec<String>,
    /// 窗口大小，`None` 表示使用游戏默认值。
    pub window: Option<(u32, u32)>,
    /// 全屏启动（追加 `--fullscreen`，此时不传窗口大小）。
    pub fullscreen: bool,
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
    /// 写日志或转发游戏输出前要替换为 `***` 的值（访问令牌）。
    pub secrets: Vec<String>,
}

impl LaunchCommand {
    /// 用于日志的命令行，访问令牌已脱敏。
    pub fn redacted(&self) -> String {
        let line = std::iter::once(self.program.display().to_string())
            .chain(self.args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        mask(&redact(&line), &self.secrets)
    }
}

/// 把 `text` 中出现的每个 `secrets` 替换为 `***`。
pub fn mask(text: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .filter(|s| !s.is_empty())
        .fold(text.to_owned(), |acc, s| acc.replace(s.as_str(), "***"))
}

/// 本次启动在当前平台的规则环境：设置了窗口大小且非全屏时启用 `has_custom_resolution`。
pub fn rule_env(spec: &LaunchSpec) -> RuleEnv {
    let env = RuleEnv::current();
    if spec.window.is_some() && !spec.fullscreen {
        env.with_feature("has_custom_resolution", true)
    } else {
        env
    }
}

/// 按当前平台拼装启动命令。
pub fn build_command(files: &GameFiles, spec: &LaunchSpec) -> LaunchCommand {
    build_command_for(files, spec, &rule_env(spec))
}

/// Forge 1.17 起用 `-DignoreList` 按文件名把客户端 jar 排除出模块层，模板写的是 `${version_name}.jar`；
/// 继承版本使用父版本的 `<jar_id>.jar`，需一并排除，否则 Minecraft 会被重复加载
/// （`ResolutionException: Module minecraft contains package ...`）。
fn ignore_client_jar(arg: String, version: &VersionJson) -> String {
    if arg.starts_with("-DignoreList=") && version.jar_id() != version.id {
        format!("{arg},{}.jar", version.jar_id())
    } else {
        arg
    }
}

/// [`build_command`] 的可测试版本：显式给出规则环境。
///
/// 顺序：`-Xms`/`-Xmx` → 版本 JVM 参数模板 → 自定义 JVM 参数 → 日志配置 → 主类 →
/// 版本游戏参数模板 → `--fullscreen` → 自定义游戏参数。自定义参数中的占位符同样会被替换。
pub fn build_command_for(files: &GameFiles, spec: &LaunchSpec, env: &RuleEnv) -> LaunchCommand {
    let version = &spec.version;
    let vars = Placeholders::new(files, spec, env);
    let subst = |s: &String| vars.apply(s);

    let mut args = vec![
        format!("-Xms{}M", spec.min_memory_mb),
        format!("-Xmx{}M", spec.max_memory_mb),
    ];
    args.extend(
        version
            .jvm_arguments(env)
            .iter()
            .map(subst)
            .map(|a| ignore_client_jar(a, version)),
    );
    args.extend(spec.jvm_args.iter().map(subst));
    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = files.log_config(&log.file.id);
        args.push(log.argument.replace("${path}", &path.display().to_string()));
    }
    args.push(version.main_class.clone());
    args.extend(version.game_arguments(env).iter().map(subst));
    if spec.fullscreen && !args.iter().any(|a| a == "--fullscreen") {
        args.push("--fullscreen".to_owned());
    }
    args.extend(spec.game_args.iter().map(subst));

    let token = &spec.account.access_token;
    let secrets = if token.len() > OFFLINE_ACCESS_TOKEN.len() {
        vec![token.clone()]
    } else {
        Vec::new()
    };
    LaunchCommand {
        program: spec.java.clone(),
        args,
        cwd: spec.game_dir.clone(),
        secrets,
    }
}
