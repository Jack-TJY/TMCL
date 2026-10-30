//! 合并 Modrinth 与 CurseForge 的资源搜索，并列出 CurseForge 项目中适用于实例的文件。
//!
//! - 两个平台并发搜索，各取同一 `offset` / `limit` 的一页；一个平台失败时照常返回另一个的
//!   结果，并在 [`ResourcePage::failed`] 中标出。两个都失败时返回 Modrinth 的错误。
//! - 去重：短名或标题（只比较字母与数字，不分大小写）相同的视为同一项目，保留 Modrinth
//!   的条目，CurseForge 的条目记入 [`ResourceHit::also_on`]。只在同一页内去重。
//! - 排序：按下载量时合并后按下载量排序，按更新时间时按 `date_modified` 排序，其余交替排列
//!   （Modrinth 在前），保持各平台自身的相关度顺序。
//! - CurseForge 的过滤与 Modrinth 一致（见 [`InstanceTarget`]），Quilt 实例按 Fabric 过滤。

use tmcl_common::{AppResult, TaskCtx};

use crate::browse::{
    self, BrowseQuery, DEFAULT_LIMIT, InstanceTarget, ResourceHit, ResourceKind, ResourcePage,
    ResourcePlatform, ResourceRef, ResourceSort, ResourceVersion, is_not_found,
};
use crate::curseforge::{self, ClassId, CurseForgeClient, FileFilter, ModLoaderType};
use crate::modpack::source::{hit_from_mod, sort_field, version_from_file};
use crate::modrinth::ModrinthClient;

/// 可用的客户端与要搜索的平台。
#[derive(Debug, Clone, Copy)]
pub struct Platforms<'a> {
    /// Modrinth 客户端。
    pub modrinth: &'a ModrinthClient,
    /// CurseForge 客户端；没有 key 时为 `None`，此时只搜 Modrinth。
    pub curseforge: Option<&'a CurseForgeClient>,
    /// 要搜索的平台；为空表示全部可用的平台。
    pub only: &'a [ResourcePlatform],
}

impl Platforms<'_> {
    fn wants(&self, platform: ResourcePlatform) -> bool {
        self.only.is_empty() || self.only.contains(&platform)
    }
}

/// 合并搜索 `kind` 类资源；`target` 为 `None` 时不按实例过滤（数据包仍只要数据包）。
pub async fn search(
    platforms: Platforms<'_>,
    target: Option<&InstanceTarget>,
    kind: ResourceKind,
    query: &BrowseQuery,
    ctx: &TaskCtx,
) -> AppResult<ResourcePage> {
    let modrinth = async {
        if !platforms.wants(ResourcePlatform::Modrinth) {
            return None;
        }
        Some(match target {
            Some(target) => browse::search(platforms.modrinth, target, kind, query, ctx).await,
            None => browse::search_any(platforms.modrinth, kind, query, ctx).await,
        })
    };
    let curseforge = async {
        let client = platforms
            .curseforge
            .filter(|_| platforms.wants(ResourcePlatform::Curseforge))?;
        Some(search_curseforge(client, target, kind, query, ctx).await)
    };
    let (modrinth, curseforge) = tokio::join!(modrinth, curseforge);
    combine(modrinth, curseforge, query)
}

fn combine(
    modrinth: Option<AppResult<ResourcePage>>,
    curseforge: Option<AppResult<ResourcePage>>,
    query: &BrowseQuery,
) -> AppResult<ResourcePage> {
    let mut failed = Vec::new();
    let mut first_error = None;
    let mut pages = Vec::new();
    for (platform, result) in [
        (ResourcePlatform::Modrinth, modrinth),
        (ResourcePlatform::Curseforge, curseforge),
    ] {
        match result {
            None => {}
            Some(Ok(page)) => pages.push(page),
            Some(Err(err)) if err.is_cancelled() => return Err(err),
            Some(Err(err)) => {
                tracing::warn!(?platform, %err, "资源搜索失败");
                failed.push(platform);
                first_error.get_or_insert(err);
            }
        }
    }
    if pages.is_empty()
        && let Some(err) = first_error
    {
        return Err(err);
    }
    let total_hits = pages.iter().map(|p| p.total_hits).max().unwrap_or(0);
    let mut lists = pages.into_iter().map(|p| p.hits);
    let modrinth = lists.next().unwrap_or_default();
    let curseforge = lists.next().unwrap_or_default();
    Ok(ResourcePage {
        hits: merge(modrinth, curseforge, query.sort),
        offset: query.offset,
        limit: limit(query),
        total_hits,
        failed,
    })
}

