//! 资源索引与资源文件，含 1.7.2 以前的 `virtual`（`legacy`）与 1.6 以前的
//! `map_to_resources`（`pre-1.6`）布局。
//!
//! 资源文件统一存放在 `shared/assets/objects/<hh>/<hash>`。旧版本的游戏按资源名读取文件，
//! 启动前用 [`prepare_legacy_assets`] 按名字复制一份：
//!
//! - `virtual`：复制到 `shared/assets/virtual/<索引 ID>/<名字>`，所有实例共用；
//! - `map_to_resources`：复制到 `<gameDir>/resources/<名字>`，每个实例一份。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use tmcl_common::{AppError, AppResult, ErrorCode};
use tmcl_net::DownloadItem;

use crate::install::GameFiles;

/// 资源索引文件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssetIndex {
    /// 资源名 → 对象。
    pub objects: BTreeMap<String, AssetObject>,
    /// 1.6–1.7.2 的 `legacy` 索引：按名字复制到 `assets/virtual/<索引 ID>/`。
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    /// 1.6 以前的 `pre-1.6` 索引：按名字复制到 `<gameDir>/resources/`。
    #[serde(default)]
    pub map_to_resources: bool,
}

/// 资源对象。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssetObject {
    /// SHA1，同时是存储文件名。
    pub hash: String,
    /// 大小。
    pub size: u64,
}

impl AssetIndex {
    /// 解析索引内容；格式错误时返回 `COMMON_JSON`。
    pub fn parse(bytes: &[u8]) -> AppResult<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }

    /// 读取 `shared/assets/indexes/<id>.json`。
    pub fn load(files: &GameFiles, id: &str) -> AppResult<Self> {
        let path = files.asset_index(id);
        let bytes = std::fs::read(&path).map_err(|e| io_error(&path, &e))?;
        Self::parse(&bytes)
    }
}

/// 资源索引中的对象，按哈希去重。
pub fn asset_items(
    files: &GameFiles,
    index: &AssetIndex,
    resources_url: &str,
) -> Vec<DownloadItem> {
    let mut seen = BTreeSet::new();
    index
        .objects
        .values()
        .filter(|o| seen.insert(o.hash.as_str()))
        .map(|o| DownloadItem {
            url: format!(
                "{}/{}/{}",
                resources_url.trim_end_matches('/'),
                o.hash.get(..2).unwrap_or(&o.hash),
                o.hash
            ),
            path: files.asset_object(&o.hash),
            size: Some(o.size),
            sha1: Some(o.hash.clone()),
        })
        .collect()
}

/// 启动参数 `${game_assets}` 指向的目录：`virtual` 索引为 `assets/virtual/<id>/`，
/// `map_to_resources` 索引为 `<gameDir>/resources/`，其他为 `assets/`。
pub fn game_assets_dir(
    files: &GameFiles,
    index_id: &str,
    index: &AssetIndex,
    game_dir: &Path,
) -> PathBuf {
    if index.map_to_resources {
        game_dir.join("resources")
    } else if index.is_virtual {
        files.virtual_assets_dir(index_id)
    } else {
        files.assets_dir()
    }
}

/// 旧版本启动前按资源名复制资源文件到 [`game_assets_dir`]，返回复制的文件数；
/// 新版本索引什么都不做。目标已存在且大小一致时跳过；会跳出目标目录的资源名被忽略。
///
/// 这是阻塞操作，异步代码中应放进 `spawn_blocking`。资源文件缺失时返回 `COMMON_IO`。
pub fn prepare_legacy_assets(
    files: &GameFiles,
    index_id: &str,
    index: &AssetIndex,
    game_dir: &Path,
) -> AppResult<usize> {
    if !index.is_virtual && !index.map_to_resources {
        return Ok(0);
    }
    let root = game_assets_dir(files, index_id, index, game_dir);
    let mut copied = 0;
    for (name, object) in &index.objects {
        let Some(target) = safe_join(&root, name) else {
            continue;
        };
        if std::fs::metadata(&target).is_ok_and(|m| m.is_file() && m.len() == object.size) {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_error(parent, &e))?;
        }
        let source = files.asset_object(&object.hash);
        std::fs::copy(&source, &target).map_err(|e| io_error(&source, &e))?;
        copied += 1;
    }
    Ok(copied)
}

/// 把 `/` 分隔的资源名拼到 `root` 下；含 `..`、绝对路径或盘符时返回 `None`。
fn safe_join(root: &Path, name: &str) -> Option<PathBuf> {
    let relative = Path::new(name);
    let normal = relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    (normal && !name.is_empty()).then(|| root.join(relative))
}

fn io_error(path: &Path, err: &std::io::Error) -> AppError {
    AppError::new(ErrorCode::CommonIo)
        .with_param("path", path.display())
        .with_detail(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_flags() {
        let index =
            AssetIndex::parse(br#"{"virtual":true,"objects":{"a.ogg":{"hash":"aa11","size":1}}}"#)
                .unwrap();
        assert!(index.is_virtual && !index.map_to_resources);
        let index = AssetIndex::parse(br#"{"map_to_resources":true,"objects":{}}"#).unwrap();
        assert!(index.map_to_resources);
        let index = AssetIndex::parse(br#"{"objects":{}}"#).unwrap();
        assert!(!index.is_virtual && !index.map_to_resources);
        assert_eq!(
            AssetIndex::parse(b"[]").unwrap_err().code,
            ErrorCode::CommonJson
        );
    }

    #[test]
    fn rejects_escaping_names() {
        let root = Path::new("root");
        assert_eq!(
            safe_join(root, "sound/a.ogg"),
            Some(root.join("sound/a.ogg"))
        );
        for bad in ["", "../x", "a/../../x", "/abs", "C:/x", "C:x"] {
            if cfg!(windows) || !bad.starts_with('C') {
                assert_eq!(safe_join(root, bad), None, "{bad}");
            }
        }
    }
}
