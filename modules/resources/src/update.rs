//! 已安装资源的管理：列出（按 SHA1 在 Modrinth 反查来源）、从本地文件导入、
//! 启用/禁用（`.disabled` 后缀）、删除、检查更新与更新。
//!
//! 更新只替换文件本身：新版本新增的必需前置不会自动安装（可对该版本再走一次安装计划）。
//! 被禁用的资源也会检查更新，更新后保持禁用。

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::Downloader;
use ts_rs::TS;

use crate::browse::{InstanceTarget, ResourceKind, ResourceVersion};
use crate::install::{
    DISABLED_SUFFIX, INSTALL_STAGE, LocalFile, file_name, io_error, is_resource_file, kind_dir,
    local_files,
};
use crate::modrinth::{HashAlgorithm, ModrinthClient, Version, VersionFilter};

/// 已安装资源在 Modrinth 上的来源。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ResourceSource {
    /// 项目 ID。
    pub project_id: String,
    /// 版本 ID。
    pub version_id: String,
    /// 版本号。
    pub version_number: String,
    /// 项目标题；取不到时为项目 ID。
    pub title: String,
    /// 项目图标地址。
    pub icon_url: Option<String>,
}

/// 一个已安装的资源文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct InstalledResource {
    /// 磁盘上的文件名（禁用时带 `.disabled` 后缀）。
    pub file_name: String,
    /// 是否启用。
    pub enabled: bool,
    /// 文件大小（字节）。
    #[ts(type = "number")]
    pub size: u64,
    /// SHA1。
    pub sha1: String,
    /// Modrinth 来源；不是 Modrinth 上的文件或反查失败时为 `null`。
    pub source: Option<ResourceSource>,
}

/// 已安装资源列表。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct InstalledList {
    /// 按文件名排序的资源。
    pub resources: Vec<InstalledResource>,
    /// 反查来源失败（如断网）时的错误；此时 `source` 全为 `null`，列表本身仍可用。
    pub lookup_error: Option<AppError>,
}

/// 一个可更新的资源。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ResourceUpdate {
    /// 当前文件名。
    pub file_name: String,
    /// 项目 ID。
    pub project_id: String,
    /// 项目标题。
    pub title: String,
    /// 当前版本 ID。
    pub current_version_id: String,
    /// 当前版本号。
    pub current_version_number: String,
    /// 可更新到的版本。
    pub latest: ResourceVersion,
}

/// 导入时跳过文件的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ResourceImportSkipReason {
    /// 目录中已有同名文件（含其 `.disabled` 形式）。
    Exists,
    /// 不是 `.jar` / `.zip` 文件。
    Unsupported,
    /// 文件不存在或不是普通文件。
    NotFound,
}

/// 导入时被跳过的一个文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ResourceImportSkip {
    /// 选中的源文件路径。
    pub path: String,
    /// 源文件名。
    pub file_name: String,
    /// 跳过的原因。
    pub reason: ResourceImportSkipReason,
}

/// 导入的结果。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct ResourceImportResult {
    /// 已复制进目录的文件名，按选择顺序。
    pub imported: Vec<String>,
    /// 被跳过的文件。
    pub skipped: Vec<ResourceImportSkip>,
}

/// 实例中某一类资源的管理器。
#[derive(Debug, Clone)]
pub struct ResourceManager {
    client: ModrinthClient,
    target: InstanceTarget,
    kind: ResourceKind,
    dir: PathBuf,
}

impl ResourceManager {
    /// `game_dir` 为实例的 `game/`；数据包须给出存档名，存档不存在时返回 `RESOURCES_WORLD_NOT_FOUND`。
    pub fn new(
        client: ModrinthClient,
        target: InstanceTarget,
        game_dir: &Path,
        kind: ResourceKind,
        world: Option<&str>,
    ) -> AppResult<Self> {
        Ok(Self {
            dir: kind_dir(game_dir, kind, world)?,
            client,
            target,
            kind,
        })
    }

    /// 资源所在目录。
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 列出已安装的资源，并按 SHA1 反查来源；反查失败（取消除外）不影响列表。
    pub async fn list(&self, ctx: &TaskCtx) -> AppResult<InstalledList> {
        let files = local_files(&self.dir, ctx).await?;
        let (sources, lookup_error) = match self.sources(&files, ctx).await {
            Ok(sources) => (sources, None),
            Err(err) if err.is_cancelled() => return Err(err),
            Err(err) => (HashMap::new(), Some(err)),
        };
        let resources = files
            .into_iter()
            .map(|f| {
                let file_name = file_name(&f.path);
                InstalledResource {
                    enabled: !file_name.ends_with(DISABLED_SUFFIX),
                    file_name,
                    size: f.size,
                    source: sources.get(&f.sha1).cloned(),
                    sha1: f.sha1,
                }
            })
            .collect();
        Ok(InstalledList {
            resources,
            lookup_error,
        })
    }

