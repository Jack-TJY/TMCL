//! CurseForge API v1 的响应结构（<https://docs.curseforge.com/rest-api/>），只保留启动器用到的字段。
//!
//! 枚举在 API 中是整数；未知取值统一落到 `Other`，避免新增取值导致整个响应解析失败。

use serde::{Deserialize, Deserializer};
use tmcl_net::DownloadItem;

/// Minecraft 在 CurseForge 上的游戏 ID。
pub const MINECRAFT_GAME_ID: u32 = 432;

/// 列表响应：`{ data, pagination }`。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Page<T> {
    /// 本页条目。
    pub data: Vec<T>,
    /// 分页信息。
    pub pagination: Pagination,
}

/// 分页信息。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    /// 跳过的条数。
    pub index: u32,
    /// 请求的每页条数。
    pub page_size: u32,
    /// 本页实际条数。
    pub result_count: u32,
    /// 总条数（搜索最多能翻到前 10000 条）。
    pub total_count: u64,
}

/// 项目（API 中叫 Mod，也包括整合包、资源包、光影等）。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    /// 项目 ID。
    pub id: u32,
    /// 名称。
    pub name: String,
    /// 短名，用于网页地址。
    pub slug: String,
    /// 简介。
    #[serde(default, deserialize_with = "nullable")]
    pub summary: String,
    /// 网页链接。
    #[serde(default, deserialize_with = "nullable")]
    pub links: ModLinks,
    /// 下载次数。
    #[serde(default)]
    pub download_count: f64,
    /// 图标。
    #[serde(default)]
    pub logo: Option<Asset>,
    /// 作者。
    #[serde(default, deserialize_with = "nullable")]
    pub authors: Vec<Author>,
    /// 分类（顶级类别，如 mod / 整合包），见 [`ClassId`]。
    #[serde(default)]
    pub class_id: Option<ClassId>,
    /// 子分类。
    #[serde(default, deserialize_with = "nullable")]
    pub categories: Vec<Category>,
    /// 各游戏版本 + 加载器的最新文件索引。
    #[serde(default, deserialize_with = "nullable")]
    pub latest_files_indexes: Vec<FileIndex>,
    /// 作者是否允许第三方分发文件。为 `false` 时文件的 `downloadUrl` 为空，只能去网页下载。
    #[serde(default)]
    pub allow_mod_distribution: Option<bool>,
    /// 最后修改时间（RFC 3339）。
    #[serde(default, deserialize_with = "nullable")]
    pub date_modified: String,
}

impl Mod {
    /// 项目网页；API 没给时按短名拼出 mod 页面。
    pub fn website_url(&self) -> String {
        match &self.links.website_url {
            Some(url) if url.starts_with("https://") => url.trim_end_matches('/').to_owned(),
            _ => format!("https://www.curseforge.com/minecraft/mc-mods/{}", self.slug),
        }
    }

    /// 某个文件的下载页面，用于作者禁止第三方下载时让用户手动下载。
    pub fn file_page_url(&self, file_id: u32) -> String {
        format!("{}/files/{file_id}", self.website_url())
    }

    /// 支持的游戏版本（去重，按 API 顺序）。
    pub fn game_versions(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for index in &self.latest_files_indexes {
            if !out.contains(&index.game_version) {
                out.push(index.game_version.clone());
            }
        }
        out
    }
}

/// 项目的网页链接。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLinks {
    /// 项目主页。
    #[serde(default)]
    pub website_url: Option<String>,
}

/// 图片资源。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    /// 缩略图。
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    /// 原图。
    #[serde(default)]
    pub url: Option<String>,
}

/// 作者。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Author {
    /// 名称。
    pub name: String,
}

/// 子分类。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    /// 分类 ID。
    pub id: u32,
    /// 名称（英文）。
    pub name: String,
    /// 短名。
    #[serde(default, deserialize_with = "nullable")]
    pub slug: String,
}

/// `latestFilesIndexes` 的一项。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIndex {
    /// 游戏版本。
    pub game_version: String,
    /// 文件 ID。
    pub file_id: u32,
    /// 加载器，未指定时为 `None`。
    #[serde(default)]
    pub mod_loader: Option<ModLoaderType>,
}

