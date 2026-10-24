//! Modrinth API v2 的响应结构，只保留启动器用得到的字段。
//!
//! 时间保持 API 原样的 RFC 3339 字符串。枚举都带 `Other` 兜底，Modrinth 新增取值时不会解析失败。

use std::path::Path;

use serde::{Deserialize, Serialize};
use tmcl_net::DownloadItem;

/// 项目类型。数据包在 Modrinth 上是 `mod` 类型、加载器为 `datapack`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    /// 模组。
    Mod,
    /// 整合包。
    Modpack,
    /// 资源包。
    Resourcepack,
    /// 光影包。
    Shader,
    /// 服务端插件。
    Plugin,
    /// 数据包。
    Datapack,
    /// 未识别的类型。
    #[serde(other)]
    Other,
}

impl ProjectType {
    /// API 中使用的名称，用于搜索的 `project_type:` 条件；`Other` 返回 `None`。
    pub fn as_api_str(self) -> Option<&'static str> {
        Some(match self {
            Self::Mod => "mod",
            Self::Modpack => "modpack",
            Self::Resourcepack => "resourcepack",
            Self::Shader => "shader",
            Self::Plugin => "plugin",
            Self::Datapack => "datapack",
            Self::Other => return None,
        })
    }
}

/// 客户端或服务端对项目的支持程度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SideSupport {
    /// 必须安装。
    Required,
    /// 可选。
    Optional,
    /// 不支持。
    Unsupported,
    /// 作者未说明或无法识别。
    #[serde(other)]
    Unknown,
}

/// 搜索结果的排序方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchIndex {
    /// 相关度（Modrinth 默认）。
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

impl SearchIndex {
    /// 查询参数 `index` 的取值。
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Follows => "follows",
            Self::Newest => "newest",
            Self::Updated => "updated",
        }
    }
}

/// `GET /search` 的响应。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResponse {
    /// 本页结果。
    pub hits: Vec<SearchHit>,
    /// 本页起始位置。
    pub offset: u32,
    /// 本页最大条数。
    pub limit: u32,
    /// 符合条件的总数。
    pub total_hits: u64,
}

/// 搜索结果中的一个项目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// 项目 ID。
    pub project_id: String,
    /// 项目类型。
    pub project_type: ProjectType,
    /// 短名（URL 中使用）。
    pub slug: String,
    /// 作者用户名。
    #[serde(default)]
    pub author: String,
    /// 标题。
    pub title: String,
    /// 一句话简介。
    #[serde(default)]
    pub description: String,
    /// 分类，包含加载器名（如 `fabric`）。
    #[serde(default)]
    pub categories: Vec<String>,
    /// 支持的游戏版本。
    #[serde(default)]
    pub versions: Vec<String>,
    /// 下载量。
    #[serde(default)]
    pub downloads: u64,
    /// 关注数。
    #[serde(default)]
    pub follows: u64,
    /// 图标地址。
    #[serde(default)]
    pub icon_url: Option<String>,
    /// 创建时间。
    #[serde(default)]
    pub date_created: String,
    /// 最近更新时间。
    #[serde(default)]
    pub date_modified: String,
    /// 最新版本 ID。
    #[serde(default)]
    pub latest_version: Option<String>,
    /// 许可证 SPDX 标识。
    #[serde(default)]
    pub license: String,
    /// 客户端支持程度。
    pub client_side: SideSupport,
    /// 服务端支持程度。
    pub server_side: SideSupport,
    /// 精选图片地址。
    #[serde(default)]
    pub featured_gallery: Option<String>,
}

/// `GET /project/{id}` 的响应。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// 项目 ID。
    pub id: String,
    /// 短名。
    pub slug: String,
    /// 项目类型。
    pub project_type: ProjectType,
    /// 标题。
    pub title: String,
    /// 一句话简介。
    #[serde(default)]
    pub description: String,
    /// 详细介绍（Markdown）。
    #[serde(default)]
    pub body: String,
    /// 分类。
    #[serde(default)]
    pub categories: Vec<String>,
    /// 附加分类。
    #[serde(default)]
    pub additional_categories: Vec<String>,
    /// 客户端支持程度。
    pub client_side: SideSupport,
    /// 服务端支持程度。
    pub server_side: SideSupport,
    /// 支持的加载器。
    #[serde(default)]
    pub loaders: Vec<String>,
    /// 支持的游戏版本。
    #[serde(default)]
    pub game_versions: Vec<String>,
    /// 全部版本 ID。
    #[serde(default)]
    pub versions: Vec<String>,
    /// 下载量。
    #[serde(default)]
    pub downloads: u64,
    /// 关注数。
    #[serde(default)]
    pub followers: u64,
    /// 图标地址。
    #[serde(default)]
    pub icon_url: Option<String>,
    /// 问题反馈地址。
    #[serde(default)]
    pub issues_url: Option<String>,
    /// 源码地址。
    #[serde(default)]
    pub source_url: Option<String>,
    /// Wiki 地址。
    #[serde(default)]
    pub wiki_url: Option<String>,
    /// 许可证。
    pub license: License,
    /// 发布时间。
    #[serde(default)]
    pub published: String,
    /// 最近更新时间。
    #[serde(default)]
    pub updated: String,
    /// 图库。
    #[serde(default)]
    pub gallery: Vec<GalleryImage>,
}