/// 合并两页结果：先去重，再按 `sort` 排列。
pub fn merge(
    mut primary: Vec<ResourceHit>,
    secondary: Vec<ResourceHit>,
    sort: ResourceSort,
) -> Vec<ResourceHit> {
    let mut rest = Vec::new();
    for hit in secondary {
        let same = primary.iter_mut().find(|p| {
            p.platform != hit.platform
                && (same_name(&p.slug, &hit.slug) || same_name(&p.title, &hit.title))
        });
        match same {
            Some(p) => p.also_on.push(ResourceRef {
                platform: hit.platform,
                project_id: hit.project_id,
                slug: hit.slug,
            }),
            None => rest.push(hit),
        }
    }
    match sort {
        ResourceSort::Downloads => {
            primary.extend(rest);
            primary.sort_by_key(|h| std::cmp::Reverse(h.downloads));
            primary
        }
        ResourceSort::Updated => {
            primary.extend(rest);
            primary.sort_by(|a, b| b.date_modified.cmp(&a.date_modified));
            primary
        }
        _ => interleave(primary, rest),
    }
}

fn interleave(a: Vec<ResourceHit>, b: Vec<ResourceHit>) -> Vec<ResourceHit> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut a, mut b) = (a.into_iter(), b.into_iter());
    loop {
        match (a.next(), b.next()) {
            (None, None) => return out,
            (x, y) => out.extend(x.into_iter().chain(y)),
        }
    }
}

/// 只比较字母与数字，不分大小写；空名不算相同。
fn same_name(a: &str, b: &str) -> bool {
    let norm = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let a = norm(a);
    !a.is_empty() && a == norm(b)
}

fn limit(query: &BrowseQuery) -> u32 {
    if query.limit == 0 {
        DEFAULT_LIMIT
    } else {
        query.limit
    }
}

/// 资源种类对应的 CurseForge 顶级类别。
pub fn class_id(kind: ResourceKind) -> ClassId {
    match kind {
        ResourceKind::Mod => ClassId::Mods,
        ResourceKind::Resourcepack => ClassId::ResourcePacks,
        ResourceKind::Shader => ClassId::Shaders,
        ResourceKind::Datapack => ClassId::DataPacks,
    }
}

/// CurseForge 上按加载器过滤的条件：只对 mod 生效；Quilt 实例按 Fabric 过滤
/// （CurseForge 上大部分 Quilt 可用的模组只标了 Fabric）。
pub fn curseforge_loader(target: &InstanceTarget, kind: ResourceKind) -> Option<ModLoaderType> {
    match (kind, target.loader.as_deref()) {
        (ResourceKind::Mod, Some("quilt")) => Some(ModLoaderType::Fabric),
        (ResourceKind::Mod, Some(loader)) => ModLoaderType::from_loader_str(loader),
        _ => None,
    }
}

/// 在 CurseForge 搜索 `kind` 类资源。
pub async fn search_curseforge(
    client: &CurseForgeClient,
    target: Option<&InstanceTarget>,
    kind: ResourceKind,
    query: &BrowseQuery,
    ctx: &TaskCtx,
) -> AppResult<ResourcePage> {
    let search = curseforge::SearchQuery {
        query: query.query.trim().to_owned(),
        class_id: Some(class_id(kind)),
        game_version: target.map(|t| t.game_version.clone()),
        loader: target.and_then(|t| curseforge_loader(t, kind)),
        sort: sort_field(query.sort),
        offset: query.offset,
        limit: limit(query),
    };
    let page = client.search(&search, ctx).await?;
    Ok(ResourcePage {
        hits: page.data.into_iter().map(hit_from_mod).collect(),
        offset: page.pagination.index,
        limit: page.pagination.page_size,
        total_hits: page.pagination.total_count,
        failed: Vec::new(),
    })
}

