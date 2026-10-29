//! 统一的资源浏览：按实例的游戏版本与加载器搜索 mod、资源包、光影与数据包，并列出可安装的版本。
//!
//! 只做 Modrinth（CurseForge 见 M2-02）。过滤规则见 [`InstanceTarget`]。

use serde::{Deserialize, Serialize};
use tmcl_common::{AppResult, TaskCtx};
use ts_rs::TS;

use crate::modrinth::{
    ModrinthClient, ProjectType, SearchHit, SearchIndex, SearchQuery, Version, VersionFilter,
    VersionType,
};

/// 可以浏览与安装的资源种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    /// 模组，装到 `game/mods/`。
    Mod,
    /// 资源包，装到 `game/resourcepacks/`。
    Resourcepack,
    /// 光影包，装到 `game/shaderpacks/`。
    Shader,
    /// 数据包，装到 `game/saves/<存档>/datapacks/`。
    Datapack,
}

impl ResourceKind {
    /// 对应的 Modrinth 项目类型（搜索的 `project_type:` 条件）。
    pub fn project_type(self) -> ProjectType {
        match self {
            Self::Mod => ProjectType::Mod,
            Self::Resourcepack => ProjectType::Resourcepack,
            Self::Shader => ProjectType::Shader,
            Self::Datapack => ProjectType::Datapack,
        }
    }
}

/// 安装目标实例的游戏版本与加载器，决定搜索与选版本时的过滤条件：
///
/// | 种类 | 游戏版本 | 加载器 |
/// | --- | --- | --- |
/// | mod | 实例版本 | 实例加载器（Quilt 实例同时接受 Fabric）；原版实例不过滤 |
/// | 资源包、光影 | 实例版本 | 不过滤（光影的“加载器”是 Iris / OptiFine 等，由用户自行选择） |
/// | 数据包 | 实例版本 | `datapack`，避免选到同一项目的模组文件 |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceTarget {
    /// 游戏版本，例如 `1.20.1`。
    pub game_version: String,
    /// 加载器（`fabric`、`quilt`、`forge`、`neoforge`），原版为 `None`。
    pub loader: Option<String>,
}

impl InstanceTarget {
    /// 该种类资源要求的加载器，满足任一即可；为空表示不限。
    pub fn loaders(&self, kind: ResourceKind) -> Vec<String> {
        match kind {
            ResourceKind::Mod => match self.loader.as_deref() {
                Some("quilt") => vec!["quilt".to_owned(), "fabric".to_owned()],
                Some(loader) => vec![loader.to_owned()],
                None => Vec::new(),
            },
            ResourceKind::Datapack => vec!["datapack".to_owned()],
            ResourceKind::Resourcepack | ResourceKind::Shader => Vec::new(),
        }
    }

    /// 列出项目版本时使用的过滤条件。
    pub fn version_filter(&self, kind: ResourceKind) -> VersionFilter {
        VersionFilter {
            loaders: self.loaders(kind),
            game_versions: vec![self.game_version.clone()],
            featured: None,
        }
    }
}

/// 搜索结果的排序方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ResourceSort {
    /// 相关度。
    #[default]
    Relevance,
    /// 下载量。
    Downloads,
    /// 关注数。
    Follows,
    /// 发布时间。
    Newest,
    /// 更新时间。
    Updated,
}

impl From<ResourceSort> for SearchIndex {
    fn from(sort: ResourceSort) -> Self {
        match sort {
            ResourceSort::Relevance => Self::Relevance,
            ResourceSort::Downloads => Self::Downloads,
            ResourceSort::Follows => Self::Follows,
            ResourceSort::Newest => Self::Newest,
            ResourceSort::Updated => Self::Updated,
        }
    }
}

/// 一次搜索的条件。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrowseQuery {
    /// 关键词，可为空。
    pub query: String,
    /// 排序方式。
    pub sort: ResourceSort,
    /// 跳过的条数。
    pub offset: u32,
    /// 每页条数，0 为默认（20），最多 100。
    pub limit: u32,
}

