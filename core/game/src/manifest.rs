//! 官方版本清单 `version_manifest_v2.json`。
//!
//! 缓存（SQLite，1 小时过期）由 M1-03 补充，目前每次调用都重新获取。

use serde::Deserialize;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::HttpClient;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
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
    fn missing_version_is_error() {
        let manifest: VersionManifest = serde_json::from_str(SAMPLE).unwrap();
        let err = manifest.find("9.9.9").unwrap_err();
        assert_eq!(err.code, ErrorCode::GameVersionNotFound);
        assert_eq!(err.params["version"], "9.9.9");
    }
}
