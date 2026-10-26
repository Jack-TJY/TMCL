//! 资源安装：先生成计划（含前置、冲突与缺失的前置），确认后下载到实例对应目录。
//!
//! 安装目录（相对实例的 `game/`）：
//!
//! | 种类 | 目录 |
//! | --- | --- |
//! | mod | `mods/` |
//! | 资源包 | `resourcepacks/` |
//! | 光影 | `shaderpacks/` |
//! | 数据包 | `saves/<存档>/datapacks/`（数据包按存档生效，必须指定已存在的存档） |
//!
//! 同一目录中已装有同一项目的其他版本时，新文件下载并校验成功后删除旧文件。
//! 计划中有冲突或缺失的前置时拒绝安装，见 [`InstallPlan::check`]。

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::{DownloadItem, Downloader, sha1_file};
use ts_rs::TS;

use crate::browse::{InstanceTarget, ResourceKind};
use crate::deps::{self, Conflict, InstalledVersion, UnavailableDependency};
use crate::modrinth::{HashAlgorithm, ModrinthClient};

/// 下载阶段名。
pub const INSTALL_STAGE: &str = "resources";

/// 被禁用的资源文件的后缀。
pub const DISABLED_SUFFIX: &str = ".disabled";

/// 要安装的版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    /// Modrinth 版本 ID。
    pub version_id: String,
    /// 资源种类。
    pub kind: ResourceKind,
    /// 数据包安装到的存档（`saves/` 下的目录名），其他种类忽略。
    pub world: Option<String>,
}

/// 计划中的一个文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PlannedFile {
    /// 项目 ID。
    pub project_id: String,
    /// 版本 ID。
    pub version_id: String,
    /// 项目标题。
    pub title: String,
    /// 版本号。
    pub version_number: String,
    /// 资源种类，决定安装目录。
    pub kind: ResourceKind,
    /// 文件名。
    pub file_name: String,
    /// 文件大小（字节）。
    #[ts(type = "number")]
    pub size: u64,
    /// 因哪个项目的必需前置而安装；用户选择的版本为 `null`。
    pub required_by: Option<String>,
    /// 安装后要删除的同一项目旧文件名（同一目录中）。
    pub replaces: Option<String>,
}

/// 安装计划。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct InstallPlan {
    /// 要下载的文件：第一个是用户选择的版本，其余为前置。
    pub files: Vec<PlannedFile>,
    /// 与本次安装不兼容的项目，非空时不能安装。
    pub conflicts: Vec<Conflict>,
    /// 找不到可用版本的必需前置，非空时不能安装。
    pub unavailable: Vec<UnavailableDependency>,
}

impl InstallPlan {
    /// 有冲突返回 `RESOURCES_INCOMPATIBLE`，有缺失的前置返回 `RESOURCES_DEPENDENCY_UNAVAILABLE`。
    pub fn check(&self) -> AppResult<()> {
        if let Some(c) = self.conflicts.first() {
            return Err(AppError::new(ErrorCode::ResourcesIncompatible)
                .with_param("project", &c.title)
                .with_param("with", &c.with_title));
        }
        if let Some(u) = self.unavailable.first() {
            let name = u.title.as_ref().or(u.project_id.as_ref());
            return Err(AppError::new(ErrorCode::ResourcesDependencyUnavailable)
                .with_param("project", name.map_or("?", String::as_str)));
        }
        Ok(())
    }
}

/// 某种资源在实例中的安装目录；数据包的存档名非法或存档不存在时返回 `RESOURCES_WORLD_NOT_FOUND`。
pub fn kind_dir(game_dir: &Path, kind: ResourceKind, world: Option<&str>) -> AppResult<PathBuf> {
    Ok(match kind {
        ResourceKind::Mod => game_dir.join("mods"),
        ResourceKind::Resourcepack => game_dir.join("resourcepacks"),
        ResourceKind::Shader => game_dir.join("shaderpacks"),
        ResourceKind::Datapack => {
            let world = world.unwrap_or_default();
            let mut parts = Path::new(world).components();
            let single =
                matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none();
            let dir = game_dir.join("saves").join(world);
            if !single || world.contains(':') || !dir.is_dir() {
                return Err(
                    AppError::new(ErrorCode::ResourcesWorldNotFound).with_param("world", world)
                );
            }
            dir.join("datapacks")
        }
    })
}

