//! 新实例自动同步：设置保存在 `<data_root>/sync/auto.json`。
//!
//! 开启后，实例首次启动前（游戏目录中还没有 `options.txt`）把主配置的选中类别写入该实例；
//! 已有 `options.txt` 的实例（启动过的实例、自带设置的整合包）不受影响。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, DataPaths, write_atomic};
use ts_rs::TS;

use crate::apply::{self, Applied};
use crate::profile::{self, Category};

/// 自动同步设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "SyncAutoSettings")]
pub struct AutoSync {
    /// 是否开启。
    pub enabled: bool,
    /// 要同步的类别。
    pub categories: Vec<Category>,
}

impl Default for AutoSync {
    fn default() -> Self {
        Self {
            enabled: false,
            categories: vec![
                Category::Keys,
                Category::Video,
                Category::Audio,
                Category::Language,
            ],
        }
    }
}

/// 设置文件路径：`<data_root>/sync/auto.json`。
pub fn auto_file(paths: &DataPaths) -> PathBuf {
    paths.root().join("sync").join("auto.json")
}

/// 读取设置；文件不存在或损坏时返回默认值（关闭），损坏只记日志。
pub fn load(paths: &DataPaths) -> AutoSync {
    let path = auto_file(paths);
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "sync/auto.json 无法解析，按关闭处理");
            AutoSync::default()
        }),
        Err(e) => {
            if e.kind() != ErrorKind::NotFound {
                tracing::warn!(error = %e, "读取 sync/auto.json 失败，按关闭处理");
            }
            AutoSync::default()
        }
    }
}

/// 保存设置。
pub fn save(paths: &DataPaths, settings: &AutoSync) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(AppError::from)?;
    let path = auto_file(paths);
    write_atomic(&path, &bytes).map_err(|e| e.with_param("path", path.display()))
}

/// 首次启动前的自动同步：开启了自动同步、已有主配置、且 `game_dir` 中还没有 `options.txt` 时
/// 应用主配置，否则什么也不做并返回 `None`。`client_jar` 用于取数据版本。
pub fn apply_before_first_launch(
    paths: &DataPaths,
    game_dir: &Path,
    game_version: &str,
    client_jar: &Path,
) -> AppResult<Option<Applied>> {
    let settings = load(paths);
    if !settings.enabled || settings.categories.is_empty() {
        return Ok(None);
    }
    if game_dir.join(apply::OPTIONS_FILE).exists() {
        return Ok(None);
    }
    let Some(master) = profile::load_master(paths)? else {
        return Ok(None);
    };
    fs::create_dir_all(game_dir)
        .map_err(|e| AppError::from(e).with_param("path", game_dir.display()))?;
    apply::apply(
        game_dir,
        game_version,
        client_jar,
        &master,
        &settings.categories,
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{SyncOption, SyncProfile, save_master};

    fn master() -> SyncProfile {
        SyncProfile {
            source_instance: None,
            source_version: None,
            updated_at: 0,
            options: vec![
                SyncOption {
                    key: "key_key.jump".into(),
                    value: "key.keyboard.space".into(),
                },
                SyncOption {
                    key: "fov".into(),
                    value: "0.5".into(),
                },
            ],
        }
    }

    #[test]
    fn settings_round_trip_and_default_off() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        assert_eq!(load(&paths), AutoSync::default());
        assert!(!load(&paths).enabled);
        let on = AutoSync {
            enabled: true,
            categories: vec![Category::Keys],
        };
        save(&paths, &on).unwrap();
        assert_eq!(load(&paths), on);
        fs::write(auto_file(&paths), "{").unwrap();
        assert_eq!(load(&paths), AutoSync::default());
    }

    #[test]
    fn applies_only_before_first_launch() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        let game = dir.path().join("game");
        let jar = dir.path().join("missing.jar");
        save_master(&paths, &master()).unwrap();

        // 关闭时不写。
        assert_eq!(
            apply_before_first_launch(&paths, &game, "1.12.2", &jar).unwrap(),
            None
        );
        assert!(!game.join("options.txt").exists());

        save(
            &paths,
            &AutoSync {
                enabled: true,
                categories: vec![Category::Keys],
            },
        )
        .unwrap();
        let applied = apply_before_first_launch(&paths, &game, "1.12.2", &jar)
            .unwrap()
            .unwrap();
        assert_eq!(applied.changed, 1);
        let text = fs::read_to_string(game.join("options.txt")).unwrap();
        assert_eq!(text.trim(), "key_key.jump:57");

        // 已有 options.txt 时不再改动。
        fs::write(game.join("options.txt"), "key_key.jump:17\n").unwrap();
        assert_eq!(
            apply_before_first_launch(&paths, &game, "1.12.2", &jar).unwrap(),
            None
        );
        assert_eq!(
            fs::read_to_string(game.join("options.txt")).unwrap(),
            "key_key.jump:17\n"
        );
    }
}