/// 项目的一个文件（即一个版本）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// 文件 ID。
    pub id: u32,
    /// 所属项目 ID。
    pub mod_id: u32,
    /// 显示名。
    #[serde(default, deserialize_with = "nullable")]
    pub display_name: String,
    /// 文件名。
    pub file_name: String,
    /// 发布类型。
    pub release_type: ReleaseType,
    /// 哈希。
    #[serde(default, deserialize_with = "nullable")]
    pub hashes: Vec<FileHash>,
    /// 大小（字节）。
    #[serde(default)]
    pub file_length: u64,
    /// 下载地址；作者禁止第三方分发时为空。
    #[serde(default)]
    pub download_url: Option<String>,
    /// 游戏版本与加载器名称混在一起，如 `["1.20.1", "Fabric", "Client"]`。
    #[serde(default, deserialize_with = "nullable")]
    pub game_versions: Vec<String>,
    /// 依赖。
    #[serde(default, deserialize_with = "nullable")]
    pub dependencies: Vec<FileDependency>,
    /// MurmurHash2 指纹，见 [`super::fingerprint`]。
    #[serde(default)]
    pub file_fingerprint: u32,
    /// 发布时间（RFC 3339）。
    #[serde(default, deserialize_with = "nullable")]
    pub file_date: String,
    /// 文件是否可用（未被删除或屏蔽）。
    #[serde(default = "default_true")]
    pub is_available: bool,
}

/// API 有时用 `null` 代替空数组或空串。
fn nullable<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Option::<T>::deserialize(d).map(Option::unwrap_or_default)
}

fn default_true() -> bool {
    true
}

impl File {
    /// SHA1（小写十六进制）。
    pub fn sha1(&self) -> Option<String> {
        self.hashes
            .iter()
            .find(|h| h.algo == HashAlgo::Sha1)
            .map(|h| h.value.to_ascii_lowercase())
            .filter(|h| h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()))
    }

    /// 可直接下载的地址；作者禁止第三方分发时为 `None`。
    ///
    /// 有 SHA1 时也接受 http（下载后按 SHA1 校验），否则只接受 https。
    pub fn usable_download_url(&self) -> Option<&str> {
        let url = self.download_url.as_deref()?;
        let ok =
            url.starts_with("https://") || (url.starts_with("http://") && self.sha1().is_some());
        ok.then_some(url)
    }

    /// 下载到 `dir/<文件名>` 的下载项；不能直接下载或文件名不安全时为 `None`。
    pub fn download_item(&self, dir: &std::path::Path) -> Option<DownloadItem> {
        let url = self.usable_download_url()?;
        if !is_safe_file_name(&self.file_name) {
            return None;
        }
        Some(DownloadItem {
            url: url.to_owned(),
            path: dir.join(&self.file_name),
            size: (self.file_length > 0).then_some(self.file_length),
            sha1: self.sha1(),
        })
    }

    /// `gameVersions` 中的游戏版本（去掉加载器与环境标签）。
    pub fn minecraft_versions(&self) -> Vec<String> {
        self.game_versions
            .iter()
            .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
            .cloned()
            .collect()
    }

    /// `gameVersions` 中的加载器（小写，如 `fabric`）。
    pub fn loaders(&self) -> Vec<String> {
        self.game_versions
            .iter()
            .filter_map(|v| ModLoaderType::from_label(v))
            .filter_map(ModLoaderType::as_loader_str)
            .map(ToOwned::to_owned)
            .collect()
    }
}

/// 文件名不含路径分隔符、`..`、控制字符，且不以 `.` 开头。
pub(crate) fn is_safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.contains("..")
        && !name.chars().any(char::is_control)
}

/// 文件哈希。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileHash {
    /// 十六进制值。
    pub value: String,
    /// 算法。
    pub algo: HashAlgo,
}

/// 文件依赖。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDependency {
    /// 依赖的项目 ID。
    pub mod_id: u32,
    /// 关系。
    pub relation_type: RelationType,
}

/// 按指纹查找的结果（`POST /fingerprints/{gameId}`）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintMatches {
    /// 精确匹配的文件。
    #[serde(default, deserialize_with = "nullable")]
    pub exact_matches: Vec<FingerprintMatch>,
    /// 没有匹配的指纹。
    #[serde(default, deserialize_with = "nullable")]
    pub unmatched_fingerprints: Vec<u32>,
}

