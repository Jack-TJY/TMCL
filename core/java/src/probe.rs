//! 识别一个 Java 的版本、架构与发行商。
//!
//! 优先读取 Java 主目录下的 `release` 文件（不启动进程，最快）；
//! 读不到或缺字段时运行 `java -XshowSettings:properties -version` 解析系统属性。

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode};
use ts_rs::TS;

/// 运行 `java` 获取属性的超时时间。
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// CPU 架构，序列化为小写字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    /// x86-64（`amd64`、`x86_64`）。
    X64,
    /// 32 位 x86（`i386`–`i686`）。
    X86,
    /// 64 位 ARM（`aarch64`）。
    Arm64,
    /// 32 位 ARM。
    Arm32,
    /// 无法识别。
    Unknown,
}

impl Arch {
    /// 当前程序运行的架构。
    pub fn host() -> Self {
        Self::parse(std::env::consts::ARCH)
    }

    /// 解析 `os.arch`、`OS_ARCH` 或 Rust 的架构名。
    pub fn parse(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "amd64" | "x86_64" | "x64" | "x86-64" => Self::X64,
            "x86" | "i386" | "i486" | "i586" | "i686" => Self::X86,
            "aarch64" | "arm64" => Self::Arm64,
            "arm" | "aarch32" | "armv7" | "armv7l" | "armhf" => Self::Arm32,
            _ => Self::Unknown,
        }
    }

    /// 序列化后的名称，例如 `x64`。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::X86 => "x86",
            Self::Arm64 => "arm64",
            Self::Arm32 => "arm32",
            Self::Unknown => "unknown",
        }
    }
}

/// 一个可用的 Java。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct JavaInstall {
    /// `java` 可执行文件的路径（Windows 为 `bin\java.exe`）。
    #[ts(type = "string")]
    pub path: PathBuf,
    /// 主版本号，例如 8、17、21。
    pub major: u32,
    /// 完整版本号，例如 `1.8.0_202`、`17.0.8`。
    pub version: String,
    /// 架构。
    pub arch: Arch,
    /// 发行商，例如 `Eclipse Adoptium`；未知时为 `None`。
    pub vendor: Option<String>,
    /// 是否为启动器下载到 `java/` 下的运行时。
    pub managed: bool,
}

/// 从 `release` 文件或系统属性解析出的原始信息。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct JavaInfo {
    pub version: Option<String>,
    pub arch: Option<String>,
    pub vendor: Option<String>,
}

impl JavaInfo {
    fn complete(&self) -> bool {
        self.version.is_some() && self.arch.is_some()
    }
}

/// `java` 可执行文件的文件名。
pub const JAVA_EXE: &str = if cfg!(windows) { "java.exe" } else { "java" };

/// `java` 可执行文件在主目录下的位置：`<home>/bin/java`。
pub fn java_executable(home: &Path) -> PathBuf {
    home.join("bin").join(JAVA_EXE)
}

/// 由 `bin/java` 推出 Java 主目录。
pub fn java_home(executable: &Path) -> Option<&Path> {
    executable.parent()?.parent()
}

/// 识别 `executable` 指向的 Java；`managed` 原样写入结果。
pub async fn probe(executable: &Path, managed: bool) -> AppResult<JavaInstall> {
    let failed =
        || AppError::new(ErrorCode::JavaProbeFailed).with_param("java", executable.display());
    if !executable.is_file() {
        return Err(failed().with_detail("file not found"));
    }

    let mut info = match java_home(executable) {
        Some(home) => read_release(home).await.unwrap_or_default(),
        None => JavaInfo::default(),
    };
    if !info.complete() {
        let props = run_properties(executable)
            .await
            .map_err(|e| failed().with_detail(e))?;
        info.version = info.version.or(props.version);
        info.arch = info.arch.or(props.arch);
        info.vendor = info.vendor.or(props.vendor);
    }

    let version = info
        .version
        .ok_or_else(|| failed().with_detail("no java.version"))?;
    let major = parse_major(&version)
        .ok_or_else(|| failed().with_detail(format!("bad version {version}")))?;
    Ok(JavaInstall {
        path: executable.to_path_buf(),
        major,
        version,
        arch: info.arch.as_deref().map_or(Arch::Unknown, Arch::parse),
        vendor: info.vendor,
        managed,
    })
}

/// 由完整版本号求主版本号：`1.8.0_202` → 8，`17.0.8` → 17，`21-ea` → 21。
pub fn parse_major(version: &str) -> Option<u32> {
    let mut parts = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u32>().ok());
    match parts.next()?? {
        1 => parts.next()?,
        n => Some(n),
    }
}

async fn read_release(home: &Path) -> Option<JavaInfo> {
    let text = tokio::fs::read_to_string(home.join("release")).await.ok()?;
    Some(parse_release(&text))
}

