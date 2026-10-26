//! 同步类别与“主配置”。
//!
//! 主配置保存在 `<data_root>/sync/master.json`，选项按 1.13 以后的格式规范化存储：
//! 按键为键名、语言代码为小写，应用到目标实例时再按目标版本转换。

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, DataPaths, ErrorCode, write_atomic};
use ts_rs::TS;

use crate::keymap::{self, KeyFormat};
use crate::options::OptionsFile;

/// `master.json` 的当前版本。
pub const MASTER_SCHEMA: u32 = 1;
/// 主配置最多包含的选项数。
pub const MAX_OPTIONS: usize = 4096;

/// 同步类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename = "SyncCategory")]
pub enum Category {
    /// 按键绑定（`key_*`）。
    Keys,
    /// 视频设置。
    Video,
    /// 音频设置（`soundCategory_*` 等）。
    Audio,
    /// 语言与字体。
    Language,
    /// 其余选项。
    Other,
}

/// 视频类选项（含新旧版本的不同写法）。
const VIDEO_KEYS: &[&str] = &[
    "anaglyph3d",
    "ao",
    "attackIndicator",
    "biomeBlendRadius",
    "bobView",
    "darknessEffectScale",
    "enableVsync",
    "entityDistanceScaling",
    "entityShadows",
    "fancyGraphics",
    "fov",
    "fovEffectScale",
    "fullscreen",
    "fullscreenResolution",
    "gamma",
    "glintSpeed",
    "glintStrength",
    "graphicsMode",
    "guiScale",
    "inactivityFpsLimit",
    "maxFps",
    "menuBackgroundBlurriness",
    "mipmapLevels",
    "particles",
    "prioritizeChunkUpdates",
    "renderClouds",
    "renderDistance",
    "screenEffectScale",
    "simulationDistance",
    "useVbo",
];
/// 音频类选项（另含所有 `soundCategory_*`）。
const AUDIO_KEYS: &[&str] = &["directionalAudio", "showSubtitles", "soundDevice"];
/// 语言类选项。
const LANGUAGE_KEYS: &[&str] = &["forceUnicodeFont", "lang"];
/// 从不同步的选项：数据版本号与依赖实例内容的选项。
const EXCLUDED_KEYS: &[&str] = &[
    "incompatibleResourcePacks",
    "lastServer",
    "resourcePacks",
    "version",
];

/// 选项所属的类别；不参与同步的选项返回 `None`。
pub fn category_of(key: &str) -> Option<Category> {
    if EXCLUDED_KEYS.contains(&key) {
        None
    } else if key.starts_with("key_") {
        Some(Category::Keys)
    } else if key.starts_with("soundCategory_") || AUDIO_KEYS.contains(&key) {
        Some(Category::Audio)
    } else if VIDEO_KEYS.contains(&key) {
        Some(Category::Video)
    } else if LANGUAGE_KEYS.contains(&key) {
        Some(Category::Language)
    } else {
        Some(Category::Other)
    }
}

/// 一条选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SyncOption {
    /// `options.txt` 中的键。
    pub key: String,
    /// 值（规范化格式）。
    pub value: String,
}

/// 主配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SyncProfile {
    /// 采集来源实例；手动编辑时为 `null`。
    pub source_instance: Option<String>,
    /// 来源实例的游戏版本。
    pub source_version: Option<String>,
    /// 最后修改时间，unix 毫秒。
    #[ts(type = "number")]
    pub updated_at: u64,
    /// 选项，保持来源文件中的顺序。
    pub options: Vec<SyncOption>,
}

#[derive(Serialize, Deserialize)]
struct MasterFile {
    schema: u32,
    #[serde(flatten)]
    profile: SyncProfile,
}

/// 主配置文件路径：`<data_root>/sync/master.json`。
pub fn master_file(paths: &DataPaths) -> PathBuf {
    paths.root().join("sync").join("master.json")
}

/// 把某个游戏版本的 `options.txt` 规范化为主配置的选项列表；不参与同步的选项被丢弃。
pub fn capture_options(file: &OptionsFile, game_version: &str) -> Vec<SyncOption> {
    let version_format = keymap::format_for_version(game_version);
    file.entries()
        .filter(|(key, _)| category_of(key).is_some())
        .map(|(key, value)| {
            let value = if key.starts_with("key_") {
                match version_format.or_else(|| keymap::detect_value_format(value)) {
                    Some(KeyFormat::Lwjgl2) => keymap::convert_value(value, KeyFormat::Named),
                    _ => value.to_owned(),
                }
            } else if key == "lang" {
                value.to_ascii_lowercase()
            } else {
                value.to_owned()
            };
            SyncOption {
                key: key.to_owned(),
                value,
            }
        })
        .collect()
}