/// 列出 CurseForge 项目中适用于该实例的文件（最近 50 个中筛选），新文件在前；
/// 项目不存在或 ID 不是数字时为空列表。
pub async fn versions_curseforge(
    client: &CurseForgeClient,
    target: &InstanceTarget,
    kind: ResourceKind,
    project_id: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<ResourceVersion>> {
    let Ok(id) = project_id.parse::<u32>() else {
        return Ok(Vec::new());
    };
    let filter = FileFilter {
        game_version: Some(target.game_version.clone()),
        loader: curseforge_loader(target, kind),
        limit: 50,
        ..FileFilter::default()
    };
    let page = match client.project_files(id, &filter, ctx).await {
        Ok(page) => page,
        Err(err) if is_not_found(&err) => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    Ok(page
        .data
        .iter()
        .filter(|f| f.is_available)
        .map(version_from_file)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tmcl_common::{AppError, ErrorCode};

    fn hit(platform: ResourcePlatform, id: &str, title: &str, downloads: u64) -> ResourceHit {
        ResourceHit {
            platform,
            project_id: id.to_owned(),
            slug: title.to_lowercase().replace(' ', "-"),
            title: title.to_owned(),
            description: String::new(),
            author: String::new(),
            icon_url: None,
            downloads,
            follows: 0,
            categories: Vec::new(),
            game_versions: Vec::new(),
            date_modified: format!("2026-01-{:02}T00:00:00Z", downloads % 28 + 1),
            also_on: Vec::new(),
        }
    }

    const M: ResourcePlatform = ResourcePlatform::Modrinth;
    const C: ResourcePlatform = ResourcePlatform::Curseforge;

    fn ids(hits: &[ResourceHit]) -> Vec<&str> {
        hits.iter().map(|h| h.project_id.as_str()).collect()
    }

    #[test]
    fn duplicates_fold_into_modrinth_hit() {
        let m = vec![hit(M, "AANobbMI", "Sodium", 10), hit(M, "jei-m", "JEI", 5)];
        let mut jei = hit(C, "238222", "Just Enough Items (JEI)", 50);
        jei.slug = "jei".into();
        let c = vec![hit(C, "394468", "Sodium", 20), jei, hit(C, "1", "Other", 1)];
        let merged = merge(m, c, ResourceSort::Relevance);
        assert_eq!(ids(&merged), ["AANobbMI", "1", "jei-m"]);
        assert_eq!(
            merged[0].also_on,
            [ResourceRef {
                platform: C,
                project_id: "394468".into(),
                slug: "sodium".into()
            }]
        );
        assert_eq!(merged[2].also_on[0].project_id, "238222");
        assert!(merged[1].also_on.is_empty());
    }

    #[test]
    fn names_compare_letters_and_digits_only() {
        assert!(same_name("Fabric API", "fabric-api"));
        assert!(same_name("JEI", "jei"));
        assert!(!same_name("", ""));
        assert!(!same_name("Create", "Create Addition"));
    }

    #[test]
    fn sort_orders() {
        let m = || vec![hit(M, "m1", "A", 3), hit(M, "m2", "B", 1)];
        let c = || {
            vec![
                hit(C, "c1", "C", 9),
                hit(C, "c2", "D", 2),
                hit(C, "c3", "E", 0),
            ]
        };
        assert_eq!(
            ids(&merge(m(), c(), ResourceSort::Relevance)),
            ["m1", "c1", "m2", "c2", "c3"]
        );
        assert_eq!(
            ids(&merge(m(), c(), ResourceSort::Downloads)),
            ["c1", "m1", "c2", "m2", "c3"]
        );
        assert_eq!(
            ids(&merge(m(), c(), ResourceSort::Updated)),
            ["c1", "m1", "c2", "m2", "c3"]
        );
    }

    fn page(hits: Vec<ResourceHit>, total: u64) -> ResourcePage {
        ResourcePage {
            hits,
            offset: 0,
            limit: 20,
            total_hits: total,
            failed: Vec::new(),
        }
    }

    #[test]
    fn one_failed_platform_is_reported_not_fatal() {
        let q = BrowseQuery::default();
        let err = || AppError::new(ErrorCode::NetConnect);
        let out = combine(
            Some(Ok(page(vec![hit(M, "m", "A", 1)], 7))),
            Some(Err(err())),
            &q,
        )
        .unwrap();
        assert_eq!(ids(&out.hits), ["m"]);
        assert_eq!(out.failed, [C]);
        assert_eq!(out.total_hits, 7);

        let out = combine(
            Some(Ok(page(vec![], 3))),
            Some(Ok(page(vec![hit(C, "c", "B", 1)], 40))),
            &q,
        )
        .unwrap();
        assert_eq!(out.total_hits, 40);
        assert!(out.failed.is_empty());

        let e = combine(Some(Err(err())), Some(Err(err())), &q).unwrap_err();
        assert_eq!(e.code, ErrorCode::NetConnect);
        assert!(combine(None, None, &q).unwrap().hits.is_empty());
    }

    #[test]
    fn curseforge_filters_follow_instance() {
        let target = |loader: Option<&str>| InstanceTarget {
            game_version: "1.20.1".into(),
            loader: loader.map(Into::into),
        };
        assert_eq!(
            curseforge_loader(&target(Some("quilt")), ResourceKind::Mod),
            Some(ModLoaderType::Fabric)
        );
        assert_eq!(
            curseforge_loader(&target(Some("neoforge")), ResourceKind::Mod),
            Some(ModLoaderType::NeoForge)
        );
        assert_eq!(
            curseforge_loader(&target(Some("forge")), ResourceKind::Shader),
            None
        );
        assert_eq!(curseforge_loader(&target(None), ResourceKind::Mod), None);
        assert_eq!(class_id(ResourceKind::Datapack), ClassId::DataPacks);
    }
}
