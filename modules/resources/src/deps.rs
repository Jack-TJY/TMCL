//! 依赖解析：递归找出必需前置（`required`）的版本，并在安装前找出不兼容（`incompatible`）的组合。
//!
//! - 已安装（按项目判断，不论版本）或已在计划中的前置不再重复安装。
//! - 依赖指定了版本 ID 时用该版本；否则在依赖项目中按实例过滤选版本：最新的正式版，没有正式版时
//!   取最新版本。找不到可用版本的前置记入 [`Resolution::unavailable`]。
//! - 只有外部文件名（不在 Modrinth 上）的依赖无法自动安装，忽略。
//! - 不兼容检查覆盖“计划 × 计划”与“计划 × 已安装”；已安装资源之间原有的冲突不报告。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tmcl_common::{AppResult, TaskCtx};
use ts_rs::TS;

use crate::browse::{InstanceTarget, ResourceKind};
use crate::modrinth::{
    Dependency, DependencyType, ModrinthClient, Project, ProjectType, Version, VersionType,
};

/// 实例中已安装、并能在 Modrinth 上按哈希找到的资源。
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledVersion {
    /// 文件路径（文件名可能带 `.disabled` 后缀）。
    pub path: PathBuf,
    /// 文件对应的版本。
    pub version: Version,
}

/// 计划安装的一个版本。
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    /// 版本。
    pub version: Version,
    /// 资源种类，决定安装目录。
    pub kind: ResourceKind,
    /// 项目标题；取不到时为项目 ID。
    pub title: String,
    /// 因哪个项目的必需前置而安装；用户选择的版本为 `None`。
    pub required_by: Option<String>,
}

/// 一对不兼容的项目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Conflict {
    /// 计划安装的项目 ID。
    pub project_id: String,
    /// 计划安装的项目标题。
    pub title: String,
    /// 与之不兼容的项目 ID。
    pub with_project_id: String,
    /// 与之不兼容的项目标题。
    pub with_title: String,
    /// 对方是否为已安装的资源（否则也在本次计划中）。
    pub with_installed: bool,
}

/// 找不到可用版本的必需前置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UnavailableDependency {
    /// 前置项目 ID；依赖只给出了不存在的版本 ID 时为 `None`。
    pub project_id: Option<String>,
    /// 前置项目标题；项目不存在时为 `None`。
    pub title: Option<String>,
    /// 需要它的项目 ID。
    pub required_by: String,
}

/// 依赖解析结果。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Resolution {
    /// 要安装的版本：第一个是用户选择的版本，其余为前置，按发现顺序。
    pub resolved: Vec<Resolved>,
    /// 不兼容的组合。
    pub conflicts: Vec<Conflict>,
    /// 找不到可用版本的前置。
    pub unavailable: Vec<UnavailableDependency>,
}

