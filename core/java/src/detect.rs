//! 枚举本机已安装的 Java。
//!
//! 来源：`JAVA_HOME`、`PATH`、Windows 注册表（JavaSoft / Eclipse Adoptium / Microsoft 等）、
//! 常见安装目录、官方启动器的 `runtime/`，以及启动器自己下载到 `java/` 下的运行时。
//! 同一个 Java 经不同来源找到时按真实路径去重。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tokio::task::JoinSet;

use crate::probe::{JAVA_EXE, JavaInstall, java_executable, probe};

/// 去哪里找 Java。测试中可以手工构造，实际使用 [`Sources::system`]。
#[derive(Debug, Clone, Default)]
pub struct Sources {
    /// 启动器自己的运行时目录（`<data_root>/java/`），其中找到的 Java 标记为 `managed`。
    pub managed_dir: Option<PathBuf>,
    /// 直接给出的 Java 主目录（`JAVA_HOME`、注册表等）。
    pub homes: Vec<PathBuf>,
    /// 直接给出的 `java` 可执行文件（`PATH` 中找到的）。
    pub executables: Vec<PathBuf>,
    /// 需要向下搜索的目录与最大深度。
    pub scan_roots: Vec<(PathBuf, usize)>,
}

/// 搜索启动器 `java/` 目录的深度：`java/<runtime>/<平台>/<runtime>/jre.bundle/Contents/Home`。
const MANAGED_DEPTH: usize = 6;
/// 搜索常见安装目录的深度：`Java/<jdk>/` 或 `JavaVirtualMachines/<jdk>/Contents/Home`。
const INSTALL_DEPTH: usize = 3;

impl Sources {
    /// 本机的全部来源；`managed_dir` 为 `<data_root>/java/`。
    pub fn system(managed_dir: &Path) -> Self {
        let mut sources = Self {
            managed_dir: Some(managed_dir.to_path_buf()),
            ..Self::default()
        };
        if let Some(home) = std::env::var_os("JAVA_HOME").filter(|v| !v.is_empty()) {
            sources.homes.push(PathBuf::from(home));
        }
        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                sources.executables.push(dir.join(JAVA_EXE));
            }
        }
        #[cfg(windows)]
        sources.homes.extend(registry::java_homes());
        sources.scan_roots.extend(install_roots());
        sources
    }
}

/// 找出所有可用的 Java，启动器管理的排在前面，其余按主版本从高到低。
pub async fn detect(sources: &Sources) -> Vec<JavaInstall> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.extend(sources.homes.iter().map(|h| java_executable(h)));
    candidates.extend(sources.executables.iter().cloned());
    for (root, depth) in &sources.scan_roots {
        candidates.extend(scan(root, *depth));
    }
    if let Some(dir) = &sources.managed_dir {
        candidates.extend(scan(dir, MANAGED_DEPTH));
    }
    let managed_dir = sources
        .managed_dir
        .as_deref()
        .and_then(|d| std::fs::canonicalize(d).ok());

    let mut seen = HashSet::new();
    let mut set = JoinSet::new();
    for exe in candidates {
        let Ok(real) = std::fs::canonicalize(&exe) else {
            continue;
        };
        if !real.is_file() || is_oracle_javapath(&real) || !seen.insert(real.clone()) {
            continue;
        }
        let managed = managed_dir.as_deref().is_some_and(|d| real.starts_with(d));
        let path = simplify(&real);
        set.spawn(async move { probe(&path, managed).await });
    }

    let mut installs = Vec::new();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(Ok(install)) => installs.push(install),
            Ok(Err(e)) => tracing::debug!(error = %e, "跳过无法识别的 Java"),
            Err(e) => tracing::warn!(error = %e, "Java 探测任务异常结束"),
        }
    }
    installs.sort_by(|a, b| {
        b.managed
            .cmp(&a.managed)
            .then(b.major.cmp(&a.major))
            .then_with(|| a.path.cmp(&b.path))
    });
    installs
}

