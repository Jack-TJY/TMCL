//! 启动器设置 `config.json`（PLAN.md 第 3.5 节）。
//!
//! - 读取：文件不存在时用默认值；JSON 损坏或 `schema` 不受支持时，把原文件
//!   备份为 `config.json.corrupt-<unix 毫秒>` 后回退默认值。
//! - 写入：原子写（先写临时文件再 rename）。
//! - 结构变化时提升 [`CONFIG_SCHEMA`]，并在 [`migrate`] 中追加迁移步骤。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::paths::write_atomic;

/// 当前程序写出的 `config.json` 结构版本。
pub const CONFIG_SCHEMA: u32 = 2;

/// 界面主题偏好。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// 跟随系统。
    #[default]
    System,
    /// 亮色。
    Light,
    /// 暗色。
    Dark,
}

/// 下载相关设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct DownloadConfig {
    /// 同时进行的下载数。
    pub max_concurrency: u32,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 16,
        }
    }
}

/// 启动器设置。缺失的字段取默认值，未知字段被忽略。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct Config {
    /// 结构版本，见 [`CONFIG_SCHEMA`]。
    pub schema: u32,
    /// 界面语言，BCP 47 标签，例如 `zh-CN`、`en-US`。
    pub language: String,
    /// 界面主题。
    pub theme: ThemeMode,
    /// 下载设置。
    pub download: DownloadConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema: CONFIG_SCHEMA,
            language: "zh-CN".to_owned(),
            theme: ThemeMode::default(),
            download: DownloadConfig::default(),
        }
    }
}

/// [`Config::load`] 的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    /// 读取到的（或回退的默认）配置。
    pub config: Config,
    /// 原文件无法使用时的备份位置；界面可据此提示用户。
    pub backup: Option<PathBuf>,
}

impl Config {
    /// 读取配置，必要时迁移旧版本；损坏时备份原文件并回退默认值。
    ///
    /// 只有读取或备份本身失败（例如没有权限）才返回错误。
    pub fn load(path: &Path) -> AppResult<LoadedConfig> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Ok(LoadedConfig {
                    config: Self::default(),
                    backup: None,
                });
            }
            Err(e) => {
                return Err(AppError::from(e).with_param("path", path.display()));
            }
        };

        match Self::parse(&bytes) {
            Ok(config) => Ok(LoadedConfig {
                config,
                backup: None,
            }),
            Err(err) => {
                let backup = backup_path(path);
                fs::rename(path, &backup)
                    .map_err(|e| AppError::from(e).with_param("path", path.display()))?;
                tracing::warn!(
                    code = %err.code,
                    detail = err.detail.as_deref().unwrap_or_default(),
                    backup = %backup.display(),
                    "config.json 无法使用，已备份并回退默认值"
                );
                Ok(LoadedConfig {
                    config: Self::default(),
                    backup: Some(backup),
                })
            }
        }
    }

    /// 从 JSON 字节解析并迁移到当前 schema。
    pub fn parse(bytes: &[u8]) -> AppResult<Self> {
        let value: Value = serde_json::from_slice(bytes)?;
        let value = migrate(value)?;
        Ok(serde_json::from_value(value)?)
    }

    /// 原子写入配置，`schema` 总是写为 [`CONFIG_SCHEMA`]。
    pub fn save(&self, path: &Path) -> AppResult<()> {
        let mut config = self.clone();
        config.schema = CONFIG_SCHEMA;
        let mut json = serde_json::to_vec_pretty(&config)?;
        json.push(b'\n');
        write_atomic(path, &json).map_err(|e| e.with_param("path", path.display()))
    }
}

/// 把任意版本的配置 JSON 迁移到 [`CONFIG_SCHEMA`]。
///
/// 每次提升 schema 时，在循环中追加 `n => n + 1` 的迁移分支并补测试。
pub fn migrate(mut value: Value) -> AppResult<Value> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::new(ErrorCode::CommonJson).with_detail("config 不是 JSON 对象"))?;
    let schema = match object.get("schema") {
        None => CONFIG_SCHEMA,
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| AppError::new(ErrorCode::CommonJson).with_detail("schema 不是整数"))?,
    };
    if schema == 0 || schema > CONFIG_SCHEMA {
        return Err(AppError::new(ErrorCode::CommonConfigSchemaUnsupported)
            .with_param("schema", schema)
            .with_param("supported", CONFIG_SCHEMA));
    }
    let mut schema = schema;
    if schema == 1 {
        migrate_1_to_2(&mut value);
        schema = 2;
    }
    debug_assert_eq!(schema, CONFIG_SCHEMA);
    if let Some(object) = value.as_object_mut() {
        object.insert("schema".to_owned(), Value::from(CONFIG_SCHEMA));
    }
    Ok(value)
}

/// schema 1 → 2：界面语言字段 `locale` 改名为 `language`。
fn migrate_1_to_2(value: &mut Value) {
    if let Some(object) = value.as_object_mut()
        && let Some(locale) = object.remove("locale")
    {
        object.entry("language").or_insert(locale);
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".corrupt-{millis}"));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_default() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = Config::load(&dir.path().join("config.json")).unwrap();
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.backup, None);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let config = Config {
            language: "en-US".to_owned(),
            theme: ThemeMode::Dark,
            download: DownloadConfig { max_concurrency: 4 },
            ..Config::default()
        };
        config.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.config, config);
        assert_eq!(loaded.backup, None);
    }

    #[test]
    fn corrupt_file_is_backed_up_and_defaults_used() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, b"{ not json").unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.config, Config::default());
        let backup = loaded.backup.expect("应当产生备份");
        assert_eq!(fs::read(&backup).unwrap(), b"{ not json");
        assert!(!path.exists(), "原文件应被移走");
        assert!(
            backup
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("config.json.corrupt-")
        );
    }

    #[test]
    fn newer_schema_is_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, br#"{"schema": 99, "language": "en-US"}"#).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.config, Config::default());
        assert!(loaded.backup.is_some());
    }

    #[test]
    fn unsupported_schema_error_code() {
        let err = Config::parse(br#"{"schema": 3}"#).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonConfigSchemaUnsupported);
        assert_eq!(err.params.get("schema").map(String::as_str), Some("3"));
    }

    #[test]
    fn partial_file_fills_defaults_and_ignores_unknown() {
        let config = Config::parse(br#"{"schema": 2, "theme": "light", "future": true}"#).unwrap();
        assert_eq!(config.theme, ThemeMode::Light);
        assert_eq!(config.language, "zh-CN");
        assert_eq!(config.download.max_concurrency, 16);
    }

    #[test]
    fn wrong_field_type_is_treated_as_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            br#"{"schema": 1, "download": {"max_concurrency": "many"}}"#,
        )
        .unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.config, Config::default());
        assert!(loaded.backup.is_some());
    }

    #[test]
    fn schema_1_locale_is_renamed_to_language() {
        let config = Config::parse(br#"{"schema": 1, "locale": "en-US"}"#).unwrap();
        assert_eq!(config.language, "en-US");
        let value = migrate(serde_json::json!({"schema": 1, "locale": "en-US"})).unwrap();
        assert_eq!(value, serde_json::json!({"schema": 2, "language": "en-US"}));
    }

    #[test]
    fn saved_json_has_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        Config::default().save(&path).unwrap();
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["schema"], CONFIG_SCHEMA);
    }
}
