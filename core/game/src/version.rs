//! 版本 JSON（`<id>.json`）的数据结构与规则判断。
//!
//! M0-09 只覆盖原版新式格式（`arguments.{game,jvm}`、LWJGL3 natives 作为普通库）。
//! `inheritsFrom` 合并、旧式 `minecraftArguments` 的完整处理与 `natives` 分类器
//! 由 M1-03 / M1-04 补充；这里先把字段解析出来，避免以后改结构。

use std::collections::BTreeMap;

use serde::Deserialize;

/// 一个版本的完整描述。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    /// 版本号，例如 `1.20.1`。
    pub id: String,
    /// 版本类型，例如 `release`。
    #[serde(rename = "type", default)]
    pub kind: String,
    /// 主类。
    pub main_class: String,
    /// 继承的父版本（加载器版本使用）。
    #[serde(default)]
    pub inherits_from: Option<String>,
    /// 新式启动参数（1.13 起）。
    #[serde(default)]
    pub arguments: Option<Arguments>,
    /// 旧式启动参数（1.12.2 及以前），以空格分隔。
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    /// 资源索引。
    #[serde(default)]
    pub asset_index: Option<AssetIndexRef>,
    /// 资源索引 ID（与 `asset_index.id` 相同）。
    #[serde(default)]
    pub assets: Option<String>,
    /// 客户端 jar 等下载项。
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    /// 依赖库。
    #[serde(default)]
    pub libraries: Vec<Library>,
    /// 推荐的 Java 版本。
    #[serde(default)]
    pub java_version: Option<JavaVersion>,
    /// 日志配置。
    #[serde(default)]
    pub logging: Option<Logging>,
}

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

/// 一条规则。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Rule {
    /// `allow` 或 `disallow`。
    pub action: RuleAction,
    /// 操作系统条件。
    #[serde(default)]
    pub os: Option<OsRule>,
    /// 启动器特性条件，例如 `is_demo_user`。
    #[serde(default)]
    pub features: Option<BTreeMap<String, bool>>,
}

/// 规则动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    /// 满足条件时允许。
    Allow,
    /// 满足条件时禁止。
    Disallow,
}

/// 操作系统条件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OsRule {
    /// `windows`、`osx` 或 `linux`。
    #[serde(default)]
    pub name: Option<String>,
    /// `x86` 等架构名。
    #[serde(default)]
    pub arch: Option<String>,
    /// 系统版本正则（M1-03 实现匹配，目前视为满足）。
    #[serde(default)]
    pub version: Option<String>,
}

/// 规则求值所需的运行环境。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleEnv {
    /// 规则中的系统名：`windows`、`osx` 或 `linux`。
    pub os_name: &'static str,
    /// 规则中的架构名：`x86`、`x86_64`、`arm64` 等。
    pub arch: &'static str,
    /// 启用的特性，例如 `has_custom_resolution`。
    pub features: BTreeMap<String, bool>,
}

impl RuleEnv {
    /// 当前平台、不启用任何特性的环境。
    pub fn current() -> Self {
        let os_name = match std::env::consts::OS {
            "windows" => "windows",
            "macos" => "osx",
            _ => "linux",
        };
        let arch = match std::env::consts::ARCH {
            "x86" => "x86",
            "aarch64" => "arm64",
            other => other,
        };
        Self {
            os_name,
            arch,
            features: BTreeMap::new(),
        }
    }

    /// 启用或关闭一个特性。
    pub fn with_feature(mut self, name: impl Into<String>, enabled: bool) -> Self {
        self.features.insert(name.into(), enabled);
        self
    }
}

/// 按官方启动器语义求值：没有规则时允许；否则从“禁止”开始，
/// 依次应用每条条件满足的规则，最后一条生效。
pub fn rules_allow(rules: &[Rule], env: &RuleEnv) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, env) {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, env: &RuleEnv) -> bool {
    if let Some(os) = &rule.os {
        if os.name.as_deref().is_some_and(|n| n != env.os_name) {
            return false;
        }
        if os.arch.as_deref().is_some_and(|a| a != env.arch) {
            return false;
        }
    }
    if let Some(features) = &rule.features {
        for (name, wanted) in features {
            if env.features.get(name).copied().unwrap_or(false) != *wanted {
                return false;
            }
        }
    }
    true
}

