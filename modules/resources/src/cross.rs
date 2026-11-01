//! 跨平台补全必需前置：前置在来源平台上没有适用于实例的版本（或项目只在另一个平台上）时，
//! 按短名或名称（只比较字母与数字，见 [`same_name`]）到另一个平台查找同一项目，选出适用于实例
//! 游戏版本与加载器的文件，加入安装计划。
//!
//! - 补全的前置在其所在平台上继续递归解析自己的必需前置；那里仍缺的前置不再换回原平台查找，
//!   避免两个平台之间来回解析。名称与计划中已有文件相同的前置视为已满足。
//! - CurseForge 不可用（没有 key）时不查找；另一平台查询失败时只记日志，前置照常报告为缺失。
//! - CurseForge 文件的下载规则不变：禁止第三方分发的文件先找 Modrinth 上 SHA1 相同的文件，
//!   再走 MCIM，都不行时报告为缺失（需要手动下载）。
//! - 只有补全后仍找不到的前置留在 [`crate::InstallPlan::unavailable`] 中。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use tmcl_common::{AppResult, TaskCtx};

use crate::browse::{InstanceTarget, ResourceKind};
use crate::cf_install::{self, Titles};
use crate::curseforge::{self, CurseForgeClient, File};
use crate::deps::{self, UnavailableDependency};
use crate::install::{Prepared, kind_dir, push_modrinth, scan_installed};
use crate::merged::{class_id, curseforge_loader, same_name};
use crate::modrinth::{ModrinthClient, ProjectType, SearchQuery, Version};

/// 按名称搜索时最多比对的条目数。
const SEARCH_LIMIT: u32 = 20;

/// 要安装到的实例。
pub(crate) struct Instance<'a> {
    pub(crate) target: &'a InstanceTarget,
    /// 实例的 `game/` 目录。
    pub(crate) game_dir: &'a Path,
    /// 数据包所在的存档。
    pub(crate) world: Option<&'a str>,
}

/// Modrinth 上缺失的前置改从 CurseForge 安装。返回补全的文件与仍缺失的前置；`planned` 为
/// 计划中已有文件的标题。
pub(crate) async fn from_curseforge(
    cf: &CurseForgeClient,
    modrinth: &ModrinthClient,
    inst: &Instance<'_>,
    missing: Vec<UnavailableDependency>,
    planned: &[String],
    ctx: &TaskCtx,
) -> AppResult<Prepared> {
    if missing.iter().all(|u| u.title.is_none()) {
        return Ok(Prepared::unavailable(missing));
    }
    let result = try_curseforge(cf, modrinth, inst, missing.clone(), planned, ctx).await;
    graceful(result, missing, "CurseForge")
}

async fn try_curseforge(
    cf: &CurseForgeClient,
    modrinth: &ModrinthClient,
    inst: &Instance<'_>,
    missing: Vec<UnavailableDependency>,
    planned: &[String],
    ctx: &TaskCtx,
) -> AppResult<Prepared> {
    // 只有项目存在（有标题）的前置才知道名称。
    let ids: Vec<String> = missing
        .iter()
        .filter(|u| u.title.is_some())
        .filter_map(|u| u.project_id.clone())
        .collect();
    let projects: HashMap<String, _> = modrinth
        .projects(&ids, ctx)
        .await?
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();
    let mut out = Prepared::default();
    let mut roots = Vec::new();
    for u in missing {
        let project = u
            .project_id
            .as_ref()
            .and_then(|id| projects.get(id))
            .filter(|p| p.project_type == ProjectType::Mod);
        let found = match project {
            Some(p) => curseforge_match(cf, inst.target, &p.title, &p.slug, ctx).await?,
            None => None,
        };
        match found {
            Some(file) => roots.push((file, ResourceKind::Mod, Some(u.required_by))),
            None => out.plan.unavailable.push(u),
        }
    }
    if roots.is_empty() {
        return Ok(out);
    }
    let dirs = [kind_dir(inst.game_dir, ResourceKind::Mod, None)?];
    let installed = cf_install::installed_projects(cf, &dirs, ctx).await?;
    // 已经以 CurseForge 文件的形式装在实例中。
    roots.retain(|(f, _, _)| !installed.contains_key(&f.mod_id));
    let resolution = cf_install::resolve(cf, inst.target, &installed, roots, planned, ctx).await?;
    out.plan.conflicts.extend(resolution.conflicts);
    out.plan.unavailable.extend(resolution.unavailable);
    cf_install::push_planned(
        modrinth,
        inst.game_dir,
        inst.world,
        &installed,
        resolution.planned,
        &mut out,
        ctx,
    )
    .await?;
    Ok(out)
}

/// CurseForge 上缺失的前置改从 Modrinth 安装。`titles` 为 CurseForge 项目的名称与短名，
/// `planned` 为计划中已有文件的标题。
pub(crate) async fn from_modrinth(
    modrinth: &ModrinthClient,
    inst: &Instance<'_>,
    missing: Vec<UnavailableDependency>,
    titles: &Titles,
    planned: &[String],
    ctx: &TaskCtx,
) -> AppResult<Prepared> {
    if missing.is_empty() {
        return Ok(Prepared::default());
    }
    let result = try_modrinth(modrinth, inst, missing.clone(), titles, planned, ctx).await;
    graceful(result, missing, "Modrinth")
}

