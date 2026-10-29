//! 整合包来源：在 Modrinth 搜索整合包、列出版本，把本地文件 / URL / Modrinth 版本准备成本地 `.mrpack`。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::{DownloadItem, Downloader};
use ts_rs::TS;

use crate::browse::{BrowseQuery, ResourcePage, ResourceVersion, is_not_found};
use crate::modrinth::{ModrinthClient, ProjectType, SearchQuery, VersionFilter};

/// 下载整合包文件的阶段名。
pub const FETCH_STAGE: &str = "modpack_fetch";

/// 默认每页条数。
const DEFAULT_LIMIT: u32 = 20;

/// 要安装的整合包从哪里来。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ModpackSource {
    /// 本地 `.mrpack` 文件的绝对路径。
    File {
        /// 文件路径。
        path: String,
    },
    /// `.mrpack` 的 http(s) 下载地址。
    Url {
        /// 下载地址。
        url: String,
    },
    /// Modrinth 整合包版本，来自 `resources_modpack_versions`。
    Modrinth {
        /// 版本 ID。
        version_id: String,
    },
}

/// 准备好的本地整合包文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedPack {
    /// 本地路径。
    pub path: PathBuf,
    /// 是否为下载到缓存目录的临时文件（安装结束后应删除）。
    pub temporary: bool,
}

impl FetchedPack {
    /// 删除临时文件；本地文件不动。
    pub fn cleanup(&self) {
        if self.temporary {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// 在 Modrinth 搜索整合包；`game_version` 给定时只返回支持该游戏版本的。
pub async fn search(
    client: &ModrinthClient,
    game_version: Option<&str>,
    query: &BrowseQuery,
    ctx: &TaskCtx,
) -> AppResult<ResourcePage> {
    let search = SearchQuery {
        query: query.query.trim().to_owned(),
        project_type: Some(ProjectType::Modpack),
        game_versions: game_version.map(ToOwned::to_owned).into_iter().collect(),
        loaders: Vec::new(),
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

/// 列出整合包项目的版本，新版本在前；没有文件的版本被跳过，项目不存在时为空列表。
pub async fn versions(
    client: &ModrinthClient,
    project_id: &str,
    game_version: Option<&str>,
    ctx: &TaskCtx,
) -> AppResult<Vec<ResourceVersion>> {
    let filter = VersionFilter {
        loaders: Vec::new(),
        game_versions: game_version.map(ToOwned::to_owned).into_iter().collect(),
        featured: None,
    };
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

/// 把来源准备成本地文件：本地文件原样使用；URL 与 Modrinth 版本下载到 `cache_dir/<任务 ID>.mrpack`。
///
/// 本地文件不存在返回 `COMMON_IO`；URL 不是 http(s) 返回 `RESOURCES_MODPACK_INVALID`；
/// Modrinth 版本不存在或没有文件返回 `RESOURCES_VERSION_NOT_FOUND`；下载失败返回 `NET_*`。
pub async fn fetch(
    source: &ModpackSource,
    client: &ModrinthClient,
    downloader: &Downloader,
    cache_dir: &Path,
    ctx: &TaskCtx,
) -> AppResult<FetchedPack> {
    let item = match source {
        ModpackSource::File { path } => {
            let path = PathBuf::from(path);
            if !path.is_file() {
                return Err(AppError::new(ErrorCode::CommonIo)
                    .with_param("path", path.display())
                    .with_detail("modpack file not found"));
            }
            return Ok(FetchedPack {
                path,
                temporary: false,
            });
        }
        ModpackSource::Url { url } => {
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err(AppError::new(ErrorCode::ResourcesModpackInvalid)
                    .with_detail("modpack url must be http(s)"));
            }
            DownloadItem {
                url: url.clone(),
                path: temp_path(cache_dir, ctx),
                size: None,
                sha1: None,
            }
        }
        ModpackSource::Modrinth { version_id } => {
            let not_found = || {
                AppError::new(ErrorCode::ResourcesVersionNotFound).with_param("version", version_id)
            };
            let version = client
                .version(version_id, ctx)
                .await?
                .ok_or_else(not_found)?;
            let file = version.primary_file().ok_or_else(not_found)?;
            // 不用服务器给的文件名，按任务 ID 命名，避免并发安装互相覆盖。
            DownloadItem {
                path: temp_path(cache_dir, ctx),
                ..file.download_item(cache_dir)
            }
        }
    };
    std::fs::create_dir_all(cache_dir).map_err(|e| crate::install::io_error(cache_dir, &e))?;
    let path = item.path.clone();
    let result = downloader.download_all(vec![item], FETCH_STAGE, ctx).await;
    let fetched = FetchedPack {
        path,
        temporary: true,
    };
    if let Err(err) = result {
        fetched.cleanup();
        return Err(err);
    }
    Ok(fetched)
}

fn temp_path(cache_dir: &Path, ctx: &TaskCtx) -> PathBuf {
    cache_dir.join(format!("{}.mrpack", ctx.id))
}