    async fn sources(
        &self,
        files: &[LocalFile],
        ctx: &TaskCtx,
    ) -> AppResult<HashMap<String, ResourceSource>> {
        let hashes: Vec<String> = files.iter().map(|f| f.sha1.clone()).collect();
        let versions = self
            .client
            .versions_from_hashes(&hashes, HashAlgorithm::Sha1, ctx)
            .await?;
        let ids: HashSet<String> = versions.values().map(|v| v.project_id.clone()).collect();
        let ids: Vec<String> = ids.into_iter().collect();
        let projects: HashMap<String, _> = self
            .client
            .projects(&ids, ctx)
            .await?
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();
        Ok(versions
            .into_iter()
            .map(|(hash, v)| {
                let project = projects.get(&v.project_id);
                let source = ResourceSource {
                    title: project.map_or_else(|| v.project_id.clone(), |p| p.title.clone()),
                    icon_url: project.and_then(|p| p.icon_url.clone()),
                    project_id: v.project_id,
                    version_id: v.id,
                    version_number: v.version_number,
                };
                (hash.to_ascii_lowercase(), source)
            })
            .collect())
    }

    /// 把本地文件复制进资源目录（目录不存在时创建），保留原文件名；源文件不动。
    /// 不是 `.jar` / `.zip`、不存在或目录中已有同名文件（含 `.disabled` 形式）的文件被跳过，
    /// 不覆盖已有文件。带 `.disabled` 的源文件导入后为启用状态。先写临时文件再改名，复制失败不会留下半个文件。
    pub fn import(&self, paths: &[PathBuf]) -> AppResult<ResourceImportResult> {
        let mut result = ResourceImportResult::default();
        for path in paths {
            let name = file_name(path);
            let base = name
                .strip_suffix(DISABLED_SUFFIX)
                .unwrap_or(&name)
                .to_owned();
            let reason = if !path.is_file() {
                Some(ResourceImportSkipReason::NotFound)
            } else if !is_resource_file(path) {
                Some(ResourceImportSkipReason::Unsupported)
            } else if self.dir.join(&base).exists()
                || self.dir.join(format!("{base}{DISABLED_SUFFIX}")).exists()
                || result.imported.contains(&base)
            {
                Some(ResourceImportSkipReason::Exists)
            } else {
                None
            };
            if let Some(reason) = reason {
                result.skipped.push(ResourceImportSkip {
                    path: path.display().to_string(),
                    file_name: name,
                    reason,
                });
                continue;
            }
            std::fs::create_dir_all(&self.dir).map_err(|e| io_error(&self.dir, &e))?;
            let target = self.dir.join(&base);
            let part = self.dir.join(format!("{base}.part"));
            let copied = std::fs::copy(path, &part)
                .and_then(|_| std::fs::rename(&part, &target))
                .map_err(|e| io_error(path, &e));
            if let Err(err) = copied {
                let _ = std::fs::remove_file(&part);
                return Err(err);
            }
            result.imported.push(base);
        }
        Ok(result)
    }

    /// 启用或禁用资源（增删 `.disabled` 后缀），返回新的文件名；已是目标状态时不做改动。
    /// 文件不存在返回 `RESOURCES_FILE_NOT_FOUND`。
    pub fn set_enabled(&self, file: &str, enabled: bool) -> AppResult<String> {
        let path = self.existing(file)?;
        let next = match (file.strip_suffix(DISABLED_SUFFIX), enabled) {
            (Some(base), true) => base.to_owned(),
            (None, false) => format!("{file}{DISABLED_SUFFIX}"),
            _ => return Ok(file.to_owned()),
        };
        let target = self.dir.join(&next);
        if target.exists() {
            return Err(io_error(&target, &std::io::ErrorKind::AlreadyExists.into()));
        }
        std::fs::rename(&path, &target).map_err(|e| io_error(&path, &e))?;
        Ok(next)
    }

    /// 删除资源文件。文件不存在返回 `RESOURCES_FILE_NOT_FOUND`。
    pub fn remove(&self, file: &str) -> AppResult<()> {
        let path = self.existing(file)?;
        std::fs::remove_file(&path).map_err(|e| io_error(&path, &e))
    }

    /// 目录中名为 `file` 的资源文件；名称须为单个路径段。
    fn existing(&self, file: &str) -> AppResult<PathBuf> {
        let mut parts = Path::new(file).components();
        let single = matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none();
        let path = self.dir.join(file);
        if !single || !is_resource_file(&path) || !path.is_file() {
            return Err(AppError::new(ErrorCode::ResourcesFileNotFound).with_param("file", file));
        }
        Ok(path)
    }