/// 搜索结果中的一个项目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ResourceHit {
    /// Modrinth 项目 ID。
    pub project_id: String,
    /// 短名，用于拼项目页地址。
    pub slug: String,
    /// 标题。
    pub title: String,
    /// 一句话简介。
    pub description: String,
    /// 作者用户名。
    pub author: String,
    /// 图标地址。
    pub icon_url: Option<String>,
    /// 下载量。
    #[ts(type = "number")]
    pub downloads: u64,
    /// 关注数。
    #[ts(type = "number")]
    pub follows: u64,
    /// 分类（含加载器名）。
    pub categories: Vec<String>,
    /// 支持的游戏版本。
    pub game_versions: Vec<String>,
    /// 最近更新时间（RFC 3339）。
    pub date_modified: String,
}

impl From<SearchHit> for ResourceHit {
    fn from(hit: SearchHit) -> Self {
        Self {
            project_id: hit.project_id,
            slug: hit.slug,
            title: hit.title,
            description: hit.description,
            author: hit.author,
            icon_url: hit.icon_url,
            downloads: hit.downloads,
            follows: hit.follows,
            categories: hit.categories,
            game_versions: hit.versions,
            date_modified: hit.date_modified,
        }
    }
}

/// 一页搜索结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ResourcePage {
    /// 本页结果。
    pub hits: Vec<ResourceHit>,
    /// 本页起始位置。
    pub offset: u32,
    /// 本页最大条数。
    pub limit: u32,
    /// 符合条件的总数。
    #[ts(type = "number")]
    pub total_hits: u64,
}

/// 版本的发布渠道。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ResourceChannel {
    /// 正式版。
    Release,
    /// 测试版。
    Beta,
    /// 早期测试版。
    Alpha,
}

/// 项目的一个可安装版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ResourceVersion {
    /// 版本 ID，安装时使用。
    pub id: String,
    /// 所属项目 ID。
    pub project_id: String,
    /// 版本名称。
    pub name: String,
    /// 版本号。
    pub version_number: String,
    /// 发布渠道；无法识别的渠道按 `alpha` 处理。
    pub channel: ResourceChannel,
    /// 发布时间（RFC 3339）。
    pub date_published: String,
    /// 下载量。
    #[ts(type = "number")]
    pub downloads: u64,
    /// 支持的游戏版本。
    pub game_versions: Vec<String>,
    /// 支持的加载器。
    pub loaders: Vec<String>,
    /// 主文件名。
    pub file_name: String,
    /// 主文件大小（字节）。
    #[ts(type = "number")]
    pub file_size: u64,
}

impl ResourceVersion {
    /// 由 Modrinth 版本转换；没有文件的版本无法安装，返回 `None`。
    pub fn from_version(version: &Version) -> Option<Self> {
        let file = version.primary_file()?;
        Some(Self {
            id: version.id.clone(),
            project_id: version.project_id.clone(),
            name: version.name.clone(),
            version_number: version.version_number.clone(),
            channel: match version.version_type {
                VersionType::Release => ResourceChannel::Release,
                VersionType::Beta => ResourceChannel::Beta,
                VersionType::Alpha | VersionType::Other => ResourceChannel::Alpha,
            },
            date_published: version.date_published.clone(),
            downloads: version.downloads,
            game_versions: version.game_versions.clone(),
            loaders: version.loaders.clone(),
            file_name: file.filename.clone(),
            file_size: file.size,
        })
    }
}

/// 默认每页条数。
const DEFAULT_LIMIT: u32 = 20;

