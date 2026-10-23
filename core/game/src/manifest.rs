//! 官方版本清单 `version_manifest_v2.json`，以及它在 SQLite `kv` 表中的缓存。
//!
//! 缓存 1 小时内直接使用；过期后重新获取，获取失败时退回过期的缓存，离线也能列出版本。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::OptionalExtension;
use serde::Deserialize;
use tmcl_common::{AppError, AppResult, Db, ErrorCode, TaskCtx};
use tmcl_net::{Downloader, HttpClient};

/// 官方版本清单地址。
pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// 版本清单。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VersionManifest {
    /// 最新正式版与快照。
    pub latest: Latest,
    /// 所有版本，新版本在前。
    pub versions: Vec<ManifestEntry>,
}

/// 最新版本号。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Latest {
    /// 最新正式版。
    pub release: String,
    /// 最新快照。
    pub snapshot: String,
}

/// 版本类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    /// 正式版。
    Release,
    /// 快照。
    Snapshot,
    /// Beta 版。
    OldBeta,
    /// Alpha 版。
    OldAlpha,
    /// 未知类型（向前兼容）。
    #[serde(other)]
    Other,
}

/// 清单中的一个版本。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    /// 版本号。
    pub id: String,
    /// 类型。
    #[serde(rename = "type")]
    pub kind: VersionType,
    /// 版本 JSON 地址。
    pub url: String,
    /// 版本 JSON 的 SHA1。
    pub sha1: String,
    /// 发布时间（RFC 3339）。
    pub release_time: String,
}

impl VersionManifest {
    /// 从地址获取清单。
    pub async fn fetch(client: &HttpClient, url: &str, ctx: &TaskCtx) -> AppResult<Self> {
        client.get_json(url, ctx).await
    }

    /// 按版本号查找；找不到时返回 `GAME_VERSION_NOT_FOUND`。
    pub fn find(&self, id: &str) -> AppResult<&ManifestEntry> {
        self.versions
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| AppError::new(ErrorCode::GameVersionNotFound).with_param("version", id))
    }

    /// 解析清单内容；格式错误时返回 `COMMON_JSON`。
    pub fn parse(bytes: &[u8]) -> AppResult<Self> {
        serde_json::from_slice(bytes).map_err(|e| {
            AppError::new(ErrorCode::CommonJson)
                .with_param("url", MANIFEST_URL)
                .with_detail(e.to_string())
        })
    }

    /// 某一类型的版本，保持清单中新版本在前的顺序。
    pub fn of_kind(&self, kind: VersionType) -> impl Iterator<Item = &ManifestEntry> {
        self.versions.iter().filter(move |v| v.kind == kind)
    }
}

/// 版本清单缓存的有效期。
pub const MANIFEST_MAX_AGE: Duration = Duration::from_secs(60 * 60);

/// 缓存在 `kv` 表中的键前缀，后接清单地址。
const CACHE_KEY_PREFIX: &str = "game.version_manifest:";

/// 版本清单缓存的存取。[`Db`] 已实现；应用层可包一层共享连接。
pub trait ManifestStore {
    /// 读取缓存的清单原文与写入时间（unix 秒）。
    fn load_manifest(&self, url: &str) -> AppResult<Option<(String, i64)>>;
    /// 写入（覆盖）缓存。
    fn save_manifest(&self, url: &str, body: &str) -> AppResult<()>;
}

impl ManifestStore for Db {
    fn load_manifest(&self, url: &str) -> AppResult<Option<(String, i64)>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT value, updated_at FROM kv WHERE key = ?1",
                [format!("{CACHE_KEY_PREFIX}{url}")],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    fn save_manifest(&self, url: &str, body: &str) -> AppResult<()> {
        self.kv_set(&format!("{CACHE_KEY_PREFIX}{url}"), body)
    }
}

/// 读取清单的方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestPolicy {
    /// 缓存不超过这个时长时直接使用。
    pub max_age: Duration,
    /// 忽略未过期的缓存，强制重新获取（获取失败仍会退回缓存）。
    pub refresh: bool,
}

impl Default for ManifestPolicy {
    fn default() -> Self {
        Self {
            max_age: MANIFEST_MAX_AGE,
            refresh: false,
        }
    }
}

