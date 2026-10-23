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
use tmcl_net::{DownloadItem, DownloadReport, Downloader, is_valid};

use crate::libraries::{GameFile, library_files};
use crate::manifest::{MANIFEST_URL, VersionManifest};
use crate::version::{MAX_INHERIT_DEPTH, RuleEnv, VersionJson, parse_version, resolve};

/// 官方资源文件下载地址前缀。
pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

/// 安装选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOptions {
    /// 版本清单地址。
    pub manifest_url: String,
    /// 资源文件地址前缀。
    pub resources_url: String,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            manifest_url: MANIFEST_URL.to_owned(),
            resources_url: RESOURCES_URL.to_owned(),
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

/// 下载并校验运行 `version_id` 所需的全部文件，已校验的文件不会重复下载；也用于修复。
///
/// 先沿 `inheritsFrom` 准备整条链上的版本 JSON：加载器版本（带 `inheritsFrom`）与清单中没有的
/// 自定义版本只用本地文件，原版按清单的 SHA1 校验、不符时重新下载；清单获取失败但本地已有
/// 版本 JSON 时继续使用本地文件。随后按 [`crate::resolve`] 合并，下载资源索引、客户端 jar、
/// 依赖库（含旧式 natives）、日志配置与资源文件。
///
/// 阶段依次为 `version_json`、`asset_index`、`libraries`、`assets`。没有下载地址
/// （由加载器安装器生成）的库缺失时返回 `GAME_FILE_MISSING`。
/// 连接数上限、重试与下载源都沿用 `downloader` 的设置。
pub async fn install_version(
    downloader: &Downloader,
    paths: &DataPaths,
    version_id: &str,
    options: &InstallOptions,
    ctx: &TaskCtx,
) -> AppResult<InstallReport> {
    let files = GameFiles::new(paths.clone());
    let mut report = InstallReport::default();

    ctx.report(ProgressUpdate::new("version_json").items(0, 1));
    let mut metadata = Metadata {
        downloader,
        files: &files,
        options,
        manifest: None,
    };
    let mut id = version_id.to_owned();
    for _ in 0..MAX_INHERIT_DEPTH {
        let version = metadata.ensure_json(&id, &mut report, ctx).await?;
        match version.inherits_from {
            Some(parent) => id = parent,
            None => break,
        }
    }
    let version = resolve(version_id, |id| files.load_version(id))?;
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
        report.add(single(downloader.download_one(&item, ctx).await?));
        let bytes = tokio::fs::read(&item.path).await.map_err(|e| {
            AppError::new(ErrorCode::CommonIo)
                .with_param("path", item.path.display())
                .with_detail(e.to_string())
        })?;
        let parsed: AssetIndex = serde_json::from_slice(&bytes)?;
        index_objects = asset_items(&files, &parsed, &options.resources_url);
        ctx.report(ProgressUpdate::new("asset_index").items(1, 1));
    }

    let library_files = library_files(&files, &version, &RuleEnv::current());
    let (downloadable, local): (Vec<_>, Vec<_>) =
        library_files.into_iter().partition(|f| f.url.is_some());
    for file in local {
        if !is_valid(&file.path, file.size, file.sha1.as_deref()).await? {
            return Err(AppError::new(ErrorCode::GameFileMissing)
                .with_param("version", version_id)
                .with_param("path", file.path.display()));
        }
        report.skipped += 1;
    }
    report.add(
        downloader
            .download_all(
                downloadable
                    .iter()
                    .filter_map(GameFile::download_item)
                    .collect(),
                "libraries",
                ctx,
            )
            .await?,
    );
    report.add(
        downloader
            .download_all(index_objects, "assets", ctx)
            .await?,
    );
    Ok(report)
}

/// 准备版本 JSON 时的状态：版本清单只在需要时获取一次。
struct Metadata<'a> {
    downloader: &'a Downloader,
    files: &'a GameFiles,
    options: &'a InstallOptions,
    manifest: Option<VersionManifest>,
}

impl Metadata<'_> {
    /// 确保 `id` 的版本 JSON 在本地且为最新，返回解析结果（未合并父版本）。
    async fn ensure_json(
        &mut self,
        id: &str,
        report: &mut InstallReport,
        ctx: &TaskCtx,
    ) -> AppResult<VersionJson> {
        let local = self.files.load_version(id).ok();
        if let Some(version) = &local
            && version.inherits_from.is_some()
        {
            return Ok(version.clone());
        }
        let manifest = match &self.manifest {
            Some(manifest) => manifest,
            None => match self
                .downloader
                .get_json::<VersionManifest>(&self.options.manifest_url, ctx)
                .await
            {
                Ok(manifest) => self.manifest.insert(manifest),
                Err(e) if e.is_cancelled() => return Err(e),
                Err(e) => {
                    return local.ok_or(e).inspect(|_| {
                        tracing::warn!(version = id, "获取版本清单失败，使用本地版本 JSON");
                    });
                }
            },
        };
        let entry = match manifest.find(id) {
            Ok(entry) => entry,
            Err(e) => return local.ok_or(e),
        };
        let item = DownloadItem {
            url: entry.url.clone(),
            path: self.files.version_json(id),
            size: None,
            sha1: Some(entry.sha1.clone()),
        };
        report.add(single(self.downloader.download_one(&item, ctx).await?));
        self.files.load_version(id)
    }
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
