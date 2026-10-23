//! 版本 JSON 中的 `rules`：按操作系统名称、架构、系统版本与启动器特性决定是否生效。

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

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
    /// 系统版本正则，与 Java 的 `os.version` 比较，例如 `^10\\.`。
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
    /// 与 Java `os.version` 一致的系统版本，例如 Windows 10/11 为 `10.0`；未知时为 `None`，
    /// 此时带 `version` 条件的规则视为不满足。
    pub os_version: Option<String>,
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
        static OS_VERSION: OnceLock<Option<String>> = OnceLock::new();
        Self {
            os_name,
            arch,
            os_version: OS_VERSION.get_or_init(detect_os_version).clone(),
            features: BTreeMap::new(),
        }
    }

    /// 指定系统名与架构、系统版本未知的环境，用于按目标平台求值。
    pub fn new(os_name: &'static str, arch: &'static str) -> Self {
        Self {
            os_name,
            arch,
            os_version: None,
            features: BTreeMap::new(),
        }
    }

    /// 设置系统版本。
    pub fn with_os_version(mut self, version: impl Into<String>) -> Self {
        self.os_version = Some(version.into());
        self
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
        if let Some(pattern) = &os.version {
            let matched = env
                .os_version
                .as_deref()
                .is_some_and(|v| regex::Regex::new(pattern).is_ok_and(|re| re.is_match(v)));
            if !matched {
                return false;
            }
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

/// 取得与 Java `os.version` 相同格式的系统版本。
#[cfg(windows)]
fn detect_os_version() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .ok()?;
    match (
        key.get_value::<u32, _>("CurrentMajorVersionNumber"),
        key.get_value::<u32, _>("CurrentMinorVersionNumber"),
    ) {
        (Ok(major), Ok(minor)) => Some(format!("{major}.{minor}")),
        // Windows 8.1 及以前没有上面两个值。
        _ => key.get_value::<String, _>("CurrentVersion").ok(),
    }
}

#[cfg(target_os = "macos")]
fn detect_os_version() -> Option<String> {
    let output = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    let version = String::from_utf8(output.stdout).ok()?;
    Some(version.trim().to_owned()).filter(|v| !v.is_empty())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn detect_os_version() -> Option<String> {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok()?;
    Some(release.trim().to_owned()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(os: &'static str, arch: &'static str) -> RuleEnv {
        RuleEnv::new(os, arch)
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

        let r = rules(r#"[{"action":"allow","features":{"is_demo_user":false}}]"#);
        assert!(rules_allow(&r, &env("linux", "x86_64")));
    }

    #[test]
    fn os_version_regex() {
        // 1.7.10 以前的 LWJGL：OS X 10.5 用专门的夜间构建。
        let r = rules(
            r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx","version":"^10\\.5\\.\\d$"}}]"#,
        );
        assert!(!rules_allow(
            &r,
            &env("osx", "x86_64").with_os_version("10.5.8")
        ));
        assert!(rules_allow(
            &r,
            &env("osx", "x86_64").with_os_version("10.15.7")
        ));
        // 版本未知时条件不满足，不会误禁。
        assert!(rules_allow(&r, &env("osx", "x86_64")));

        let r = rules(r#"[{"action":"allow","os":{"name":"windows","version":"^10\\."}}]"#);
        assert!(rules_allow(
            &r,
            &env("windows", "x86_64").with_os_version("10.0")
        ));
        assert!(!rules_allow(
            &r,
            &env("windows", "x86_64").with_os_version("6.1")
        ));

        // 无效正则视为不满足。
        let r = rules(r#"[{"action":"allow","os":{"version":"("}}]"#);
        assert!(!rules_allow(
            &r,
            &env("linux", "x86_64").with_os_version("6.1")
        ));
    }

    #[test]
    fn current_env_knows_os_version() {
        let env = RuleEnv::current();
        assert!(["windows", "osx", "linux"].contains(&env.os_name));
        if cfg!(windows) {
            assert!(env.os_version.is_some());
        }
    }
}
