//! 版本文件补全：版本 JSON、客户端 jar、依赖库、日志配置、资源索引与资源文件。
//!
//! 文件放在 `<data_root>/shared/` 下，供所有实例共享：
//!
//! ```text
//! shared/versions/<id>/<id>.json, <id>.jar
//! shared/libraries/<artifact.path>
//! shared/assets/indexes/<index>.json
//! shared/assets/objects/<hash 前两位>/<hash>
//! shared/assets/log_configs/<file>
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Deserialize;
use tmcl_common::{AppError, AppResult, DataPaths, ErrorCode, ProgressUpdate, TaskCtx};
use tmcl_net::{DownloadItem, DownloadReport, HttpClient, download_all, download_one};

use crate::manifest::{MANIFEST_URL, VersionManifest};
use crate::version::{RuleEnv, VersionJson};

/// 官方资源文件下载地址前缀。
pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

/// 安装选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOptions {
    /// 版本清单地址。
    pub manifest_url: String,
    /// 资源文件地址前缀。
    pub resources_url: String,
    /// 同时下载数。
    pub concurrency: usize,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            manifest_url: MANIFEST_URL.to_owned(),
            resources_url: RESOURCES_URL.to_owned(),
            concurrency: 16,
        }
    }
}

/// 一次补全的统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InstallReport {
    /// 实际下载的文件数。
    pub downloaded: u64,
    /// 已存在且校验通过而跳过的文件数。
    pub skipped: u64,
    /// 实际下载的字节数。
    pub bytes: u64,
}

impl InstallReport {
    fn add(&mut self, r: DownloadReport) {
        self.downloaded += r.downloaded;
        self.skipped += r.skipped;
        self.bytes += r.bytes;
    }
}

/// 资源索引文件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssetIndex {
    /// 资源名 → 对象。
    pub objects: BTreeMap<String, AssetObject>,
}

/// 资源对象。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssetObject {
    /// SHA1，同时是存储文件名。
    pub hash: String,
    /// 大小。
    pub size: u64,
}

/// `shared/` 下各类游戏文件的路径。
#[derive(Debug, Clone)]
pub struct GameFiles {
    paths: DataPaths,
}

impl GameFiles {
    /// 基于数据目录创建。
    pub fn new(paths: DataPaths) -> Self {
        Self { paths }
    }

    /// `shared/versions/<id>/<id>.json`。
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.paths
            .shared_versions_dir()
            .join(id)
            .join(format!("{id}.json"))
    }

    /// `shared/versions/<id>/<id>.jar`。
    pub fn client_jar(&self, id: &str) -> PathBuf {
        self.paths
            .shared_versions_dir()
            .join(id)
            .join(format!("{id}.jar"))
    }

    /// `shared/libraries/`。
    pub fn libraries_dir(&self) -> PathBuf {
        self.paths.shared_libraries_dir()
    }

    /// `shared/libraries/<path>`，`path` 使用 `/` 分隔。
    pub fn library(&self, path: &str) -> PathBuf {
        path.split('/')
            .fold(self.libraries_dir(), |acc, part| acc.join(part))
    }

    /// `shared/assets/`，即 `--assetsDir`。
    pub fn assets_dir(&self) -> PathBuf {
        self.paths.shared_assets_dir()
    }

    /// `shared/assets/indexes/<id>.json`。
    pub fn asset_index(&self, id: &str) -> PathBuf {
        self.assets_dir().join("indexes").join(format!("{id}.json"))
    }

    /// `shared/assets/objects/<hh>/<hash>`。
    pub fn asset_object(&self, hash: &str) -> PathBuf {
        self.assets_dir()
            .join("objects")
            .join(hash.get(..2).unwrap_or(hash))
            .join(hash)
    }

    /// `shared/assets/log_configs/<file>`。
    pub fn log_config(&self, file: &str) -> PathBuf {
        self.assets_dir().join("log_configs").join(file)
    }

    /// 读取并解析本地版本 JSON。
    pub fn load_version(&self, id: &str) -> AppResult<VersionJson> {
        let path = self.version_json(id);
        let bytes = std::fs::read(&path).map_err(|e| {
            AppError::new(ErrorCode::CommonIo)
                .with_param("path", path.display())
                .with_detail(e.to_string())
        })?;
        parse_version(id, &bytes)
    }
}

/// 解析版本 JSON；格式错误时返回 `GAME_VERSION_JSON_INVALID`。
pub fn parse_version(id: &str, bytes: &[u8]) -> AppResult<VersionJson> {
    serde_json::from_slice(bytes).map_err(|e| {
        AppError::new(ErrorCode::GameVersionJsonInvalid)
            .with_param("version", id)
            .with_detail(e.to_string())
    })
}