/// 解析 `release` 文件（`KEY="value"` 每行一项）。
pub(crate) fn parse_release(text: &str) -> JavaInfo {
    let mut info = JavaInfo::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "JAVA_VERSION" => info.version = Some(value.to_owned()),
            "OS_ARCH" => info.arch = Some(value.to_owned()),
            "IMPLEMENTOR" => info.vendor = Some(value.to_owned()),
            _ => {}
        }
    }
    info
}

/// 解析 `-XshowSettings:properties` 输出中的 `key = value` 行。
pub(crate) fn parse_properties(text: &str) -> JavaInfo {
    let mut info = JavaInfo::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "java.version" => info.version = Some(value.to_owned()),
            "os.arch" => info.arch = Some(value.to_owned()),
            "java.vendor" => info.vendor = Some(value.to_owned()),
            _ => {}
        }
    }
    info
}

async fn run_properties(executable: &Path) -> Result<JavaInfo, String> {
    let mut command = tokio::process::Command::new(executable);
    command
        .args(["-XshowSettings:properties", "-version"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW：探测时不弹出控制台窗口。
        command.creation_flags(0x0800_0000);
    }
    let output = tokio::time::timeout(PROBE_TIMEOUT, command.output())
        .await
        .map_err(|_| "timed out".to_owned())?
        .map_err(|e| e.to_string())?;
    // 属性打印在 stderr；个别发行版打印到 stdout，两者都看。
    let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    let info = parse_properties(&text);
    if info.version.is_none() {
        return Err(format!("exit status {}", output.status));
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn major_versions() {
        assert_eq!(parse_major("1.8.0_202"), Some(8));
        assert_eq!(parse_major("1.7.0"), Some(7));
        assert_eq!(parse_major("17.0.8"), Some(17));
        assert_eq!(parse_major("21"), Some(21));
        assert_eq!(parse_major("21-ea"), Some(21));
        assert_eq!(parse_major("8u51"), Some(8));
        assert_eq!(parse_major("abc"), None);
    }

    #[test]
    fn arch_names() {
        assert_eq!(Arch::parse("amd64"), Arch::X64);
        assert_eq!(Arch::parse("x86_64"), Arch::X64);
        assert_eq!(Arch::parse("i586"), Arch::X86);
        assert_eq!(Arch::parse("x86"), Arch::X86);
        assert_eq!(Arch::parse("aarch64"), Arch::Arm64);
        assert_eq!(Arch::parse("sparcv9"), Arch::Unknown);
        assert_ne!(Arch::host(), Arch::Unknown);
    }

    #[test]
    fn release_file() {
        let text = "IMPLEMENTOR=\"Eclipse Adoptium\"\nJAVA_VERSION=\"17.0.8\"\nOS_ARCH=\"x86_64\"\nOS_NAME=\"Windows\"\n";
        let info = parse_release(text);
        assert_eq!(info.version.as_deref(), Some("17.0.8"));
        assert_eq!(info.arch.as_deref(), Some("x86_64"));
        assert_eq!(info.vendor.as_deref(), Some("Eclipse Adoptium"));

        // Oracle 8 的 release 没有 IMPLEMENTOR。
        let info = parse_release("JAVA_VERSION=\"1.8.0_202\"\r\nOS_ARCH=\"i586\"\r\n");
        assert_eq!(info.version.as_deref(), Some("1.8.0_202"));
        assert_eq!(info.vendor, None);
    }

    #[test]
    fn properties_output() {
        let text = "Property settings:\n    file.encoding = UTF-8\n    java.vendor = Oracle Corporation\n    java.version = 1.8.0_202\n    os.arch = amd64\n\njava version \"1.8.0_202\"\n";
        let info = parse_properties(text);
        assert_eq!(info.version.as_deref(), Some("1.8.0_202"));
        assert_eq!(info.arch.as_deref(), Some("amd64"));
        assert_eq!(info.vendor.as_deref(), Some("Oracle Corporation"));
    }

    #[tokio::test]
    async fn probe_reads_release_without_running_java() {
        let dir = tempfile::tempdir().unwrap();
        let exe = java_executable(dir.path());
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        std::fs::write(
            dir.path().join("release"),
            "JAVA_VERSION=\"21.0.7\"\nOS_ARCH=\"aarch64\"\nIMPLEMENTOR=\"Microsoft\"\n",
        )
        .unwrap();

        let install = probe(&exe, true).await.unwrap();
        assert_eq!(install.major, 21);
        assert_eq!(install.version, "21.0.7");
        assert_eq!(install.arch, Arch::Arm64);
        assert_eq!(install.vendor.as_deref(), Some("Microsoft"));
        assert!(install.managed);
    }

    #[tokio::test]
    async fn probe_missing_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let err = probe(&java_executable(dir.path()), false)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::JavaProbeFailed);
    }

    #[tokio::test]
    async fn probe_not_a_java_fails() {
        // 空文件不是可执行程序，也没有 release，应返回探测失败而不是崩溃。
        let dir = tempfile::tempdir().unwrap();
        let exe = java_executable(dir.path());
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        let err = probe(&exe, false).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::JavaProbeFailed);
    }
}