/// 从 `root` 出发递归解析必需前置，并检查不兼容。
pub async fn resolve(
    client: &ModrinthClient,
    target: &InstanceTarget,
    root: Version,
    root_kind: ResourceKind,
    installed: &[InstalledVersion],
    ctx: &TaskCtx,
) -> AppResult<Resolution> {
    let mut projects = ProjectCache::default();
    projects
        .load(client, std::slice::from_ref(&root.project_id), ctx)
        .await?;
    let mut seen: HashSet<String> = installed
        .iter()
        .map(|i| i.version.project_id.clone())
        .collect();
    seen.insert(root.project_id.clone());

    let mut out = Resolution::default();
    out.resolved.push(Resolved {
        title: projects.title(&root.project_id),
        version: root,
        kind: root_kind,
        required_by: None,
    });
    let mut frontier = vec![0];
    while !frontier.is_empty() {
        ctx.check_cancelled()?;
        let wanted: Vec<(Dependency, String)> = frontier
            .drain(..)
            .flat_map(|i: usize| {
                let parent = &out.resolved[i].version;
                parent
                    .required_dependencies()
                    .map(|d| (d.clone(), parent.project_id.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();

        // 指定了版本的依赖先批量取版本，以得到其项目 ID。
        let pinned_ids: Vec<String> = wanted
            .iter()
            .filter_map(|(d, _)| d.version_id.clone())
            .collect();
        let pinned: HashMap<String, Version> = client
            .versions(&pinned_ids, ctx)
            .await?
            .into_iter()
            .map(|v| (v.id.clone(), v))
            .collect();

        let mut fresh = Vec::new();
        for (dep, required_by) in wanted {
            // 指定的版本不存在时退回按项目选版本。
            let version = dep
                .version_id
                .as_ref()
                .and_then(|id| pinned.get(id))
                .cloned();
            let project_id = match (&version, &dep.project_id) {
                (Some(v), _) => v.project_id.clone(),
                (None, Some(id)) => id.clone(),
                (None, None) => {
                    if dep.version_id.is_some() {
                        out.unavailable.push(UnavailableDependency {
                            project_id: None,
                            title: None,
                            required_by,
                        });
                    }
                    continue;
                }
            };
            if seen.insert(project_id.clone()) {
                fresh.push((project_id, version, required_by));
            }
        }
        let ids: Vec<String> = fresh.iter().map(|(id, _, _)| id.clone()).collect();
        projects.load(client, &ids, ctx).await?;

        for (project_id, version, required_by) in fresh {
            let Some(project) = projects.get(&project_id) else {
                out.unavailable.push(UnavailableDependency {
                    project_id: Some(project_id),
                    title: None,
                    required_by,
                });
                continue;
            };
            let kind = dependency_kind(project, root_kind);
            let title = project.title.clone();
            let version = match version {
                Some(v) => Some(v),
                None => pick(client, target, kind, &project_id, ctx).await?,
            };
            match version {
                Some(version) => {
                    out.resolved.push(Resolved {
                        version,
                        kind,
                        title,
                        required_by: Some(required_by),
                    });
                    frontier.push(out.resolved.len() - 1);
                }
                None => out.unavailable.push(UnavailableDependency {
                    project_id: Some(project_id),
                    title: Some(title),
                    required_by,
                }),
            }
        }
    }

    out.conflicts = find_conflicts(client, &out.resolved, installed, &mut projects, ctx).await?;
    Ok(out)
}

/// 前置的安装种类：资源包、光影按项目类型；数据包的前置若提供数据包文件也作为数据包；其余为模组。
fn dependency_kind(project: &Project, root_kind: ResourceKind) -> ResourceKind {
    match project.project_type {
        ProjectType::Resourcepack => ResourceKind::Resourcepack,
        ProjectType::Shader => ResourceKind::Shader,
        ProjectType::Datapack => ResourceKind::Datapack,
        _ if root_kind == ResourceKind::Datapack
            && project.loaders.iter().any(|l| l == "datapack") =>
        {
            ResourceKind::Datapack
        }
        _ => ResourceKind::Mod,
    }
}

/// 选出项目中适用于实例的版本：最新的正式版，没有时取最新版本；都没有文件时为 `None`。
pub async fn pick(
    client: &ModrinthClient,
    target: &InstanceTarget,
    kind: ResourceKind,
    project_id: &str,
    ctx: &TaskCtx,
) -> AppResult<Option<Version>> {
    let versions = match client
        .project_versions(project_id, &target.version_filter(kind), ctx)
        .await
    {
        Ok(versions) => versions,
        Err(err) if crate::browse::is_not_found(&err) => return Ok(None),
        Err(err) => return Err(err),
    };
    let mut usable = versions.into_iter().filter(|v| v.primary_file().is_some());
    let first = usable.next();
    if first
        .as_ref()
        .is_some_and(|v| v.version_type == VersionType::Release)
    {
        return Ok(first);
    }
    Ok(usable
        .find(|v| v.version_type == VersionType::Release)
        .or(first))
}

/// 参与不兼容检查的一个版本。
struct Node<'a> {
    project_id: &'a str,
    version_id: &'a str,
    dependencies: &'a [Dependency],
    installed: bool,
}

async fn find_conflicts(
    client: &ModrinthClient,
    resolved: &[Resolved],
    installed: &[InstalledVersion],
    projects: &mut ProjectCache,
    ctx: &TaskCtx,
) -> AppResult<Vec<Conflict>> {
    let planned: HashSet<&str> = resolved
        .iter()
        .map(|r| r.version.project_id.as_str())
        .collect();
    let nodes: Vec<Node<'_>> = resolved
        .iter()
        .map(|r| (&r.version, false))
        // 将被替换的已安装版本（同一项目）不参与检查。
        .chain(
            installed
                .iter()
                .filter(|i| !planned.contains(i.version.project_id.as_str()))
                .map(|i| (&i.version, true)),
        )
        .map(|(v, installed)| Node {
            project_id: &v.project_id,
            version_id: &v.id,
            dependencies: &v.dependencies,
            installed,
        })
        .collect();

    let mut pairs: Vec<(&Node<'_>, &Node<'_>)> = Vec::new();
    for a in &nodes {
        for dep in a
            .dependencies
            .iter()
            .filter(|d| d.dependency_type == DependencyType::Incompatible)
        {
            for b in &nodes {
                let hit = dep.project_id.as_deref() == Some(b.project_id)
                    || dep.version_id.as_deref() == Some(b.version_id);
                if !hit || a.project_id == b.project_id || (a.installed && b.installed) {
                    continue;
                }
                // 计划中的一方放在前面；同一对只报告一次。
                let pair = if a.installed { (b, a) } else { (a, b) };
                let duplicate = pairs.iter().any(|(x, y)| {
                    (x.project_id, y.project_id) == (pair.0.project_id, pair.1.project_id)
                        || (x.project_id, y.project_id) == (pair.1.project_id, pair.0.project_id)
                });
                if !duplicate {
                    pairs.push(pair);
                }
            }
        }
    }

    let ids: Vec<String> = pairs
        .iter()
        .flat_map(|(a, b)| [a.project_id.to_owned(), b.project_id.to_owned()])
        .collect();
    projects.load(client, &ids, ctx).await?;
    Ok(pairs
        .into_iter()
        .map(|(a, b)| Conflict {
            project_id: a.project_id.to_owned(),
            title: projects.title(a.project_id),
            with_project_id: b.project_id.to_owned(),
            with_title: projects.title(b.project_id),
            with_installed: b.installed,
        })
        .collect())
}

/// 按 ID 缓存项目，减少重复请求。
#[derive(Default)]
struct ProjectCache {
    projects: HashMap<String, Project>,
    missing: HashSet<String>,
}

impl ProjectCache {
    /// 批量获取尚未缓存的项目；不存在的记为缺失。
    async fn load(
        &mut self,
        client: &ModrinthClient,
        ids: &[String],
        ctx: &TaskCtx,
    ) -> AppResult<()> {
        let mut todo: Vec<String> = ids
            .iter()
            .filter(|id| !self.projects.contains_key(*id) && !self.missing.contains(*id))
            .cloned()
            .collect();
        todo.sort();
        todo.dedup();
        if todo.is_empty() {
            return Ok(());
        }
        for project in client.projects(&todo, ctx).await? {
            // 依赖可能用短名引用项目，按 ID 与短名都登记。
            self.projects.insert(project.slug.clone(), project.clone());
            self.projects.insert(project.id.clone(), project);
        }
        for id in todo {
            if !self.projects.contains_key(&id) {
                self.missing.insert(id);
            }
        }
        Ok(())
    }

    fn get(&self, id: &str) -> Option<&Project> {
        self.projects.get(id)
    }

    fn title(&self, id: &str) -> String {
        self.get(id)
            .map_or_else(|| id.to_owned(), |p| p.title.clone())
    }
}
