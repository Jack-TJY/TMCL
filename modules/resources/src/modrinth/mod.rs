//! Modrinth API v2（<https://docs.modrinth.com/api/>）。
//!
//! [`models`] 是响应结构。文件本身从 CDN 下载，交给 `tmcl_net::Downloader`，
//! 见 [`VersionFile::download_item`]。

pub mod models;

pub use models::{
    Dependency, DependencyType, GalleryImage, License, Project, ProjectDependencies, ProjectType,
    SearchHit, SearchIndex, SearchResponse, SideSupport, Version, VersionFile, VersionFileHashes,
    VersionType,
};