/// 一个精确匹配。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintMatch {
    /// 项目 ID。
    pub id: u32,
    /// 匹配到的文件。
    pub file: File,
    /// 该项目各游戏版本的最新文件。
    #[serde(default, deserialize_with = "nullable")]
    pub latest_files: Vec<File>,
}

macro_rules! int_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $value:literal,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant,)*
            /// API 新增的、本客户端不认识的取值。
            Other(i64),
        }

        impl $name {
            /// API 中的整数取值。
            pub fn as_i64(self) -> i64 {
                match self {
                    $(Self::$variant => $value,)*
                    Self::Other(v) => v,
                }
            }

            /// 由整数取值构造。
            pub fn from_i64(value: i64) -> Self {
                match value {
                    $($value => Self::$variant,)*
                    v => Self::Other(v),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                i64::deserialize(d).map(Self::from_i64)
            }
        }
    };
}

int_enum!(
    /// 顶级类别（`classId`）。
    ClassId {
        /// 模组。
        Mods = 6,
        /// 资源包。
        ResourcePacks = 12,
        /// 存档。
        Worlds = 17,
        /// 整合包。
        Modpacks = 4471,
        /// 定制内容。
        Customization = 4546,
        /// 光影。
        Shaders = 6552,
        /// 数据包。
        DataPacks = 6945,
    }
);

int_enum!(
    /// 加载器（`modLoaderType`）。
    ModLoaderType {
        /// 不限。
        Any = 0,
        /// Forge。
        Forge = 1,
        /// Cauldron。
        Cauldron = 2,
        /// LiteLoader。
        LiteLoader = 3,
        /// Fabric。
        Fabric = 4,
        /// Quilt。
        Quilt = 5,
        /// NeoForge。
        NeoForge = 6,
    }
);

impl ModLoaderType {
    /// 与 Modrinth、`instance.json` 一致的小写名称；不是 TMCL 支持的加载器时为 `None`。
    pub fn as_loader_str(self) -> Option<&'static str> {
        match self {
            Self::Forge => Some("forge"),
            Self::Fabric => Some("fabric"),
            Self::Quilt => Some("quilt"),
            Self::NeoForge => Some("neoforge"),
            _ => None,
        }
    }

    /// 由小写加载器名称构造（`fabric` 等）。
    pub fn from_loader_str(name: &str) -> Option<Self> {
        match name {
            "forge" => Some(Self::Forge),
            "fabric" => Some(Self::Fabric),
            "quilt" => Some(Self::Quilt),
            "neoforge" => Some(Self::NeoForge),
            _ => None,
        }
    }

    /// 文件 `gameVersions` 中的加载器标签（`Fabric`、`NeoForge` 等，不分大小写）。
    pub fn from_label(label: &str) -> Option<Self> {
        Self::from_loader_str(&label.to_ascii_lowercase())
    }
}

int_enum!(
    /// 发布类型。
    ReleaseType {
        /// 正式版。
        Release = 1,
        /// 测试版。
        Beta = 2,
        /// 预览版。
        Alpha = 3,
    }
);

int_enum!(
    /// 哈希算法。
    HashAlgo {
        /// SHA1。
        Sha1 = 1,
        /// MD5。
        Md5 = 2,
    }
);

int_enum!(
    /// 依赖关系。
    RelationType {
        /// 内嵌库。
        EmbeddedLibrary = 1,
        /// 可选前置。
        OptionalDependency = 2,
        /// 必需前置。
        RequiredDependency = 3,
        /// 工具。
        Tool = 4,
        /// 不兼容。
        Incompatible = 5,
        /// 包含。
        Include = 6,
    }
);

int_enum!(
    /// 搜索排序字段（`sortField`）。
    SortField {
        /// 精选。
        Featured = 1,
        /// 热度。
        Popularity = 2,
        /// 最近更新。
        LastUpdated = 3,
        /// 名称。
        Name = 4,
        /// 作者。
        Author = 5,
        /// 总下载量。
        TotalDownloads = 6,
        /// 分类。
        Category = 7,
        /// 游戏版本。
        GameVersion = 8,
        /// 早期访问。
        EarlyAccess = 9,
        /// 精选发布。
        FeaturedReleased = 10,
        /// 发布时间。
        ReleasedDate = 11,
        /// 评分。
        Rating = 12,
    }
);
