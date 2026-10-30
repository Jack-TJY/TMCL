//! 从 CurseForge 安装资源：解析必需前置并检查不兼容，生成与 Modrinth 相同的安装计划。
//!
//! - 已安装的项目按 CurseForge 指纹反查（`mods/` 与目标目录），按项目判断，不论版本。
//! - 必需前置（`relationType` = 3）在依赖项目中按实例过滤选文件：最新的正式版，没有时取最新。
//!   找不到可用文件的前置记入 [`InstallPlan::unavailable`]。前置一律按 mod 安装。
//! - 不兼容（`relationType` = 5）覆盖“计划 × 计划”与“计划 × 已安装”。
//! - 作者禁止第三方分发的文件与整合包同样处理：先找 Modrinth 上 SHA1 相同的文件，再走
//!   MCIM；都不行（没有 SHA1）时记为找不到可用文件。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::DownloadItem;

use crate::browse::{InstanceTarget, ResourceKind};
use crate::curseforge::{
    CurseForgeClient, File, FileFilter, RelationType, ReleaseType, fingerprint,
};
use crate::deps::{Conflict, UnavailableDependency};
use crate::install::{
    InstallPlan, InstallRequest, PlannedFile, Prepared, file_name, kind_dir, local_files,
};
use crate::merged::curseforge_loader;
use crate::modpack::curseforge::{MIRROR_BASE, mirror_url};
use crate::modrinth::{HashAlgorithm, ModrinthClient};

/// 一次安装最多解析的文件数，防止依赖链失控。
const MAX_FILES: usize = 64;

/// 计划中的一个 CurseForge 文件。
struct Planned {
    file: File,
    kind: ResourceKind,
    title: String,
    required_by: Option<String>,
}

/// 生成 CurseForge 版本（文件 ID）的安装计划。文件不存在返回 `RESOURCES_VERSION_NOT_FOUND`。
pub(crate) async fn prepare(
    client: &CurseForgeClient,
    modrinth: &ModrinthClient,
    target: &InstanceTarget,
    game_dir: &Path,
    request: &InstallRequest,
    ctx: &TaskCtx,
) -> AppResult<Prepared> {
    let world = request.world.as_deref();
    let root_dir = kind_dir(game_dir, request.kind, world)?;
    let not_found = || {
        AppError::new(ErrorCode::ResourcesVersionNotFound)
            .with_param("version", &request.version_id)
    };
    let file_id: u32 = request.version_id.parse().map_err(|_| not_found())?;
    let root = client
        .files(&[file_id], ctx)
        .await?
        .into_iter()
        .find(|f| f.id == file_id && f.is_available)
        .ok_or_else(not_found)?;

    let mut dirs = vec![kind_dir(game_dir, ResourceKind::Mod, None)?];
    if !dirs.contains(&root_dir) {
        dirs.push(root_dir);
    }
    let installed = installed_projects(client, &dirs, ctx).await?;

    let mut titles = HashMap::new();
    let mut plan = InstallPlan {
        files: Vec::new(),
        conflicts: Vec::new(),
        unavailable: Vec::new(),
    };
    let mut planned: Vec<Planned> = Vec::new();
    let mut queue = VecDeque::from([(root, request.kind, None::<String>)]);
    let mut seen = HashSet::new();
    while let Some((file, kind, required_by)) = queue.pop_front() {
        if !seen.insert(file.mod_id) || planned.len() >= MAX_FILES {
            continue;
        }
        let deps: Vec<u32> = file.dependencies.iter().map(|d| d.mod_id).collect();
        load_titles(client, &mut titles, file.mod_id, &deps, ctx).await?;
        let title = title_of(&titles, file.mod_id);
        for dep in &file.dependencies {
            match dep.relation_type {
                RelationType::RequiredDependency => {
                    if installed.contains_key(&dep.mod_id) || seen.contains(&dep.mod_id) {
                        continue;
                    }
                    match pick_file(client, target, dep.mod_id, ctx).await? {
                        Some(f) => {
                            queue.push_back((f, ResourceKind::Mod, Some(file.mod_id.to_string())))
                        }
                        None => plan.unavailable.push(UnavailableDependency {
                            project_id: Some(dep.mod_id.to_string()),
                            title: titles.get(&dep.mod_id).cloned(),
                            required_by: file.mod_id.to_string(),
                        }),
                    }
                }
                RelationType::Incompatible => {
                    let with_installed = installed.contains_key(&dep.mod_id);
                    if with_installed || seen.contains(&dep.mod_id) {
                        plan.conflicts.push(Conflict {
                            project_id: file.mod_id.to_string(),
                            title: title.clone(),
                            with_project_id: dep.mod_id.to_string(),
                            with_title: title_of(&titles, dep.mod_id),
                            with_installed,
                        });
                    }
                }
                _ => {}
            }
        }
        planned.push(Planned {
            file,
            kind,
            title,
            required_by,
        });
    }

    let mut prepared = Prepared {
        plan,
        downloads: Vec::new(),
        replaced: Vec::new(),
    };
    let blocked: Vec<String> = planned
        .iter()
        .filter(|p| p.file.download_item(Path::new("")).is_none())
        .filter_map(|p| p.file.sha1())
        .collect();
    let on_modrinth = modrinth_urls(modrinth, &blocked, ctx).await?;
    for p in planned {
        let dir = kind_dir(game_dir, p.kind, world)?;
        let Some(item) = download_item(&p.file, &dir, &on_modrinth) else {
            prepared.plan.unavailable.push(UnavailableDependency {
                project_id: Some(p.file.mod_id.to_string()),
                title: Some(p.title),
                required_by: p.required_by.unwrap_or_default(),
            });
            continue;
        };
        let old = installed
            .get(&p.file.mod_id)
            .and_then(|paths| {
                paths
                    .iter()
                    .find(|o| o.parent() == Some(dir.as_path()) && **o != item.path)
            })
            .cloned();
        prepared.plan.files.push(PlannedFile {
            project_id: p.file.mod_id.to_string(),
            version_id: p.file.id.to_string(),
            title: p.title,
            version_number: p.file.display_name.clone(),
            kind: p.kind,
            file_name: file_name(&item.path),
            size: p.file.file_length,
            required_by: p.required_by,
            replaces: old.as_deref().map(file_name),
        });
        if let Some(old) = old {
            prepared.replaced.push(old);
        }
        prepared.downloads.push(item);
    }
    Ok(prepared)
}

