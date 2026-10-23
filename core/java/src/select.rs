//! 为一个游戏版本选出合适的 Java。
//!
//! 规则（PLAN.md M1-05）：版本 JSON 有 `javaVersion.majorVersion` 用它，没有的旧版本用 8；
//! 自动模式只选主版本一致且架构与本程序一致的 Java；
//! 用户手动指定的 Java 只给警告，不阻止启动。

use std::cmp::Ordering;

use tmcl_common::{AppError, AppResult, ErrorCode};

use crate::probe::{Arch, JavaInstall};

/// 没有 `javaVersion` 字段的旧版本使用的主版本号。
pub const LEGACY_MAJOR: u32 = 8;

/// 由版本 JSON 的 `javaVersion.majorVersion` 得出需要的主版本号。
pub fn required_major(major_version: Option<u32>) -> u32 {
    major_version.unwrap_or(LEGACY_MAJOR)
}

/// 自动模式：在 `installs` 中选出主版本为 `major`、架构为 `arch` 的最佳 Java。
///
/// 优先启动器管理的运行时，其次版本号较新的。
/// 只有架构不符的候选时返回 `JAVA_ARCH_MISMATCH`，完全没有候选时返回 `JAVA_NOT_FOUND`；
/// 调用方可在这两种情况下改为下载运行时。
pub fn select(installs: &[JavaInstall], major: u32, arch: Arch) -> AppResult<&JavaInstall> {
    let same_major: Vec<&JavaInstall> = installs.iter().filter(|i| i.major == major).collect();
    let best = same_major
        .iter()
        .copied()
        .filter(|i| i.arch == arch)
        .max_by(|a, b| {
            a.managed
                .cmp(&b.managed)
                .then_with(|| compare_versions(&a.version, &b.version))
        });
    if let Some(best) = best {
        return Ok(best);
    }
    match same_major.first() {
        Some(other) => Err(arch_mismatch(other, arch)),
        None => Err(AppError::new(ErrorCode::JavaNotFound).with_param("major", major)),
    }
}

/// 手动模式：检查用户指定的 Java，返回警告（`JAVA_VERSION_MISMATCH`、`JAVA_ARCH_MISMATCH`），不阻止启动。
pub fn check_manual(install: &JavaInstall, major: u32, arch: Arch) -> Vec<AppError> {
    let mut warnings = Vec::new();
    if install.major != major {
        warnings.push(
            AppError::new(ErrorCode::JavaVersionMismatch)
                .with_param("java", install.path.display())
                .with_param("expected", major)
                .with_param("actual", install.major),
        );
    }
    if install.arch != arch {
        warnings.push(arch_mismatch(install, arch));
    }
    warnings
}

fn arch_mismatch(install: &JavaInstall, expected: Arch) -> AppError {
    AppError::new(ErrorCode::JavaArchMismatch)
        .with_param("java", install.path.display())
        .with_param("expected", expected.as_str())
        .with_param("actual", install.arch.as_str())
}

/// 按数字段比较版本号：`17.0.15` > `17.0.8`，`1.8.0_202` > `1.8.0_51`。
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |v: &str| -> Vec<u64> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap_or(u64::MAX))
            .collect()
    };
    parts(a).cmp(&parts(b))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn java(path: &str, version: &str, major: u32, arch: Arch, managed: bool) -> JavaInstall {
        JavaInstall {
            path: PathBuf::from(path),
            major,
            version: version.to_owned(),
            arch,
            vendor: None,
            managed,
        }
    }

    #[test]
    fn required_major_defaults_to_8() {
        assert_eq!(required_major(Some(17)), 17);
        assert_eq!(required_major(None), 8);
    }

    #[test]
    fn version_ordering() {
        assert_eq!(compare_versions("17.0.15", "17.0.8"), Ordering::Greater);
        assert_eq!(compare_versions("1.8.0_202", "1.8.0_51"), Ordering::Greater);
        assert_eq!(compare_versions("21", "21.0.1"), Ordering::Less);
        assert_eq!(compare_versions("17.0.8", "17.0.8"), Ordering::Equal);
    }

    #[test]
    fn prefers_managed_then_newest() {
        let installs = vec![
            java("a", "17.0.15", 17, Arch::X64, false),
            java("b", "17.0.8", 17, Arch::X64, true),
            java("c", "21.0.7", 21, Arch::X64, true),
            java("d", "1.8.0_202", 8, Arch::X64, false),
            java("e", "1.8.0_51", 8, Arch::X64, false),
        ];
        assert_eq!(
            select(&installs, 17, Arch::X64).unwrap().path,
            PathBuf::from("b")
        );
        assert_eq!(
            select(&installs, 8, Arch::X64).unwrap().path,
            PathBuf::from("d")
        );
        assert_eq!(
            select(&installs, 21, Arch::X64).unwrap().path,
            PathBuf::from("c")
        );
    }

    #[test]
    fn missing_major_is_not_found() {
        let installs = vec![java("a", "21.0.7", 21, Arch::X64, false)];
        let err = select(&installs, 17, Arch::X64).unwrap_err();
        assert_eq!(err.code, ErrorCode::JavaNotFound);
        assert_eq!(err.params["major"], "17");
    }

    #[test]
    fn only_32_bit_is_arch_mismatch() {
        let installs = vec![java("x86", "1.8.0_202", 8, Arch::X86, false)];
        let err = select(&installs, 8, Arch::X64).unwrap_err();
        assert_eq!(err.code, ErrorCode::JavaArchMismatch);
        assert_eq!(err.params["expected"], "x64");
        assert_eq!(err.params["actual"], "x86");
    }

    #[test]
    fn manual_java_only_warns() {
        let ok = java("a", "17.0.8", 17, Arch::X64, false);
        assert!(check_manual(&ok, 17, Arch::X64).is_empty());

        let wrong = java("b", "1.8.0_202", 8, Arch::X86, false);
        let codes: Vec<ErrorCode> = check_manual(&wrong, 17, Arch::X64)
            .iter()
            .map(|e| e.code)
            .collect();
        assert_eq!(
            codes,
            vec![ErrorCode::JavaVersionMismatch, ErrorCode::JavaArchMismatch]
        );
    }
}