/// 下载并校验运行 `version_id` 所需的全部文件，已校验的文件不会重复下载。
///
/// 阶段依次为 `version_json`、`asset_index`、`libraries`（含客户端 jar 与日志配置）、`assets`。
pub async fn install_version(
    client: &HttpClient,
    paths: &DataPaths,
    version_id: &str,
    options: &InstallOptions,
    ctx: &TaskCtx,
) -> AppResult<InstallReport> {
    let files = GameFiles::new(paths.clone());
    let mut report = InstallReport::default();

    ctx.report(ProgressUpdate::new("version_json").items(0, 1));
    let manifest = VersionManifest::fetch(client, &options.manifest_url, ctx).await?;
    let entry = manifest.find(version_id)?;
    let json_item = DownloadItem {
        url: entry.url.clone(),
        path: files.version_json(version_id),
        size: None,
        sha1: Some(entry.sha1.clone()),
    };
    report.add(single(download_one(client, &json_item, ctx).await?));
    let version = files.load_version(version_id)?;
    ctx.report(ProgressUpdate::new("version_json").items(1, 1));

    let mut index_objects = Vec::new();
    if let Some(index) = &version.asset_index {
        ctx.report(ProgressUpdate::new("asset_index").items(0, 1));
        let item = DownloadItem {
            url: index.url.clone(),
            path: files.asset_index(&index.id),
            size: Some(index.size),
            sha1: Some(index.sha1.clone()),
        };
        report.add(single(download_one(client, &item, ctx).await?));
        let bytes = tokio::fs::read(&item.path).await.map_err(|e| {
            AppError::new(ErrorCode::CommonIo)
                .with_param("path", item.path.display())
                .with_detail(e.to_string())
        })?;
        let parsed: AssetIndex = serde_json::from_slice(&bytes)?;
        index_objects = asset_items(&files, &parsed, &options.resources_url);
        ctx.report(ProgressUpdate::new("asset_index").items(1, 1));
    }

    let library_items = library_items(&files, &version, &RuleEnv::current());
    report.add(download_all(client, library_items, options.concurrency, "libraries", ctx).await?);
    report.add(download_all(client, index_objects, options.concurrency, "assets", ctx).await?);
    Ok(report)
}

/// 客户端 jar、适用于 `env` 的依赖库和日志配置。
pub fn library_items(files: &GameFiles, version: &VersionJson, env: &RuleEnv) -> Vec<DownloadItem> {
    let mut items = Vec::new();
    if let Some(downloads) = &version.downloads {
        items.push(DownloadItem {
            url: downloads.client.url.clone(),
            path: files.client_jar(&version.id),
            size: Some(downloads.client.size),
            sha1: Some(downloads.client.sha1.clone()),
        });
    }
    let mut seen = BTreeSet::new();
    for library in version.libraries.iter().filter(|l| l.applies(env)) {
        let Some(artifact) = library.artifact() else {
            continue;
        };
        if !seen.insert(artifact.path.as_str()) {
            continue;
        }
        items.push(DownloadItem {
            url: artifact.url.clone(),
            path: files.library(&artifact.path),
            size: Some(artifact.size),
            sha1: Some(artifact.sha1.clone()),
        });
    }
    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        items.push(DownloadItem {
            url: log.file.url.clone(),
            path: files.log_config(&log.file.id),
            size: Some(log.file.size),
            sha1: Some(log.file.sha1.clone()),
        });
    }
    items
}

/// 资源索引中的对象，按哈希去重。
pub fn asset_items(
    files: &GameFiles,
    index: &AssetIndex,
    resources_url: &str,
) -> Vec<DownloadItem> {
    let mut seen = BTreeSet::new();
    index
        .objects
        .values()
        .filter(|o| seen.insert(o.hash.as_str()))
        .map(|o| DownloadItem {
            url: format!(
                "{}/{}/{}",
                resources_url.trim_end_matches('/'),
                o.hash.get(..2).unwrap_or(&o.hash),
                o.hash
            ),
            path: files.asset_object(&o.hash),
            size: Some(o.size),
            sha1: Some(o.hash.clone()),
        })
        .collect()
}

fn single(outcome: tmcl_net::Outcome) -> DownloadReport {
    match outcome {
        tmcl_net::Outcome::Downloaded(bytes) => DownloadReport {
            downloaded: 1,
            skipped: 0,
            bytes,
        },
        tmcl_net::Outcome::Skipped => DownloadReport {
            downloaded: 0,
            skipped: 1,
            bytes: 0,
        },
    }
}
