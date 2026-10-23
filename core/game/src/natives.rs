//! natives 解压：每次启动前把本地库从 jar 中解压到一个新的临时目录（`-Djava.library.path`）。
//!
//! - 旧式（1.19 以前，`natives` + `classifiers`）：解压分类器 jar 中除 `extract.exclude`
//!   前缀以外的全部文件，保留相对路径；
//! - 新式（LWJGL 3.3 起，坐标分类器为 `natives-*` 的普通库）：只取 `.dll`、`.so`、`.dylib`、
//!   `.jnilib`，去掉 jar 内的目录层级放在根目录，LWJGL 从 `java.library.path` 直接加载。

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tmcl_common::{AppError, AppResult, DataPaths, ErrorCode};
use zip::ZipArchive;

use crate::install::GameFiles;
use crate::libraries::native_file;
use crate::version::{RuleEnv, VersionJson};

/// 新式 natives jar 中需要解压的文件扩展名。
const NATIVE_EXTENSIONS: [&str; 4] = ["dll", "so", "dylib", "jnilib"];

/// 一个需要解压的 natives jar。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeJar {
    /// jar 路径。
    pub path: PathBuf,
    /// 排除的条目前缀（旧式 `extract.exclude`）。
    pub exclude: Vec<String>,
    /// 新式 natives：只取本地库文件并放到根目录。
    pub flatten: bool,
}

/// `version` 在 `env` 下需要解压的 natives jar，按版本 JSON 中的顺序。
pub fn native_jars(files: &GameFiles, version: &VersionJson, env: &RuleEnv) -> Vec<NativeJar> {
    let mut jars = Vec::new();
    for library in version.libraries_for(env) {
        if let Some(file) = native_file(library, env) {
            jars.push(NativeJar {
                path: files.library(&file.path),
                exclude: library
                    .extract
                    .as_ref()
                    .map(|e| e.exclude.clone())
                    .unwrap_or_default(),
                flatten: false,
            });
        } else if is_native_classifier(&library.name)
            && let Some(file) = library.file()
        {
            jars.push(NativeJar {
                path: files.library(&file.path),
                exclude: Vec::new(),
                flatten: true,
            });
        }
    }
    jars
}

fn is_native_classifier(name: &str) -> bool {
    let coords = name.split_once('@').map_or(name, |(c, _)| c);
    coords
        .split(':')
        .nth(3)
        .is_some_and(|c| c.starts_with("natives-"))
}

/// 为一次启动新建的 natives 目录路径：`cache/natives/<版本>-<时间戳>-<进程号>-<序号>/`，
/// 用完由调用方删除。
pub fn new_natives_dir(paths: &DataPaths, version_id: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    paths
        .cache_dir()
        .join("natives")
        .join(format!("{version_id}-{nanos}-{}-{seq}", std::process::id()))
}

/// 把 `version` 在 `env` 下的 natives 解压到 `dest`，返回解压出的文件数。
///
/// 这是阻塞操作，异步代码中应放进 `spawn_blocking`。jar 缺失或损坏时返回 `COMMON_IO`；
/// 会跳出 `dest` 的条目被忽略。
pub fn extract_natives(
    files: &GameFiles,
    version: &VersionJson,
    env: &RuleEnv,
    dest: &Path,
) -> AppResult<usize> {
    create_dir(dest)?;
    let mut count = 0;
    for jar in native_jars(files, version, env) {
        count += extract_jar(&jar, dest)?;
    }
    Ok(count)
}

fn extract_jar(jar: &NativeJar, dest: &Path) -> AppResult<usize> {
    let file = File::open(&jar.path).map_err(|e| io_error(&jar.path, &e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| zip_error(&jar.path, e))?;
    let mut count = 0;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| zip_error(&jar.path, e))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let name = relative.to_string_lossy().replace('\\', "/");
        if entry.is_dir() || jar.exclude.iter().any(|p| name.starts_with(p.as_str())) {
            continue;
        }
        let target = if jar.flatten {
            let is_native = relative
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| NATIVE_EXTENSIONS.contains(&e));
            match relative.file_name() {
                Some(name) if is_native => dest.join(name),
                _ => continue,
            }
        } else {
            dest.join(relative)
        };
        if let Some(parent) = target.parent() {
            create_dir(parent)?;
        }
        let mut out = File::create(&target).map_err(|e| io_error(&target, &e))?;
        io::copy(&mut entry, &mut out).map_err(|e| io_error(&target, &e))?;
        count += 1;
    }
    Ok(count)
}

fn create_dir(dir: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))
}

fn io_error(path: &Path, err: &io::Error) -> AppError {
    AppError::new(ErrorCode::CommonIo)
        .with_param("path", path.display())
        .with_detail(err.to_string())
}

fn zip_error(path: &Path, err: zip::result::ZipError) -> AppError {
    AppError::new(ErrorCode::CommonIo)
        .with_param("path", path.display())
        .with_detail(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_new_style_natives() {
        assert!(is_native_classifier(
            "org.lwjgl:lwjgl:3.3.1:natives-windows"
        ));
        assert!(is_native_classifier(
            "org.lwjgl:lwjgl:3.3.1:natives-macos-arm64"
        ));
        assert!(!is_native_classifier("org.lwjgl:lwjgl:3.3.1"));
        assert!(!is_native_classifier("a:b:1:client"));
    }

    #[test]
    fn natives_dirs_are_unique_under_cache() {
        let paths = DataPaths::at("root");
        let a = new_natives_dir(&paths, "1.20.1");
        let b = new_natives_dir(&paths, "1.20.1");
        assert_ne!(a, b);
        assert!(a.starts_with(paths.cache_dir().join("natives")));
    }
}