async fn try_modrinth(
    modrinth: &ModrinthClient,
    inst: &Instance<'_>,
    missing: Vec<UnavailableDependency>,
    titles: &Titles,
    planned: &[String],
    ctx: &TaskCtx,
) -> AppResult<Prepared> {
    let mut out = Prepared::default();
    let mut roots = Vec::new();
    for u in missing {
        let names = u
            .project_id
            .as_ref()
            .and_then(|id| id.parse::<u32>().ok())
            .and_then(|id| titles.get(&id));
        let found = match names {
            Some((name, slug)) => modrinth_match(modrinth, inst.target, name, slug, ctx).await?,
            None => None,
        };
        match found {
            Some(version) => roots.push((version, u.required_by)),
            None => out.plan.unavailable.push(u),
        }
    }
    if roots.is_empty() {
        return Ok(out);
    }
    let dirs = [kind_dir(inst.game_dir, ResourceKind::Mod, None)?];
    let installed = scan_installed(modrinth, &dirs, ctx).await?;
    let mut seen: HashSet<String> = installed
        .iter()
        .map(|i| i.version.project_id.clone())
        .collect();
    let mut names = planned.to_vec();
    for (version, required_by) in roots {
        // 已经以 Modrinth 文件的形式装在实例中，或已由前一个补全的前置带入。
        if !seen.insert(version.project_id.clone()) {
            continue;
        }
        let resolution = deps::resolve(
            modrinth,
            inst.target,
            version,
            ResourceKind::Mod,
            &installed,
            ctx,
        )
        .await?;
        let mut resolved = resolution.resolved.into_iter();
        let mut keep = Vec::new();
        if let Some(mut root) = resolved.next() {
            root.required_by = Some(required_by);
            keep.push(root);
        }
        for r in resolved {
            let duplicate = names.iter().any(|n| same_name(n, &r.title));
            if seen.insert(r.version.project_id.clone()) && !duplicate {
                keep.push(r);
            }
        }
        names.extend(keep.iter().map(|r| r.title.clone()));
        out.plan.conflicts.extend(resolution.conflicts);
        out.plan.unavailable.extend(resolution.unavailable);
        push_modrinth(inst.game_dir, inst.world, &installed, keep, &mut out)?;
    }
    Ok(out)
}

/// 另一平台查询失败时不影响原计划：前置仍报告为缺失。取消照常返回。
fn graceful(
    result: AppResult<Prepared>,
    missing: Vec<UnavailableDependency>,
    platform: &str,
) -> AppResult<Prepared> {
    match result {
        Ok(prepared) => Ok(prepared),
        Err(err) if err.is_cancelled() => Err(err),
        Err(err) => {
            tracing::warn!(%err, platform, "在另一平台查找缺失的前置失败");
            Ok(Prepared::unavailable(missing))
        }
    }
}

/// 在 CurseForge 上按名称搜索 mod，取短名或名称相同的项目中适用于实例的文件。
async fn curseforge_match(
    cf: &CurseForgeClient,
    target: &InstanceTarget,
    title: &str,
    slug: &str,
    ctx: &TaskCtx,
) -> AppResult<Option<File>> {
    let query = curseforge::SearchQuery {
        query: title.to_owned(),
        class_id: Some(class_id(ResourceKind::Mod)),
        game_version: Some(target.game_version.clone()),
        loader: curseforge_loader(target, ResourceKind::Mod),
        limit: SEARCH_LIMIT,
        ..curseforge::SearchQuery::default()
    };
    let hits = cf.search(&query, ctx).await?.data;
    let Some(hit) = hits
        .iter()
        .find(|m| same_name(&m.slug, slug) || same_name(&m.name, title))
    else {
        return Ok(None);
    };
    cf_install::pick_file(cf, target, hit.id, ctx).await
}

/// 在 Modrinth 上找短名或名称相同的 mod：先按短名直接取项目，再按名称搜索；取适用于实例的版本。
async fn modrinth_match(
    modrinth: &ModrinthClient,
    target: &InstanceTarget,
    title: &str,
    slug: &str,
    ctx: &TaskCtx,
) -> AppResult<Option<Version>> {
    let by_slug = match modrinth.project(slug, ctx).await {
        Ok(project) => project.filter(|p| {
            p.project_type == ProjectType::Mod
                && (same_name(&p.slug, slug) || same_name(&p.title, title))
        }),
        Err(err) if crate::browse::is_not_found(&err) => None,
        Err(err) => return Err(err),
    };
    let project_id = match by_slug {
        Some(p) => p.id,
        None => {
            let query = SearchQuery {
                query: title.to_owned(),
                project_type: Some(ProjectType::Mod),
                game_versions: vec![target.game_version.clone()],
                loaders: target.loaders(ResourceKind::Mod),
                limit: SEARCH_LIMIT,
                ..SearchQuery::default()
            };
            let hits = modrinth.search(&query, ctx).await?.hits;
            match hits
                .into_iter()
                .find(|h| same_name(&h.slug, slug) || same_name(&h.title, title))
            {
                Some(hit) => hit.project_id,
                None => return Ok(None),
            }
        }
    };
    deps::pick(modrinth, target, ResourceKind::Mod, &project_id, ctx).await
}