/// 项目许可证。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct License {
    /// SPDX 标识或 `LicenseRef-*`。
    pub id: String,
    /// 显示名称。
    #[serde(default)]
    pub name: String,
    /// 许可证全文地址。
    #[serde(default)]
    pub url: Option<String>,
}

/// 图库中的一张图片。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalleryImage {
    /// 图片地址。
    pub url: String,
    /// 是否为精选图片。
    #[serde(default)]
    pub featured: bool,
    /// 标题。
    #[serde(default)]
    pub title: Option<String>,
    /// 说明。
    #[serde(default)]
    pub description: Option<String>,
}

/// 版本发布渠道。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionType {
    /// 正式版。
    Release,
    /// 测试版。
    Beta,
    /// 早期测试版。
    Alpha,
    /// 未识别的渠道。
    #[serde(other)]
    Other,
}

/// 项目的一个版本（`GET /version/{id}`、`GET /project/{id}/version` 等的元素）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Version {
    /// 版本 ID。
    pub id: String,
    /// 所属项目 ID。
    pub project_id: String,
    /// 发布者用户 ID。
    #[serde(default)]
    pub author_id: String,
    /// 版本名称。
    pub name: String,
    /// 版本号。
    pub version_number: String,
    /// 更新日志（Markdown）。
    #[serde(default)]
    pub changelog: Option<String>,
    /// 发布时间。
    #[serde(default)]
    pub date_published: String,
    /// 下载量。
    #[serde(default)]
    pub downloads: u64,
    /// 发布渠道。
    pub version_type: VersionType,
    /// 是否为精选版本。
    #[serde(default)]
    pub featured: bool,
    /// 支持的游戏版本。
    #[serde(default)]
    pub game_versions: Vec<String>,
    /// 支持的加载器。
    #[serde(default)]
    pub loaders: Vec<String>,
    /// 文件，通常只有一个主文件。
    #[serde(default)]
    pub files: Vec<VersionFile>,
    /// 依赖。
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl Version {
    /// 主文件：标记为 `primary` 的文件，没有标记时取第一个。
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }

    /// 必需前置（`required`）依赖。
    pub fn required_dependencies(&self) -> impl Iterator<Item = &Dependency> {
        self.dependencies
            .iter()
            .filter(|d| d.dependency_type == DependencyType::Required)
    }
}

/// 版本中的一个文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionFile {
    /// 文件哈希。
    pub hashes: VersionFileHashes,
    /// CDN 下载地址。
    pub url: String,
    /// 文件名。
    pub filename: String,
    /// 是否为主文件。
    #[serde(default)]
    pub primary: bool,
    /// 文件大小（字节）。
    pub size: u64,
    /// 附加文件类型（如 `required-resource-pack`），普通文件为 `None`。
    #[serde(default)]
    pub file_type: Option<String>,
}

impl VersionFile {
    /// 生成下载到 `dir/<filename>` 的下载项，带大小与 SHA1 校验。
    ///
    /// 文件名来自 API，只取最后一段路径，避免写到 `dir` 之外。
    pub fn download_item(&self, dir: &Path) -> DownloadItem {
        let name = Path::new(&self.filename)
            .file_name()
            .map_or_else(|| self.hashes.sha1.clone().into(), ToOwned::to_owned);
        DownloadItem {
            url: self.url.clone(),
            path: dir.join(name),
            size: Some(self.size),
            sha1: Some(self.hashes.sha1.to_ascii_lowercase()),
        }
    }
}

/// 文件哈希（小写十六进制）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionFileHashes {
    /// SHA1。
    pub sha1: String,
    /// SHA512。
    #[serde(default)]
    pub sha512: String,
}

/// 按文件哈希查询时使用的算法。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    /// SHA1（与下载引擎的校验一致）。
    #[default]
    Sha1,
    /// SHA512。
    Sha512,
}

impl HashAlgorithm {
    /// 参数 `algorithm` 的取值。
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha512 => "sha512",
        }
    }
}

/// 依赖关系的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    /// 必需前置。
    Required,
    /// 可选。
    Optional,
    /// 不兼容。
    Incompatible,
    /// 已内嵌，无需另装。
    Embedded,
    /// 未识别的类型。
    #[serde(other)]
    Other,
}

/// 版本的一条依赖。`version_id` 与 `project_id` 至少有一个，
/// 只有 `file_name` 时表示 Modrinth 之外的文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// 指定的版本 ID。
    #[serde(default)]
    pub version_id: Option<String>,
    /// 依赖的项目 ID。
    #[serde(default)]
    pub project_id: Option<String>,
    /// 外部文件名。
    #[serde(default)]
    pub file_name: Option<String>,
    /// 依赖类型。
    pub dependency_type: DependencyType,
}

/// `GET /project/{id}/dependencies` 的响应：所有版本依赖到的项目与指定版本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDependencies {
    /// 依赖的项目。
    pub projects: Vec<Project>,
    /// 依赖中指定的具体版本。
    pub versions: Vec<Version>,
}