/// 在 `dir` 下最多 `depth` 层内寻找 Java 主目录，返回其中的 `java` 可执行文件。
pub(crate) fn scan(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    scan_into(dir, depth, &mut found);
    found
}

fn scan_into(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    let exe = java_executable(dir);
    if exe.is_file() {
        found.push(exe);
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            scan_into(&entry.path(), depth - 1, found);
        }
    }
}

/// Oracle 安装程序放在 `Common Files\Oracle\Java\javapath*` 下的转发程序，
/// 实际运行的是注册表里的 JRE，会与之重复，跳过。
fn is_oracle_javapath(exe: &Path) -> bool {
    exe.parent().and_then(Path::file_name).is_some_and(|n| {
        n.to_string_lossy()
            .to_ascii_lowercase()
            .starts_with("javapath")
    })
}

/// 去掉 Windows `canonicalize` 产生的 `\\?\` 前缀，便于显示与传给子进程。
fn simplify(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

/// 各平台常见的 Java 安装目录与官方启动器的运行时目录。
fn install_roots() -> Vec<(PathBuf, usize)> {
    let mut roots = Vec::new();
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    if let Some(home) = &home {
        roots.push((home.join(".jdks"), INSTALL_DEPTH));
    }
    if cfg!(windows) {
        let vendors = [
            "Java",
            "Eclipse Adoptium",
            "Eclipse Foundation",
            "AdoptOpenJDK",
            "Microsoft",
            "Zulu",
            "BellSoft",
            "Amazon Corretto",
            "Semeru",
        ];
        let mut program_dirs: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
            .iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from)
            .collect();
        program_dirs.dedup();
        for dir in program_dirs {
            roots.extend(vendors.iter().map(|v| (dir.join(v), INSTALL_DEPTH)));
        }
        if let Some(appdata) = std::env::var_os("APPDATA") {
            roots.push((
                PathBuf::from(appdata).join(".minecraft").join("runtime"),
                MANAGED_DEPTH,
            ));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            // 微软商店版官方启动器。
            let store = PathBuf::from(local)
                .join(r"Packages\Microsoft.4297127D64EC6_8wekyb3d8bbwe\LocalCache\Local\runtime");
            roots.push((store, MANAGED_DEPTH));
        }
    } else if cfg!(target_os = "macos") {
        roots.push((
            PathBuf::from("/Library/Java/JavaVirtualMachines"),
            INSTALL_DEPTH,
        ));
        if let Some(home) = &home {
            roots.push((home.join("Library/Java/JavaVirtualMachines"), INSTALL_DEPTH));
            roots.push((
                home.join("Library/Application Support/minecraft/runtime"),
                MANAGED_DEPTH,
            ));
        }
    } else {
        for dir in [
            "/usr/lib/jvm",
            "/usr/lib64/jvm",
            "/usr/java",
            "/opt/java",
            "/opt/jdk",
        ] {
            roots.push((PathBuf::from(dir), INSTALL_DEPTH));
        }
        if let Some(home) = &home {
            roots.push((home.join(".sdkman/candidates/java"), INSTALL_DEPTH));
            roots.push((home.join(".minecraft/runtime"), MANAGED_DEPTH));
        }
    }
    roots
}

#[cfg(windows)]
mod registry {
    //! 读取 Windows 注册表中登记的 Java 主目录（64 位与 32 位视图都查）。

    use std::path::PathBuf;