/// 下载项：能直接下载的用官方地址；否则用 Modrinth 上的同一文件，再否则用 MCIM（需要 SHA1）。
fn download_item(
    file: &File,
    dir: &Path,
    on_modrinth: &HashMap<String, String>,
) -> Option<DownloadItem> {
    if let Some(item) = file.download_item(dir) {
        return Some(item);
    }
    let sha1 = file.sha1()?;
    let url = match on_modrinth.get(&sha1) {
        Some(url) => url.clone(),
        None => mirror_url(MIRROR_BASE, file.id, &file.file_name)?,
    };
    // 借用文件名校验：伪造一个可下载的副本。
    let probe = File {
        download_url: Some(url),
        ..file.clone()
    };
    probe.download_item(dir)
}

/// 按 SHA1 在 Modrinth 查找禁止分发的文件；查询失败不影响安装。
async fn modrinth_urls(
    modrinth: &ModrinthClient,
    hashes: &[String],
    ctx: &TaskCtx,
) -> AppResult<HashMap<String, String>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    match modrinth
        .versions_from_hashes(hashes, HashAlgorithm::Sha1, ctx)
        .await
    {
        Ok(found) => Ok(found
            .into_iter()
            .filter_map(|(hash, version)| {
                let hash = hash.to_ascii_lowercase();
                let file = version
                    .files
                    .into_iter()
                    .find(|f| f.hashes.sha1.eq_ignore_ascii_case(&hash))?;
                Some((hash, file.url))
            })
            .collect()),
        Err(err) if err.is_cancelled() => Err(err),
        Err(err) => {
            tracing::warn!(%err, "在 Modrinth 按哈希查找禁止分发的文件失败");
            Ok(HashMap::new())
        }
    }
}

/// 依赖项目中适用于实例的文件：最新的正式版，没有时取最新的可用文件。
async fn pick_file(
    client: &CurseForgeClient,
    target: &InstanceTarget,
    mod_id: u32,
    ctx: &TaskCtx,
) -> AppResult<Option<File>> {
    let filter = FileFilter {
        game_version: Some(target.game_version.clone()),
        loader: curseforge_loader(target, ResourceKind::Mod),
        limit: 50,
        ..FileFilter::default()
    };
    let files = match client.project_files(mod_id, &filter, ctx).await {
        Ok(page) => page.data,
        Err(err) if crate::browse::is_not_found(&err) => return Ok(None),
        Err(err) => return Err(err),
    };
    let mut available = files.into_iter().filter(|f| f.is_available);
    let first = available.next();
    match first {
        Some(f) if f.release_type == ReleaseType::Release => Ok(Some(f)),
        Some(f) => Ok(Some(
            available
                .find(|f| f.release_type == ReleaseType::Release)
                .unwrap_or(f),
        )),
        None => Ok(None),
    }
}

/// 补齐 `ids` 中还没有标题的项目。
async fn load_titles(
    client: &CurseForgeClient,
    titles: &mut HashMap<u32, String>,
    id: u32,
    deps: &[u32],
    ctx: &TaskCtx,
) -> AppResult<()> {
    let missing: Vec<u32> = std::iter::once(id)
        .chain(deps.iter().copied())
        .filter(|id| !titles.contains_key(id))
        .collect();
    for project in client.projects(&missing, ctx).await? {
        titles.insert(project.id, project.name);
    }
    Ok(())
}

fn title_of(titles: &HashMap<u32, String>, id: u32) -> String {
    titles.get(&id).cloned().unwrap_or_else(|| id.to_string())
}

/// 目录中能在 CurseForge 按指纹找到的文件：项目 ID → 文件路径。
async fn installed_projects(
    client: &CurseForgeClient,
    dirs: &[PathBuf],
    ctx: &TaskCtx,
) -> AppResult<HashMap<u32, Vec<PathBuf>>> {
    let mut paths = Vec::new();
    for dir in dirs {
        paths.extend(local_files(dir, ctx).await?.into_iter().map(|f| f.path));
    }
    let hashed = tokio::task::spawn_blocking(move || {
        paths
            .into_iter()
            .filter_map(|p| Some((fingerprint::of_file(&p).ok()?, p)))
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?;
    let fingerprints: Vec<u32> = hashed.iter().map(|(f, _)| *f).collect();
    let matches = client.match_fingerprints(&fingerprints, ctx).await?;
    let mut by_project: HashMap<u32, Vec<PathBuf>> = HashMap::new();
    for m in matches.exact_matches {
        for (fp, path) in &hashed {
            if *fp == m.file.file_fingerprint {
                by_project.entry(m.id).or_default().push(path.clone());
            }
        }
    }
    Ok(by_project)
}