/// 目录中的一个资源文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    /// 文件路径。
    pub path: PathBuf,
    /// SHA1（小写十六进制）。
    pub sha1: String,
    /// 文件大小（字节）。
    pub size: u64,
}

/// 列出目录中的资源文件（见 [`is_resource_file`]）并计算 SHA1，按文件名排序；不存在的目录视为空。
pub async fn local_files(dir: &Path, ctx: &TaskCtx) -> AppResult<Vec<LocalFile>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_error(dir, &e)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| io_error(dir, &e))?.path();
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.is_file() && is_resource_file(&path) {
            ctx.check_cancelled()?;
            let sha1 = sha1_file(&path).await?;
            files.push(LocalFile {
                path,
                sha1,
                size: meta.len(),
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// 扫描目录中的资源文件，按 SHA1 在 Modrinth 反查版本；找不到来源的文件不出现在结果中。
pub async fn scan_installed(
    client: &ModrinthClient,
    dirs: &[PathBuf],
    ctx: &TaskCtx,
) -> AppResult<Vec<InstalledVersion>> {
    let mut by_hash: HashMap<String, PathBuf> = HashMap::new();
    for dir in dirs {
        for file in local_files(dir, ctx).await? {
            by_hash.insert(file.sha1, file.path);
        }
    }
    let hashes: Vec<String> = by_hash.keys().cloned().collect();
    let found = client
        .versions_from_hashes(&hashes, HashAlgorithm::Sha1, ctx)
        .await?;
    let mut installed: Vec<InstalledVersion> = found
        .into_iter()
        .filter_map(|(hash, version)| {
            let path = by_hash.remove(&hash.to_ascii_lowercase())?;
            Some(InstalledVersion { path, version })
        })
        .collect();
    installed.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(installed)
}

/// 是否为资源文件：`.jar` / `.zip`，可带 `.disabled` 后缀。
pub fn is_resource_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let name = name.strip_suffix(DISABLED_SUFFIX).unwrap_or(&name);
    name.ends_with(".jar") || name.ends_with(".zip")
}

/// 在一个实例中安装资源。
#[derive(Debug, Clone)]
pub struct ResourceInstaller {
    client: ModrinthClient,
    downloader: Downloader,
    game_dir: PathBuf,
    target: InstanceTarget,
}

/// 计划及其对应的下载项与要删除的旧文件。
struct Prepared {
    plan: InstallPlan,
    downloads: Vec<DownloadItem>,
    replaced: Vec<PathBuf>,
}

impl ResourceInstaller {
    /// `game_dir` 为实例的 `game/` 目录。
    pub fn new(
        client: ModrinthClient,
        downloader: Downloader,
        game_dir: PathBuf,
        target: InstanceTarget,
    ) -> Self {
        Self {
            client,
            downloader,
            game_dir,
            target,
        }
    }

    /// 生成安装计划，不写任何文件。版本不存在或没有文件时返回 `RESOURCES_VERSION_NOT_FOUND`。
    pub async fn plan(&self, request: &InstallRequest, ctx: &TaskCtx) -> AppResult<InstallPlan> {
        Ok(self.prepare(request, ctx).await?.plan)
    }

    /// 重新生成计划并检查（[`InstallPlan::check`]），然后下载全部文件、删除被替换的旧文件。
    /// 返回实际执行的计划。
    pub async fn install(&self, request: &InstallRequest, ctx: &TaskCtx) -> AppResult<InstallPlan> {
        let prepared = self.prepare(request, ctx).await?;
        prepared.plan.check()?;
        for item in &prepared.downloads {
            if let Some(dir) = item.path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
            }
        }
        self.downloader
            .download_all(prepared.downloads, INSTALL_STAGE, ctx)
            .await?;
        for old in &prepared.replaced {
            match std::fs::remove_file(old) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_error(old, &e)),
            }
        }
        Ok(prepared.plan)
    }

    async fn prepare(&self, request: &InstallRequest, ctx: &TaskCtx) -> AppResult<Prepared> {
        let world = request.world.as_deref();
        let root_dir = kind_dir(&self.game_dir, request.kind, world)?;
        let root = self
            .client
            .version(&request.version_id, ctx)
            .await?
            .filter(|v| v.primary_file().is_some())
            .ok_or_else(|| {
                AppError::new(ErrorCode::ResourcesVersionNotFound)
                    .with_param("version", &request.version_id)
            })?;

        // 前置通常是模组，因此总是同时检查 mods/。
        let mut dirs = vec![kind_dir(&self.game_dir, ResourceKind::Mod, None)?];
        if !dirs.contains(&root_dir) {
            dirs.push(root_dir);
        }
        let installed = scan_installed(&self.client, &dirs, ctx).await?;
        let resolution = deps::resolve(
            &self.client,
            &self.target,
            root,
            request.kind,
            &installed,
            ctx,
        )
        .await?;

        let mut prepared = Prepared {
            plan: InstallPlan {
                files: Vec::new(),
                conflicts: resolution.conflicts,
                unavailable: resolution.unavailable,
            },
            downloads: Vec::new(),
            replaced: Vec::new(),
        };
        for resolved in resolution.resolved {
            let version = &resolved.version;
            let Some(file) = version.primary_file() else {
                prepared.plan.unavailable.push(UnavailableDependency {
                    project_id: Some(version.project_id.clone()),
                    title: Some(resolved.title),
                    required_by: resolved.required_by.unwrap_or_default(),
                });
                continue;
            };
            let dir = kind_dir(&self.game_dir, resolved.kind, world)?;
            let item = file.download_item(&dir);
            let old = installed.iter().find(|i| {
                i.version.project_id == version.project_id
                    && i.path.parent() == Some(dir.as_path())
                    && i.path != item.path
            });
            if let Some(old) = old {
                prepared.replaced.push(old.path.clone());
            }
            prepared.plan.files.push(PlannedFile {
                project_id: version.project_id.clone(),
                version_id: version.id.clone(),
                title: resolved.title,
                version_number: version.version_number.clone(),
                kind: resolved.kind,
                file_name: file_name(&item.path),
                size: file.size,
                required_by: resolved.required_by,
                replaces: old.map(|o| file_name(&o.path)),
            });
            prepared.downloads.push(item);
        }
        Ok(prepared)
    }
}

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(crate) fn io_error(path: &Path, err: &std::io::Error) -> AppError {
    AppError::new(ErrorCode::CommonIo)
        .with_param("path", path.display())
        .with_detail(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path();
        assert_eq!(
            kind_dir(game, ResourceKind::Mod, None).unwrap(),
            game.join("mods")
        );
        assert_eq!(
            kind_dir(game, ResourceKind::Shader, Some("x")).unwrap(),
            game.join("shaderpacks")
        );
        std::fs::create_dir_all(game.join("saves/My World")).unwrap();
        assert_eq!(
            kind_dir(game, ResourceKind::Datapack, Some("My World")).unwrap(),
            game.join("saves/My World/datapacks")
        );
        for bad in [
            None,
            Some(""),
            Some("missing"),
            Some(".."),
            Some("a/b"),
            Some("C:x"),
        ] {
            let err = kind_dir(game, ResourceKind::Datapack, bad).unwrap_err();
            assert_eq!(err.code, ErrorCode::ResourcesWorldNotFound, "{bad:?}");
        }
    }

    #[test]
    fn resource_files() {
        for ok in [
            "a.jar",
            "B.JAR",
            "pack.zip",
            "a.jar.disabled",
            "p.zip.disabled",
        ] {
            assert!(is_resource_file(Path::new(ok)), "{ok}");
        }
        for bad in ["a.txt", "a.disabled", "options.txt", "a.jar.bak"] {
            assert!(!is_resource_file(Path::new(bad)), "{bad}");
        }
    }

    #[test]
    fn check_reports_conflicts_before_missing_dependencies() {
        let mut plan = InstallPlan {
            files: Vec::new(),
            conflicts: Vec::new(),
            unavailable: vec![UnavailableDependency {
                project_id: Some("P".into()),
                title: None,
                required_by: "A".into(),
            }],
        };
        let err = plan.check().unwrap_err();
        assert_eq!(err.code, ErrorCode::ResourcesDependencyUnavailable);
        assert_eq!(err.params["project"], "P");
        plan.conflicts.push(Conflict {
            project_id: "A".into(),
            title: "Mod A".into(),
            with_project_id: "B".into(),
            with_title: "Mod B".into(),
            with_installed: true,
        });
        let err = plan.check().unwrap_err();
        assert_eq!(err.code, ErrorCode::ResourcesIncompatible);
        assert_eq!(err.params["with"], "Mod B");
        plan.conflicts.clear();
        plan.unavailable.clear();
        assert!(plan.check().is_ok());
    }
}