/// 按实例过滤后搜索 `kind` 类资源。
pub async fn search(
    client: &ModrinthClient,
    target: &InstanceTarget,
    kind: ResourceKind,
    query: &BrowseQuery,
    ctx: &TaskCtx,
) -> AppResult<ResourcePage> {
    let search = SearchQuery {
        query: query.query.trim().to_owned(),
        project_type: Some(kind.project_type()),
        game_versions: vec![target.game_version.clone()],
        loaders: target.loaders(kind),
        categories: Vec::new(),
        index: query.sort.into(),
        offset: query.offset,
        limit: if query.limit == 0 {
            DEFAULT_LIMIT
        } else {
            query.limit
        },
    };
    let response = client.search(&search, ctx).await?;
    Ok(ResourcePage {
        hits: response.hits.into_iter().map(Into::into).collect(),
        offset: response.offset,
        limit: response.limit,
        total_hits: response.total_hits,
    })
}

/// 不按实例过滤，搜索全部 `kind` 类资源（数据包仍只要 `datapack` 加载器的项目）。
pub async fn search_any(
    client: &ModrinthClient,
    kind: ResourceKind,
    query: &BrowseQuery,
    ctx: &TaskCtx,
) -> AppResult<ResourcePage> {
    let loaders = match kind {
        ResourceKind::Datapack => vec!["datapack".to_owned()],
        _ => Vec::new(),
    };
    let search = SearchQuery {
        query: query.query.trim().to_owned(),
        project_type: Some(kind.project_type()),
        game_versions: Vec::new(),
        loaders,
        categories: Vec::new(),
        index: query.sort.into(),
        offset: query.offset,
        limit: if query.limit == 0 {
            DEFAULT_LIMIT
        } else {
            query.limit
        },
    };
    let response = client.search(&search, ctx).await?;
    Ok(ResourcePage {
        hits: response.hits.into_iter().map(Into::into).collect(),
        offset: response.offset,
        limit: response.limit,
        total_hits: response.total_hits,
    })
}

/// 列出项目中适用于该实例的版本，新版本在前；没有文件的版本被跳过。
/// 项目不存在时返回空列表。
pub async fn versions(
    client: &ModrinthClient,
    target: &InstanceTarget,
    kind: ResourceKind,
    project_id: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<ResourceVersion>> {
    let filter = target.version_filter(kind);
    let versions = match client.project_versions(project_id, &filter, ctx).await {
        Ok(versions) => versions,
        Err(err) if is_not_found(&err) => Vec::new(),
        Err(err) => return Err(err),
    };
    Ok(versions
        .iter()
        .filter_map(ResourceVersion::from_version)
        .collect())
}

/// Modrinth 客户端把列表接口的 404 报告为 `NET_HTTP_STATUS`（`status` = 404）。
pub(crate) fn is_not_found(err: &tmcl_common::AppError) -> bool {
    err.code == tmcl_common::ErrorCode::NetHttpStatus
        && err.params.get("status").map(String::as_str) == Some("404")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(loader: Option<&str>) -> InstanceTarget {
        InstanceTarget {
            game_version: "1.20.1".to_owned(),
            loader: loader.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn loaders_depend_on_kind_and_instance() {
        assert_eq!(
            target(Some("fabric")).loaders(ResourceKind::Mod),
            ["fabric"]
        );
        assert_eq!(
            target(Some("quilt")).loaders(ResourceKind::Mod),
            ["quilt", "fabric"]
        );
        assert!(target(None).loaders(ResourceKind::Mod).is_empty());
        assert!(
            target(Some("forge"))
                .loaders(ResourceKind::Shader)
                .is_empty()
        );
        assert!(
            target(Some("forge"))
                .loaders(ResourceKind::Resourcepack)
                .is_empty()
        );
        assert_eq!(target(None).loaders(ResourceKind::Datapack), ["datapack"]);
        let filter = target(Some("neoforge")).version_filter(ResourceKind::Mod);
        assert_eq!(filter.game_versions, ["1.20.1"]);
        assert_eq!(filter.loaders, ["neoforge"]);
    }

    #[test]
    fn kind_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&ResourceKind::Resourcepack).unwrap(),
            "\"resourcepack\""
        );
        assert_eq!(ResourceKind::Shader.project_type(), ProjectType::Shader);
    }
}
