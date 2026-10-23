//! 列出与游戏版本匹配的加载器版本，按新到旧排列。
//!
//! | 加载器 | 来源 |
//! | --- | --- |
//! | Fabric | `<fabric_meta>/v2/versions/loader/<游戏>`，带 `stable` 标记 |
//! | Quilt | `<quilt_meta>/v3/versions/loader/<游戏>`，版本号含 `-`（如 `-beta.9`）视为测试版 |
//! | Forge | maven 的 `maven-metadata.xml`，推荐版本来自 `promotions_slim.json` |
//! | NeoForge | maven 的 `maven-metadata.xml`；1.20.1 使用旧的 `net.neoforged:forge` 构件 |

use std::cmp::Ordering;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::Downloader;
use ts_rs::TS;

use crate::{Endpoints, LoaderKind, display_name};

/// Forge 支持的最早游戏版本。
pub const FORGE_MIN_GAME: &str = "1.7.10";

/// NeoForge 仍以 `net.neoforged:forge` 构件发布的游戏版本。
pub const NEOFORGE_LEGACY_GAME: &str = "1.20.1";

/// 一个可安装的加载器版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LoaderVersion {
    /// 加载器种类。
    pub kind: LoaderKind,
    /// 游戏版本。
    pub game_version: String,
    /// 加载器版本，即 `instance.json` 的 `loader.version`，例如 Forge 的 `47.2.0`。
    pub version: String,
    /// 是否为正式版（非 alpha/beta）。
    pub stable: bool,
    /// 是否为推荐版本：Forge 取官方推荐，其余取最新的正式版。
    pub recommended: bool,
}

/// 列出 `kind` 在 `game_version` 上可安装的版本，新版本在前。
///
/// Fabric、Quilt 不支持该游戏版本时返回空列表；Forge 不支持 1.7.10 以前的版本，返回
/// `LOADER_UNSUPPORTED_VERSION`。Forge 的推荐信息获取失败时不影响列表。
pub async fn list_versions(
    downloader: &Downloader,
    endpoints: &Endpoints,
    kind: LoaderKind,
    game_version: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<LoaderVersion>> {
    let mut versions = match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => {
            let url = match kind {
                LoaderKind::Fabric => format!(
                    "{}/v2/versions/loader/{}",
                    endpoints.fabric_meta,
                    segment(game_version)
                ),
                _ => format!(
                    "{}/v3/versions/loader/{}",
                    endpoints.quilt_meta,
                    segment(game_version)
                ),
            };
            meta_versions(downloader, &url, kind, game_version, ctx).await?
        }
        LoaderKind::Forge => forge(downloader, endpoints, game_version, ctx).await?,
        LoaderKind::Neoforge => neoforge(downloader, endpoints, game_version, ctx).await?,
    };
    if !versions.iter().any(|v| v.recommended)
        && let Some(first) = versions.iter_mut().find(|v| v.stable)
    {
        first.recommended = true;
    }
    Ok(versions)
}

#[derive(Deserialize)]
struct MetaEntry {
    loader: MetaLoader,
}