    /// 检查 Modrinth 上来源可查的资源是否有适用于实例的更新版本（发布时间晚于当前版本）。
    pub async fn check_updates(&self, ctx: &TaskCtx) -> AppResult<Vec<ResourceUpdate>> {
        Ok(self
            .find_updates(ctx)
            .await?
            .into_iter()
            .map(|(update, _)| update)
            .collect())
    }

    async fn find_updates(&self, ctx: &TaskCtx) -> AppResult<Vec<(ResourceUpdate, Version)>> {
        let files = local_files(&self.dir, ctx).await?;
        let hashes: Vec<String> = files.iter().map(|f| f.sha1.clone()).collect();
        let current = self
            .client
            .versions_from_hashes(&hashes, HashAlgorithm::Sha1, ctx)
            .await?;
        let latest = self
            .client
            .latest_versions_from_hashes(&hashes, HashAlgorithm::Sha1, &self.filter(), ctx)
            .await?;
        let lower = |m: HashMap<String, Version>| -> HashMap<String, Version> {
            m.into_iter()
                .map(|(h, v)| (h.to_ascii_lowercase(), v))
                .collect()
        };
        let (current, mut latest) = (lower(current), lower(latest));

        let mut updates = Vec::new();
        for file in &files {
            let (Some(now), Some(next)) = (current.get(&file.sha1), latest.remove(&file.sha1))
            else {
                continue;
            };
            if next.id == now.id || next.date_published <= now.date_published {
                continue;
            }
            let Some(view) = ResourceVersion::from_version(&next) else {
                continue;
            };
            updates.push((
                ResourceUpdate {
                    file_name: file_name(&file.path),
                    project_id: now.project_id.clone(),
                    title: now.project_id.clone(),
                    current_version_id: now.id.clone(),
                    current_version_number: now.version_number.clone(),
                    latest: view,
                },
                next,
            ));
        }
        let ids: Vec<String> = updates.iter().map(|(u, _)| u.project_id.clone()).collect();
        let titles: HashMap<String, String> = self
            .client
            .projects(&ids, ctx)
            .await?
            .into_iter()
            .map(|p| (p.id, p.title))
            .collect();
        for (update, _) in &mut updates {
            if let Some(title) = titles.get(&update.project_id) {
                update.title.clone_from(title);
            }
        }
        Ok(updates)
    }

    /// 更新 `files` 中列出的资源（`None` 为全部可更新的），返回实际执行的更新。
    ///
    /// 新文件下载并校验成功后才删除旧文件；被禁用的资源更新后仍为禁用。
    pub async fn update(
        &self,
        downloader: &Downloader,
        files: Option<&[String]>,
        ctx: &TaskCtx,
    ) -> AppResult<Vec<ResourceUpdate>> {
        let mut chosen = self.find_updates(ctx).await?;
        if let Some(files) = files {
            chosen.retain(|(u, _)| files.contains(&u.file_name));
        }
        let mut items = Vec::new();
        let mut old = Vec::new();
        for (update, version) in &chosen {
            let Some(file) = version.primary_file() else {
                continue;
            };
            let mut item = file.download_item(&self.dir);
            if update.file_name.ends_with(DISABLED_SUFFIX) {
                let mut name = item.path.clone().into_os_string();
                name.push(DISABLED_SUFFIX);
                item.path = name.into();
            }
            let current = self.dir.join(&update.file_name);
            if current != item.path {
                old.push(current);
            }
            items.push(item);
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| io_error(&self.dir, &e))?;
        downloader.download_all(items, INSTALL_STAGE, ctx).await?;
        for path in old {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_error(&path, &e)),
            }
        }
        Ok(chosen.into_iter().map(|(update, _)| update).collect())
    }

    /// 检查更新时的过滤条件。`POST /version_files/update` 按加载器匹配，因此资源包、光影
    /// 使用其固定的“加载器”名。
    fn filter(&self) -> VersionFilter {
        let mut filter = self.target.version_filter(self.kind);
        if filter.loaders.is_empty() {
            filter.loaders = match self.kind {
                ResourceKind::Resourcepack => vec!["minecraft".to_owned()],
                ResourceKind::Shader => ["iris", "optifine", "canvas", "vanilla"]
                    .map(ToOwned::to_owned)
                    .to_vec(),
                ResourceKind::Mod | ResourceKind::Datapack => Vec::new(),
            };
        }
        filter
    }
}

/// 实例 `saves/` 下的存档目录名（按名称排序），供选择数据包的安装目标。
pub fn worlds(game_dir: &Path) -> AppResult<Vec<String>> {
    let saves = game_dir.join("saves");
    let entries = match std::fs::read_dir(&saves) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_error(&saves, &e)),
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    Ok(names)
}