    use winreg::RegKey;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };

    /// 各发行商的注册表根键（位于 `SOFTWARE\` 下）。
    const ROOTS: &[&str] = &[
        r"JavaSoft",
        r"Eclipse Adoptium",
        r"Eclipse Foundation",
        r"AdoptOpenJDK",
        r"Microsoft\JDK",
        r"Azul Systems\Zulu",
        r"BellSoft\Liberica",
        r"Amazon Corretto",
    ];
    /// 存放主目录的值名。
    const VALUES: &[&str] = &["JavaHome", "Path", "InstallationPath"];
    const MAX_DEPTH: usize = 4;

    pub(super) fn java_homes() -> Vec<PathBuf> {
        let mut homes = Vec::new();
        for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
                for root in ROOTS {
                    let path = format!(r"SOFTWARE\{root}");
                    if let Ok(key) =
                        RegKey::predef(hive).open_subkey_with_flags(&path, KEY_READ | view)
                    {
                        walk(&key, view, MAX_DEPTH, &mut homes);
                    }
                }
            }
        }
        homes
    }

    fn walk(key: &RegKey, view: u32, depth: usize, homes: &mut Vec<PathBuf>) {
        for name in VALUES {
            if let Ok(value) = key.get_value::<String, _>(name)
                && !value.trim().is_empty()
            {
                homes.push(PathBuf::from(value.trim()));
            }
        }
        if depth == 0 {
            return;
        }
        for sub in key.enum_keys().flatten() {
            if let Ok(child) = key.open_subkey_with_flags(&sub, KEY_READ | view) {
                walk(&child, view, depth - 1, homes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_java(home: &Path, version: &str, arch: &str) -> PathBuf {
        let exe = java_executable(home);
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        std::fs::write(
            home.join("release"),
            format!("JAVA_VERSION=\"{version}\"\nOS_ARCH=\"{arch}\"\n"),
        )
        .unwrap();
        exe
    }

    #[tokio::test]
    async fn finds_dedupes_and_sorts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let jdk8 = root.join("installs/jdk8");
        let exe8 = fake_java(&jdk8, "1.8.0_202", "amd64");
        fake_java(&root.join("installs/vendor/jdk-21"), "21.0.7", "x86_64");
        // 官方启动器式的深层目录。
        fake_java(
            &root.join("java/java-runtime-gamma/windows-x64/java-runtime-gamma"),
            "17.0.15",
            "amd64",
        );
        // 不是 Java 的目录与损坏的 Java 都被忽略。
        std::fs::create_dir_all(root.join("installs/empty/bin")).unwrap();
        let broken = java_executable(&root.join("installs/broken"));
        std::fs::create_dir_all(broken.parent().unwrap()).unwrap();
        std::fs::write(&broken, b"").unwrap();

        let sources = Sources {
            managed_dir: Some(root.join("java")),
            // 同一个 Java 经主目录、可执行文件和目录扫描三处找到。
            homes: vec![jdk8.clone(), root.join("missing")],
            executables: vec![exe8],
            scan_roots: vec![(root.join("installs"), INSTALL_DEPTH)],
        };
        let installs = detect(&sources).await;
        let summary: Vec<(u32, bool)> = installs.iter().map(|i| (i.major, i.managed)).collect();
        assert_eq!(summary, vec![(17, true), (21, false), (8, false)]);
        assert!(
            installs
                .iter()
                .all(|i| !i.path.to_string_lossy().starts_with(r"\\?\"))
        );
    }

    #[tokio::test]
    async fn empty_sources_find_nothing() {
        assert!(detect(&Sources::default()).await.is_empty());
    }

    #[test]
    fn simplify_strips_verbatim_prefix() {
        assert_eq!(
            simplify(Path::new(r"\\?\C:\Java\bin\java.exe")),
            PathBuf::from(r"C:\Java\bin\java.exe")
        );
        assert_eq!(
            simplify(Path::new(r"\\?\UNC\server\share")),
            PathBuf::from(r"\\?\UNC\server\share")
        );
        assert_eq!(
            simplify(Path::new("/usr/bin/java")),
            PathBuf::from("/usr/bin/java")
        );
    }

    #[tokio::test]
    #[ignore = "依赖本机安装的 Java，手动运行"]
    async fn system_detection() {
        let dir = tempfile::tempdir().unwrap();
        for install in detect(&Sources::system(dir.path())).await {
            println!("{install:?}");
        }
    }
}