#[derive(Deserialize)]
struct MetaLoader {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

/// Fabric / Quilt 元数据。服务器对不认识的游戏版本返回 400/404 或空数组，都视为空列表。
async fn meta_versions(
    downloader: &Downloader,
    url: &str,
    kind: LoaderKind,
    game_version: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<LoaderVersion>> {
    let entries: Vec<MetaEntry> = match downloader.get_json(url, ctx).await {
        Ok(entries) => entries,
        Err(e) if is_missing(&e) => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut versions: Vec<LoaderVersion> = entries
        .into_iter()
        .map(|e| LoaderVersion {
            kind,
            game_version: game_version.to_owned(),
            stable: e
                .loader
                .stable
                .unwrap_or_else(|| !e.loader.version.contains('-')),
            version: e.loader.version,
            recommended: false,
        })
        .collect();
    // Quilt 的列表不保证新版本在前。
    versions.sort_by(|a, b| compare_versions(&b.version, &a.version));
    Ok(versions)
}

fn is_missing(err: &AppError) -> bool {
    err.code == ErrorCode::NetHttpStatus
        && matches!(
            err.params.get("status").map(String::as_str),
            Some("400" | "404")
        )
}

static MAVEN_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<version>\s*([^<\s]+)\s*</version>").expect("正则表达式有效"));

/// `maven-metadata.xml` 中的全部版本号。
pub fn parse_maven_metadata(xml: &str) -> Vec<String> {
    MAVEN_VERSION
        .captures_iter(xml)
        .map(|c| c[1].to_owned())
        .collect()
}

async fn maven_metadata(
    downloader: &Downloader,
    url: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<String>> {
    let bytes = downloader.get_bytes(url, ctx).await?;
    Ok(parse_maven_metadata(&String::from_utf8_lossy(&bytes)))
}

/// Forge 是否支持该游戏版本（1.7.10 起的正式版）。
pub fn forge_supports(game_version: &str) -> bool {
    numbers(game_version).is_some_and(|n| n >= numbers(FORGE_MIN_GAME).unwrap_or_default())
}

/// 纯数字点分版本号的各段，例如 `1.7.10` → `[1, 7, 10]`；含其他字符时为 `None`。
fn numbers(version: &str) -> Option<Vec<u64>> {
    version.split('.').map(|p| p.parse().ok()).collect()
}

fn unsupported(kind: LoaderKind, game_version: &str) -> AppError {
    AppError::new(ErrorCode::LoaderUnsupportedVersion)
        .with_param("kind", display_name(kind))
        .with_param("game", game_version)
}

/// Forge 在 `game_version` 上的版本，新版本在前：`(加载器版本, maven 版本)`。
///
/// maven 版本形如 `1.20.1-47.2.0`，1.7.10 等旧版本带分支后缀（`1.7.10-10.13.4.1614-1.7.10`），
/// 加载器版本去掉游戏版本前缀与分支后缀，与 `promotions_slim.json` 一致。
pub async fn forge_versions(
    downloader: &Downloader,
    endpoints: &Endpoints,
    game_version: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<(String, String)>> {
    if !forge_supports(game_version) {
        return Err(unsupported(LoaderKind::Forge, game_version));
    }
    let url = format!(
        "{}/net/minecraftforge/forge/maven-metadata.xml",
        endpoints.forge_maven
    );
    let mut out = forge_versions_from(&maven_metadata(downloader, &url, ctx).await?, game_version);
    out.sort_by(|a, b| compare_versions(&b.0, &a.0));
    Ok(out)
}

/// 从 maven 版本列表中挑出 `game_version` 的 Forge 版本：`(加载器版本, maven 版本)`。
pub fn forge_versions_from(maven: &[String], game_version: &str) -> Vec<(String, String)> {
    let prefix = format!("{game_version}-");
    let suffix = format!("-{game_version}");
    maven
        .iter()
        .filter_map(|mv| {
            let rest = mv.strip_prefix(&prefix)?;
            let short = rest.strip_suffix(&suffix).unwrap_or(rest);
            Some((short.to_owned(), mv.clone()))
        })
        .collect()
}

#[derive(Deserialize)]
struct Promotions {
    promos: std::collections::BTreeMap<String, String>,
}

async fn forge(
    downloader: &Downloader,
    endpoints: &Endpoints,
    game_version: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<LoaderVersion>> {
    let versions = forge_versions(downloader, endpoints, game_version, ctx).await?;
    let url = format!(
        "{}/net/minecraftforge/forge/promotions_slim.json",
        endpoints.forge_files
    );
    let recommended = match downloader.get_json::<Promotions>(&url, ctx).await {
        Ok(p) => p
            .promos
            .get(&format!("{game_version}-recommended"))
            .or_else(|| p.promos.get(&format!("{game_version}-latest")))
            .cloned(),
        Err(e) if e.is_cancelled() => return Err(e),
        Err(e) => {
            tracing::warn!(error = %e, "获取 Forge 推荐版本失败");
            None
        }
    };
    Ok(versions
        .into_iter()
        .map(|(version, _)| LoaderVersion {
            kind: LoaderKind::Forge,
            game_version: game_version.to_owned(),
            recommended: recommended.as_deref() == Some(version.as_str()),
            stable: true,
            version,
        })
        .collect())
}

/// NeoForge 版本对应的游戏版本。
///
/// 旧编号 `<次版本>.<修订>.<构建>` 对应 `1.<次版本>[.<修订>]`（`21.1.72` → `1.21.1`，`21.0.5` → `1.21`）；
/// 26 起按年份编号，`<年>.<序号>.<修订>.<构建>` 对应 `<年>.<序号>[.<修订>]`。快照构建（含 `+`）返回 `None`。
pub fn neoforge_game(version: &str) -> Option<String> {
    if version.contains('+') {
        return None;
    }
    let base = version.split('-').next()?;
    let n = numbers(base)?;
    match n.as_slice() {
        [major, minor, patch, _] if *major >= 26 => Some(if *patch == 0 {
            format!("{major}.{minor}")
        } else {
            format!("{major}.{minor}.{patch}")
        }),
        [major, minor, _] if *major < 26 => Some(if *minor == 0 {
            format!("1.{major}")
        } else {
            format!("1.{major}.{minor}")
        }),
        _ => None,
    }
}

async fn neoforge(
    downloader: &Downloader,
    endpoints: &Endpoints,
    game_version: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<LoaderVersion>> {
    let mut versions: Vec<String> = if game_version == NEOFORGE_LEGACY_GAME {
        let url = format!(
            "{}/net/neoforged/forge/maven-metadata.xml",
            endpoints.neoforge_maven
        );
        forge_versions_from(&maven_metadata(downloader, &url, ctx).await?, game_version)
            .into_iter()
            .map(|(short, _)| short)
            .collect()
    } else {
        let url = format!(
            "{}/net/neoforged/neoforge/maven-metadata.xml",
            endpoints.neoforge_maven
        );
        maven_metadata(downloader, &url, ctx)
            .await?
            .into_iter()
            .filter(|v| neoforge_game(v).as_deref() == Some(game_version))
            .collect()
    };
    versions.sort_by(|a, b| compare_versions(b, a));
    Ok(versions
        .into_iter()
        .map(|version| LoaderVersion {
            kind: LoaderKind::Neoforge,
            game_version: game_version.to_owned(),
            stable: !version.contains('-'),
            recommended: false,
            version,
        })
        .collect())
}

/// 比较两个加载器版本号：逐段比较其中的数字，数字相同时不带 `-` 后缀（正式版）的较新。
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let digits = |s: &str| -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse().unwrap_or(u64::MAX))
            .collect()
    };
    let (base_a, pre_a) = a.split_once('-').map_or((a, false), |(b, _)| (b, true));
    let (base_b, pre_b) = b.split_once('-').map_or((b, false), |(b, _)| (b, true));
    digits(base_a)
        .cmp(&digits(base_b))
        .then(pre_b.cmp(&pre_a))
        .then_with(|| digits(a).cmp(&digits(b)))
}

/// URL 路径片段：版本号中的 `+`（如 `0.7.2+build.175`）需要转义。
pub(crate) fn segment(s: &str) -> String {
    s.replace('+', "%2B")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_support_starts_at_1_7_10() {
        for game in ["1.7.10", "1.8", "1.12.2", "1.20.1", "26.3"] {
            assert!(forge_supports(game), "{game}");
        }
        for game in ["1.7.2", "1.6.4", "b1.7.3", "23w31a", "1.20.1-pre1"] {
            assert!(!forge_supports(game), "{game}");
        }
    }

    #[test]
    fn forge_short_versions_drop_game_prefix_and_branch_suffix() {
        let maven: Vec<String> = [
            "1.20.1-47.2.0",
            "1.20.10-99.0.0",
            "1.7.10-10.13.4.1614-1.7.10",
            "1.7.10_pre4-10.12.2.1149-prerelease",
            "1.7.10-10.13.0.1150",
        ]
        .map(str::to_owned)
        .to_vec();
        assert_eq!(
            forge_versions_from(&maven, "1.20.1"),
            [("47.2.0".to_owned(), "1.20.1-47.2.0".to_owned())]
        );
        assert_eq!(
            forge_versions_from(&maven, "1.7.10"),
            [
                (
                    "10.13.4.1614".to_owned(),
                    "1.7.10-10.13.4.1614-1.7.10".to_owned()
                ),
                ("10.13.0.1150".to_owned(), "1.7.10-10.13.0.1150".to_owned()),
            ]
        );
    }

    #[test]
    fn neoforge_versions_map_to_game_versions() {
        let cases = [
            ("20.2.3-beta", Some("1.20.2")),
            ("20.4.80", Some("1.20.4")),
            ("21.0.5-beta", Some("1.21")),
            ("21.1.209", Some("1.21.1")),
            ("26.1.0.12", Some("26.1")),
            ("26.3.1.4", Some("26.3.1")),
            ("26.1.0.0-alpha.1+snapshot-1", None),
            ("abc", None),
        ];
        for (version, game) in cases {
            assert_eq!(neoforge_game(version).as_deref(), game, "{version}");
        }
    }

    #[test]
    fn versions_compare_numerically_and_releases_win() {
        assert_eq!(compare_versions("47.10.0", "47.9.1"), Ordering::Greater);
        assert_eq!(
            compare_versions("14.23.5.2860", "14.23.5.2859"),
            Ordering::Greater
        );
        assert_eq!(
            compare_versions("0.20.0", "0.20.0-beta.9"),
            Ordering::Greater
        );
        assert_eq!(
            compare_versions("0.20.0-beta.10", "0.20.0-beta.9"),
            Ordering::Greater
        );
        assert_eq!(compare_versions("21.1.1", "21.1.1"), Ordering::Equal);
    }

    #[test]
    fn maven_metadata_versions_are_extracted() {
        let xml = "<metadata><versioning><latest>2</latest><versions>\n\
                   <version>1.20.1-47.1.7</version>\n  <version> 1.20.1-47.1.5 </version>\n\
                   </versions></versioning></metadata>";
        assert_eq!(
            parse_maven_metadata(xml),
            ["1.20.1-47.1.7", "1.20.1-47.1.5"]
        );
    }
}
