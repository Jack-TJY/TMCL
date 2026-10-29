//! CurseForge API v1（<https://docs.curseforge.com/rest-api/>）。
//!
//! API Key 在构建期由 `build.rs` 注入；没有 key 的构建中 [`is_available`] 为 `false`，
//! CurseForge 来源应整体隐藏。[`models`] 是响应结构，[`fingerprint`] 计算按文件反查所需的指纹。
//!
//! 条款要点：API 数据不缓存到磁盘；从 `forgecdn.net` 下载文件也要带 `x-api-key`
//! （见 [`with_cdn_key`]）；作者禁止第三方分发（`downloadUrl` 为空）的文件只能打开网页让用户手动下载。

pub mod fingerprint;
pub mod models;

use tmcl_common::AppResult;
use tmcl_net::HttpClient;

pub use models::{
    Asset, Author, Category, ClassId, File, FileDependency, FileHash, FileIndex, FingerprintMatch,
    FingerprintMatches, HashAlgo, MINECRAFT_GAME_ID, Mod, ModLinks, ModLoaderType, Page,
    Pagination, RelationType, ReleaseType, SortField,
};

/// CurseForge 文件 CDN 的域名（`edge.forgecdn.net`、`mediafilez.forgecdn.net`）。
pub const CDN_HOST: &str = "forgecdn.net";

/// 构建期注入的 key（空文件表示没有）。
const BUILD_KEY: &str = include_str!(concat!(env!("OUT_DIR"), "/curseforge_api_key"));

pub(crate) fn api_key() -> Option<&'static str> {
    let key = BUILD_KEY.trim();
    (!key.is_empty()).then_some(key)
}

/// 本次构建是否带有 CurseForge API Key；为 `false` 时界面应隐藏 CurseForge 来源。
pub fn is_available() -> bool {
    api_key().is_some()
}

/// 返回一个对 [`CDN_HOST`] 的请求附加 `x-api-key` 的 `http` 克隆；没有 key 时原样克隆。
///
/// 下载引擎应使用它创建，CurseForge 文件才能下载。
pub fn with_cdn_key(http: &HttpClient) -> AppResult<HttpClient> {
    match api_key() {
        Some(key) => http.with_host_header(CDN_HOST, "x-api-key", key),
        None => Ok(http.clone()),
    }
}