/// 读取主配置；从未保存过时为 `None`。文件损坏或格式版本不受支持返回 `SYNC_MASTER_INVALID`。
pub fn load_master(paths: &DataPaths) -> AppResult<Option<SyncProfile>> {
    let path = master_file(paths);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppError::from(e).with_param("path", path.display())),
    };
    let file: MasterFile = serde_json::from_slice(&bytes).map_err(|e| {
        AppError::new(ErrorCode::SyncMasterInvalid)
            .with_param("path", path.display())
            .with_detail(e.to_string())
    })?;
    if file.schema != MASTER_SCHEMA {
        return Err(AppError::new(ErrorCode::SyncMasterInvalid)
            .with_param("path", path.display())
            .with_detail(format!("unsupported master.json schema {}", file.schema)));
    }
    Ok(Some(file.profile))
}

/// 校验并原子保存主配置。键为空或含 `:`、换行，值含换行，或选项过多时返回 `SYNC_MASTER_INVALID`。
pub fn save_master(paths: &DataPaths, profile: &SyncProfile) -> AppResult<()> {
    let bad_line = |s: &str| s.contains(['\r', '\n']);
    let valid = profile.options.len() <= MAX_OPTIONS
        && profile.options.iter().all(|o| {
            !o.key.is_empty() && !o.key.contains(':') && !bad_line(&o.key) && !bad_line(&o.value)
        });
    if !valid {
        return Err(AppError::new(ErrorCode::SyncMasterInvalid)
            .with_detail("sync master: invalid option key or value"));
    }
    let doc = MasterFile {
        schema: MASTER_SCHEMA,
        profile: profile.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&doc).map_err(AppError::from)?;
    let path = master_file(paths);
    write_atomic(&path, &bytes).map_err(|e| e.with_param("path", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories() {
        assert_eq!(category_of("key_key.jump"), Some(Category::Keys));
        assert_eq!(category_of("key_examplemod.key.dash"), Some(Category::Keys));
        assert_eq!(category_of("soundCategory_master"), Some(Category::Audio));
        assert_eq!(category_of("renderDistance"), Some(Category::Video));
        assert_eq!(category_of("lang"), Some(Category::Language));
        assert_eq!(category_of("mouseSensitivity"), Some(Category::Other));
        assert_eq!(category_of("version"), None);
        assert_eq!(category_of("resourcePacks"), None);
    }

    #[test]
    fn capture_normalizes_legacy_values() {
        let file = OptionsFile::parse(
            "version:1343\nlang:zh_CN\nkey_key.jump:57\nkey_key.attack:-100\nkey_mod.x:300\n",
        );
        let options = capture_options(&file, "1.12.2");
        let values: Vec<(&str, &str)> = options
            .iter()
            .map(|o| (o.key.as_str(), o.value.as_str()))
            .collect();
        assert_eq!(
            values,
            [
                ("lang", "zh_cn"),
                ("key_key.jump", "key.keyboard.space"),
                ("key_key.attack", "key.mouse.left"),
                ("key_mod.x", "300"),
            ]
        );
    }

    #[test]
    fn master_save_load_and_validation() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        assert_eq!(load_master(&paths).unwrap(), None);

        let mut profile = SyncProfile {
            source_instance: None,
            source_version: Some("1.20.1".into()),
            updated_at: 1,
            options: vec![SyncOption {
                key: "fov".into(),
                value: "0.5".into(),
            }],
        };
        save_master(&paths, &profile).unwrap();
        assert!(dir.path().join("sync/master.json").is_file());
        assert_eq!(load_master(&paths).unwrap(), Some(profile.clone()));

        profile.options[0].value = "1\nx:2".into();
        let err = save_master(&paths, &profile).unwrap_err();
        assert_eq!(err.code, ErrorCode::SyncMasterInvalid);

        fs::write(master_file(&paths), b"{ broken").unwrap();
        assert_eq!(
            load_master(&paths).unwrap_err().code,
            ErrorCode::SyncMasterInvalid
        );
    }
}
