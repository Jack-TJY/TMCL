//! 依赖库清单：客户端 jar、库主构件、旧式 natives 分类器与日志配置。
//!
//! 三种库格式（官方 `downloads`、Fabric 只给坐标、Forge 安装器生成）统一经
//! [`Library::file`] 定位；旧式 natives（`natives` + `classifiers`，1.19 以前）
//! 按当前系统挑出分类器，见 [`native_file`]。

use std::collections::BTreeSet;
use std::path::PathBuf;

use tmcl_net::DownloadItem;

use crate::install::GameFiles;
use crate::version::{LIBRARIES_URL, Library, LibraryFile, RuleEnv, VersionJson, maven_path};

/// 运行一个版本需要存在于磁盘上的文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameFile {
    /// 本地路径。
    pub path: PathBuf,
    /// 官方下载地址；`None` 表示由加载器安装器在本地生成，无法下载。
    pub url: Option<String>,
    /// 期望大小，未知时为 `None`。
    pub size: Option<u64>,
    /// 期望 SHA1，未知时为 `None`。
    pub sha1: Option<String>,
}

impl GameFile {
    /// 有下载地址时转换为下载项。
    pub fn download_item(&self) -> Option<DownloadItem> {
        Some(DownloadItem {
            url: self.url.clone()?,
            path: self.path.clone(),
            size: self.size,
            sha1: self.sha1.clone(),
        })
    }

    fn library(files: &GameFiles, file: LibraryFile) -> Self {
        Self {
            path: files.library(&file.path),
            url: file.url,
            // Forge 安装器生成的库在版本 JSON 中写作 `"size": 0`，表示未知。
            size: file.size.filter(|&s| s > 0),
            sha1: file.sha1,
        }
    }
}

/// 旧式 natives 在 `env` 下使用的分类器名，例如 `natives-windows`；
/// `${arch}` 按位数替换为 `32` 或 `64`。不是旧式 natives 库或不支持该系统时为 `None`。
pub fn native_classifier(library: &Library, env: &RuleEnv) -> Option<String> {
    let classifier = library.natives.as_ref()?.get(env.os_name)?;
    let bits = if env.arch == "x86" { "32" } else { "64" };
    Some(classifier.replace("${arch}", bits))
}

/// 旧式 natives 在 `env` 下要下载的 jar：优先 `downloads.classifiers`，没有时按坐标推算。
pub fn native_file(library: &Library, env: &RuleEnv) -> Option<LibraryFile> {
    let classifier = native_classifier(library, env)?;
    if let Some(downloads) = &library.downloads {
        let a = downloads.classifiers.as_ref()?.get(&classifier)?;
        return Some(LibraryFile {
            path: a.path.clone(),
            url: Some(a.url.clone()).filter(|u| !u.is_empty()),
            sha1: Some(a.sha1.clone()).filter(|s| !s.is_empty()),
            size: Some(a.size),
        });
    }
    let (coords, ext) = match library.name.split_once('@') {
        Some((coords, ext)) => (coords, format!("@{ext}")),
        None => (library.name.as_str(), String::new()),
    };
    let path = maven_path(&format!("{coords}:{classifier}{ext}"))?;
    let base = library.url.as_deref().unwrap_or(LIBRARIES_URL);
    Some(LibraryFile {
        url: Some(format!("{}/{path}", base.trim_end_matches('/'))),
        path,
        sha1: None,
        size: None,
    })
}

/// 运行 `version`（已经过 [`crate::resolve`] 合并）在 `env` 下所需的客户端 jar、
/// 依赖库（含旧式 natives）和日志配置，按路径去重，顺序与版本 JSON 一致。
pub fn library_files(files: &GameFiles, version: &VersionJson, env: &RuleEnv) -> Vec<GameFile> {
    let mut out = Vec::new();
    if let Some(downloads) = &version.downloads {
        out.push(GameFile {
            path: files.client_jar(version.jar_id()),
            url: Some(downloads.client.url.clone()),
            size: Some(downloads.client.size),
            sha1: Some(downloads.client.sha1.clone()),
        });
    }
    for library in version.libraries_for(env) {
        out.extend(library.file().map(|f| GameFile::library(files, f)));
        out.extend(native_file(library, env).map(|f| GameFile::library(files, f)));
    }
    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        out.push(GameFile {
            path: files.log_config(&log.file.id),
            url: Some(log.file.url.clone()),
            size: Some(log.file.size),
            sha1: Some(log.file.sha1.clone()),
        });
    }
    let mut seen = BTreeSet::new();
    out.retain(|f| seen.insert(f.path.clone()));
    out
}

/// [`library_files`] 中可下载的部分。
pub fn library_items(files: &GameFiles, version: &VersionJson, env: &RuleEnv) -> Vec<DownloadItem> {
    library_files(files, version, env)
        .iter()
        .filter_map(GameFile::download_item)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tmcl_common::DataPaths;

    fn library(json: &str) -> Library {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn picks_classifier_for_os_and_bits() {
        let lib = library(
            r#"{"name":"tv.twitch:twitch-platform:5.16",
                "natives":{"linux":"natives-linux","windows":"natives-windows-${arch}"}}"#,
        );
        assert_eq!(
            native_classifier(&lib, &RuleEnv::new("windows", "x86")).as_deref(),
            Some("natives-windows-32")
        );
        assert_eq!(
            native_classifier(&lib, &RuleEnv::new("windows", "x86_64")).as_deref(),
            Some("natives-windows-64")
        );
        assert_eq!(
            native_classifier(&lib, &RuleEnv::new("osx", "x86_64")),
            None
        );
        // 没有 downloads 时按坐标推算地址。
        let file = native_file(&lib, &RuleEnv::new("linux", "x86_64")).unwrap();
        assert_eq!(
            file.path,
            "tv/twitch/twitch-platform/5.16/twitch-platform-5.16-natives-linux.jar"
        );
        assert!(file.url.unwrap().starts_with(LIBRARIES_URL));
    }

    #[test]
    fn classifier_missing_from_downloads_is_skipped() {
        let lib = library(
            r#"{"name":"a:b:1","natives":{"osx":"natives-osx"},
                "downloads":{"classifiers":{"natives-linux":
                  {"path":"a/b/1/b-1-natives-linux.jar","sha1":"00","size":1,"url":"u"}}}}"#,
        );
        assert_eq!(native_file(&lib, &RuleEnv::new("osx", "x86_64")), None);
        assert!(native_file(&lib, &RuleEnv::new("linux", "x86_64")).is_none());
    }

    #[test]
    fn local_only_files_are_not_downloadable() {
        let files = GameFiles::new(DataPaths::at("root"));
        let version: VersionJson = serde_json::from_str(
            r#"{"id":"forge","mainClass":"M","jar":"1.20.1","libraries":[
                {"name":"net.minecraftforge:forge:1:client","downloads":{"artifact":
                  {"path":"net/minecraftforge/forge/1/forge-1-client.jar","sha1":"","size":0,"url":""}}},
                {"name":"a:b:1","url":"https://maven.example/"}]}"#,
        )
        .unwrap();
        let all = library_files(&files, &version, &RuleEnv::new("linux", "x86_64"));
        assert_eq!(all.len(), 2);
        assert_eq!(
            (all[0].url.as_deref(), all[0].sha1.as_deref()),
            (None, None)
        );
        let items = library_items(&files, &version, &RuleEnv::new("linux", "x86_64"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://maven.example/a/b/1/b-1.jar");
    }
}
