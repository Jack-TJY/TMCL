//! Modrinth API v2（<https://docs.modrinth.com/api/>）。
//!
//! [`ModrinthClient`] 负责请求、`User-Agent`、速率限制与重试，[`models`] 是响应结构。
//! 文件本身从 CDN 下载，交给 `tmcl_net::Downloader`，见 [`VersionFile::download_item`]。

pub mod client;
pub mod models;

pub use client::{API_BASE, ModrinthClient, SearchQuery, USER_AGENT, VersionFilter};
pub use models::{
    Dependency, DependencyType, GalleryImage, HashAlgorithm, License, Project, ProjectDependencies,
    ProjectType, SearchHit, SearchIndex, SearchResponse, SideSupport, Version, VersionFile,
    VersionFileHashes, VersionType,
};