/// 资源索引引用。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    /// 索引 ID，例如 `5`。
    pub id: String,
    /// 索引文件 SHA1。
    pub sha1: String,
    /// 索引文件大小。
    pub size: u64,
    /// 所有资源文件的总大小。
    #[serde(default)]
    pub total_size: Option<u64>,
    /// 索引文件地址。
    pub url: String,
}

/// 一个可下载文件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileDownload {
    /// SHA1。
    pub sha1: String,
    /// 大小（字节）。
    pub size: u64,
    /// 地址。
    pub url: String,
}

/// 版本级下载项。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VersionDownloads {
    /// 客户端 jar。
    pub client: FileDownload,
}

/// 一个依赖库。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Library {
    /// Maven 坐标，例如 `org.lwjgl:lwjgl:3.3.1:natives-windows`。
    pub name: String,
    /// 下载信息。
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    /// 适用规则。
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// 旧式 natives：系统名 → 分类器名（M1-04 处理）。
    #[serde(default)]
    pub natives: Option<BTreeMap<String, String>>,
}

/// 库的下载信息。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LibraryDownloads {
    /// 主构件。
    #[serde(default)]
    pub artifact: Option<Artifact>,
    /// 旧式 natives 分类器（M1-04 处理）。
    #[serde(default)]
    pub classifiers: Option<BTreeMap<String, Artifact>>,
}

/// 库构件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Artifact {
    /// 相对 `libraries/` 的路径。
    pub path: String,
    /// SHA1。
    pub sha1: String,
    /// 大小。
    pub size: u64,
    /// 地址。
    pub url: String,
}

impl Library {
    /// 在给定环境下是否需要该库。
    pub fn applies(&self, env: &RuleEnv) -> bool {
        rules_allow(&self.rules, env)
    }

    /// 主构件（新式格式中 natives 也是主构件）。
    pub fn artifact(&self) -> Option<&Artifact> {
        self.downloads.as_ref()?.artifact.as_ref()
    }
}

/// 推荐的 Java 版本。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    /// 官方运行时组件名，例如 `java-runtime-gamma`。
    #[serde(default)]
    pub component: Option<String>,
    /// 主版本号，例如 17。
    pub major_version: u32,
}

/// `logging` 字段。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Logging {
    /// 客户端日志配置。
    #[serde(default)]
    pub client: Option<LoggingClient>,
}

/// 客户端日志配置。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoggingClient {
    /// JVM 参数模板，含 `${path}` 占位符。
    pub argument: String,
    /// 配置文件。
    pub file: LoggingFile,
}

/// 日志配置文件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoggingFile {
    /// 文件名，例如 `client-1.12.xml`。
    pub id: String,
    /// SHA1。
    pub sha1: String,
    /// 大小。
    pub size: u64,
    /// 地址。
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(os: &'static str, arch: &'static str) -> RuleEnv {
        RuleEnv {
            os_name: os,
            arch,
            features: BTreeMap::new(),
        }
    }

    fn rules(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn empty_rules_allow() {
        assert!(rules_allow(&[], &env("windows", "x86_64")));
    }

    #[test]
    fn os_allow_rule() {
        let r = rules(r#"[{"action":"allow","os":{"name":"osx"}}]"#);
        assert!(rules_allow(&r, &env("osx", "arm64")));
        assert!(!rules_allow(&r, &env("windows", "x86_64")));
    }

    #[test]
    fn last_matching_rule_wins() {
        let r = rules(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#);
        assert!(rules_allow(&r, &env("linux", "x86_64")));
        assert!(!rules_allow(&r, &env("osx", "x86_64")));
    }

    #[test]
    fn arch_and_features() {
        let r = rules(r#"[{"action":"allow","os":{"arch":"x86"}}]"#);
        assert!(rules_allow(&r, &env("windows", "x86")));
        assert!(!rules_allow(&r, &env("windows", "x86_64")));

        let r = rules(r#"[{"action":"allow","features":{"has_custom_resolution":true}}]"#);
        assert!(!rules_allow(&r, &env("windows", "x86_64")));
        let e = env("windows", "x86_64").with_feature("has_custom_resolution", true);
        assert!(rules_allow(&r, &e));
    }
}