/// 按 `policy` 读取版本清单：缓存未过期时直接返回，否则经 `downloader`（按下载源回退）获取并写回缓存；
/// 获取失败但有可用的旧缓存时返回旧缓存，否则返回获取时的错误。
///
/// 缓存读写失败只记日志，不影响结果。
pub async fn load_manifest(
    downloader: &Downloader,
    store: &impl ManifestStore,
    url: &str,
    policy: ManifestPolicy,
    ctx: &TaskCtx,
) -> AppResult<VersionManifest> {
    let cached = store
        .load_manifest(url)
        .inspect_err(|e| tracing::warn!(error = %e, "读取版本清单缓存失败"))
        .ok()
        .flatten()
        .and_then(|(body, at)| Some((VersionManifest::parse(body.as_bytes()).ok()?, at)));
    if let Some((manifest, at)) = &cached {
        let age = unix_now().saturating_sub(*at);
        if !policy.refresh && u64::try_from(age).is_ok_and(|age| age < policy.max_age.as_secs()) {
            return Ok(manifest.clone());
        }
    }
    let fetched = async {
        let bytes = downloader.get_bytes(url, ctx).await?;
        let manifest = VersionManifest::parse(&bytes)?;
        Ok::<_, AppError>((manifest, bytes))
    }
    .await;
    match (fetched, cached) {
        (Ok((manifest, bytes)), _) => {
            let body = String::from_utf8_lossy(&bytes);
            if let Err(e) = store.save_manifest(url, &body) {
                tracing::warn!(error = %e, "写入版本清单缓存失败");
            }
            Ok(manifest)
        }
        (Err(e), Some((manifest, _))) if !e.is_cancelled() => {
            tracing::warn!(error = %e, "获取版本清单失败，使用过期的缓存");
            Ok(manifest)
        }
        (Err(e), _) => Err(e),
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "latest": {"release": "1.20.1", "snapshot": "23w31a"},
        "versions": [
            {"id": "23w31a", "type": "snapshot", "url": "https://example.invalid/23w31a.json",
             "time": "2023-08-01T00:00:00+00:00", "releaseTime": "2023-08-01T00:00:00+00:00",
             "sha1": "1111111111111111111111111111111111111111", "complianceLevel": 1},
            {"id": "1.20.1", "type": "release", "url": "https://example.invalid/1.20.1.json",
             "time": "2023-06-12T13:25:51+00:00", "releaseTime": "2023-06-12T13:25:51+00:00",
             "sha1": "38d412af3b247e485c7f7e50ac25febc4e79ab1d", "complianceLevel": 1},
            {"id": "b1.7.3", "type": "old_beta", "url": "https://example.invalid/b1.7.3.json",
             "time": "2011-07-07T22:00:00+00:00", "releaseTime": "2011-07-07T22:00:00+00:00",
             "sha1": "2222222222222222222222222222222222222222", "complianceLevel": 0},
            {"id": "x", "type": "pending", "url": "https://example.invalid/x.json",
             "time": "", "releaseTime": "", "sha1": "3333333333333333333333333333333333333333"}
        ]
    }"#;

    #[test]
    fn parses_and_finds_versions() {
        let manifest: VersionManifest = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(manifest.latest.release, "1.20.1");
        let entry = manifest.find("1.20.1").unwrap();
        assert_eq!(entry.kind, VersionType::Release);
        assert_eq!(entry.sha1, "38d412af3b247e485c7f7e50ac25febc4e79ab1d");
        assert_eq!(manifest.find("b1.7.3").unwrap().kind, VersionType::OldBeta);
        assert_eq!(manifest.find("x").unwrap().kind, VersionType::Other);
    }

    #[test]
    fn groups_by_kind() {
        let manifest = VersionManifest::parse(SAMPLE.as_bytes()).unwrap();
        let ids = |kind| {
            manifest
                .of_kind(kind)
                .map(|v| v.id.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(VersionType::Release), ["1.20.1"]);
        assert_eq!(ids(VersionType::Snapshot), ["23w31a"]);
        assert_eq!(ids(VersionType::OldBeta), ["b1.7.3"]);
        assert!(ids(VersionType::OldAlpha).is_empty());
        assert_eq!(
            VersionManifest::parse(b"{}").unwrap_err().code,
            ErrorCode::CommonJson
        );
    }

    #[test]
    fn missing_version_is_error() {
        let manifest: VersionManifest = serde_json::from_str(SAMPLE).unwrap();
        let err = manifest.find("9.9.9").unwrap_err();
        assert_eq!(err.code, ErrorCode::GameVersionNotFound);
        assert_eq!(err.params["version"], "9.9.9");
    }
}
