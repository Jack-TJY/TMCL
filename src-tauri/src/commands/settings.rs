//! `settings_*` 命令：读写 `config.json` 中的启动器设置。
//!
//! 界面语言单独由 `settings_*_language` 管理（启动时最先需要）；其余设置经
//! `settings_get` / `settings_set` 整体读写。

use std::ops::RangeInclusive;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;
use tmcl_common::config::{AppearanceConfig, DownloadConfig, MemoryConfig, ThemeMode};
use tmcl_common::{AppError, AppResult, Config, DataPaths, ErrorCode};
use ts_rs::TS;

use crate::state::AppState;

/// 同时下载数的上限。
pub const MAX_CONCURRENCY: u32 = 64;
/// 默认内存的取值范围（MiB）。
pub const MEMORY_RANGE_MB: RangeInclusive<u32> = 512..=32768;

/// 设置页编辑的启动器设置：`config.json` 中除 `schema`、`language` 外的全部字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LauncherSettings {
    /// 界面主题。
    pub theme: ThemeMode,
    /// 外观。
    pub appearance: AppearanceConfig,
    /// 新建实例的默认内存。
    pub memory: MemoryConfig,
    /// 下载设置。
    pub download: DownloadConfig,
}

impl LauncherSettings {
    fn from_config(config: &Config) -> Self {
        Self {
            theme: config.theme,
            appearance: config.appearance.clone(),
            memory: config.memory,
            download: config.download.clone(),
        }
    }

    fn apply_to(self, config: &mut Config) {
        config.theme = self.theme;
        config.appearance = self.appearance;
        config.memory = self.memory;
        config.download = self.download;
    }
}

/// `settings_get` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SettingsSnapshot {
    /// 当前设置。
    pub settings: LauncherSettings,
    /// 数据目录的绝对路径（只读显示）。
    pub data_dir: String,
    /// 本次读取时发现 `config.json` 损坏、已备份到此路径并回退默认值；否则为 `null`。
    pub config_backup: Option<String>,
}

/// `settings_set` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SettingsSetArgs {
    /// 新的完整设置。
    pub settings: LauncherSettings,
}

/// 读取启动器设置与数据目录。
#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<SettingsSnapshot, AppError> {
    let _guard = state.config_lock.lock().await;
    get_settings(state.data()?)
}

/// 保存启动器设置（界面语言不变），返回保存后的设置；下载设置变化时重建下载引擎。
#[tauri::command]
pub async fn settings_set(
    state: State<'_, AppState>,
    args: SettingsSetArgs,
) -> Result<LauncherSettings, AppError> {
    validate_settings(&args.settings)?;
    let _guard = state.config_lock.lock().await;
    let (saved, download_changed) = set_settings(state.data()?, args.settings)?;
    if download_changed {
        state.reload_downloader(&saved.download);
    }
    Ok(saved)
}

/// 在系统文件管理器中打开数据目录。
#[tauri::command]
pub async fn settings_open_data_dir(state: State<'_, AppState>) -> Result<(), AppError> {
    let paths = state.data()?;
    paths.ensure_dirs()?;
    open_in_file_manager(paths.root())
}

fn get_settings(paths: &DataPaths) -> AppResult<SettingsSnapshot> {
    let loaded = Config::load(&paths.config_file())?;
    Ok(SettingsSnapshot {
        settings: LauncherSettings::from_config(&loaded.config),
        data_dir: paths.root().display().to_string(),
        config_backup: loaded.backup.map(|p| p.display().to_string()),
    })
}

/// 写入设置，返回保存后的设置以及下载设置是否变化。
fn set_settings(
    paths: &DataPaths,
    settings: LauncherSettings,
) -> AppResult<(LauncherSettings, bool)> {
    let path = paths.config_file();
    let mut config = Config::load(&path)?.config;
    let download_changed = config.download != settings.download;
    settings.apply_to(&mut config);
    config.save(&path)?;
    Ok((LauncherSettings::from_config(&config), download_changed))
}

fn validate_settings(settings: &LauncherSettings) -> AppResult<()> {
    let accent_ok = settings.appearance.accent.as_deref().is_none_or(|accent| {
        accent.len() == 7
            && accent.starts_with('#')
            && accent[1..].chars().all(|c| c.is_ascii_hexdigit())
    });
    let concurrency_ok = (1..=MAX_CONCURRENCY).contains(&settings.download.max_concurrency);
    let memory_ok = MEMORY_RANGE_MB.contains(&settings.memory.max_mb);
    if !(accent_ok && concurrency_ok && memory_ok) {
        return Err(AppError::new(ErrorCode::CommonInternal)
            .with_param("accent", accent_ok)
            .with_param("max_concurrency", concurrency_ok)
            .with_param("max_mb", memory_ok)
            .with_detail("settings_set: value out of range"));
    }
    Ok(())
}

/// 在系统文件管理器中打开目录。
pub(crate) fn open_in_file_manager(dir: &Path) -> AppResult<()> {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(dir)
        .spawn()
        .map(drop)
        .map_err(|e| AppError::from(e).with_param("path", dir.display()))
}

/// `settings_get_language` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SettingsLanguage {
    /// 已保存的界面语言（BCP 47）；首次启动（尚无可用的 `config.json`）时为 `null`，
    /// 由前端按系统语言选择后通过 `settings_set_language` 写入。
    pub language: Option<String>,
}

/// `settings_set_language` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SettingsSetLanguageArgs {
    /// 界面语言，BCP 47 标签，例如 `zh-CN`、`en-US`。
    pub language: String,
}

/// 读取已保存的界面语言。
#[tauri::command]
pub async fn settings_get_language(
    state: State<'_, AppState>,
) -> Result<SettingsLanguage, AppError> {
    let _guard = state.config_lock.lock().await;
    get_language(state.data()?)
}

/// 保存界面语言到 `config.json`，其他设置保持不变。
#[tauri::command]
pub async fn settings_set_language(
    state: State<'_, AppState>,
    args: SettingsSetLanguageArgs,
) -> Result<(), AppError> {
    validate_language(&args.language)?;
    let _guard = state.config_lock.lock().await;
    set_language(state.data()?, args.language)
}

fn get_language(paths: &DataPaths) -> AppResult<SettingsLanguage> {
    let path = paths.config_file();
    if !path.exists() {
        return Ok(SettingsLanguage { language: None });
    }
    let loaded = Config::load(&path)?;
    // 原文件损坏时已被备份并回退默认值，视同首次启动。
    let language = loaded.backup.is_none().then_some(loaded.config.language);
    Ok(SettingsLanguage { language })
}

fn set_language(paths: &DataPaths, language: String) -> AppResult<()> {
    let path = paths.config_file();
    let mut config = Config::load(&path)?.config;
    config.language = language;
    config.save(&path)
}

/// 只接受形如 `zh-CN` 的标签：字母、数字和连字符，最长 35 个字符。
fn validate_language(language: &str) -> AppResult<()> {
    let valid = !language.is_empty()
        && language.len() <= 35
        && language
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !valid {
        return Err(AppError::new(ErrorCode::CommonInternal)
            .with_detail("settings_set_language: language must be a BCP 47 tag"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn first_launch_has_no_language() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("tmcl-data"));
        assert_eq!(get_language(&paths).unwrap().language, None);
    }

    #[test]
    fn set_then_get_keeps_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("tmcl-data"));
        let mut config = Config::default();
        config.download.max_concurrency = 4;
        config.save(&paths.config_file()).unwrap();

        set_language(&paths, "en-US".to_owned()).unwrap();

        assert_eq!(
            get_language(&paths).unwrap().language.as_deref(),
            Some("en-US")
        );
        let saved = Config::load(&paths.config_file()).unwrap().config;
        assert_eq!(saved.download.max_concurrency, 4);
    }

    #[test]
    fn corrupt_config_counts_as_first_launch() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        fs::write(paths.config_file(), b"{ not json").unwrap();
        assert_eq!(get_language(&paths).unwrap().language, None);
    }

    #[test]
    fn rejects_malformed_language() {
        assert!(validate_language("zh-CN").is_ok());
        for bad in ["", "../x", "zh CN", &"a".repeat(36)] {
            let err = validate_language(bad).unwrap_err();
            assert_eq!(err.code, ErrorCode::CommonInternal);
        }
    }

    fn sample() -> LauncherSettings {
        LauncherSettings::from_config(&Config::default())
    }

    #[test]
    fn settings_roundtrip_keeps_language() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        set_language(&paths, "en-US".to_owned()).unwrap();

        let mut next = sample();
        next.theme = ThemeMode::Dark;
        next.appearance.accent = Some("#FF0000".to_owned());
        next.memory.max_mb = 8192;
        let (saved, download_changed) = set_settings(&paths, next.clone()).unwrap();
        assert_eq!(saved, next);
        assert!(!download_changed);

        let snapshot = get_settings(&paths).unwrap();
        assert_eq!(snapshot.settings, next);
        assert_eq!(snapshot.config_backup, None);
        assert_eq!(snapshot.data_dir, dir.path().display().to_string());
        assert_eq!(
            get_language(&paths).unwrap().language.as_deref(),
            Some("en-US")
        );

        next.download.max_concurrency = 4;
        assert!(set_settings(&paths, next).unwrap().1);
    }

    #[test]
    fn corrupt_config_is_reported_by_get() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        fs::write(paths.config_file(), b"{ not json").unwrap();
        let snapshot = get_settings(&paths).unwrap();
        assert_eq!(snapshot.settings, sample());
        assert!(snapshot.config_backup.is_some());
    }

    #[test]
    fn rejects_out_of_range_settings() {
        assert!(validate_settings(&sample()).is_ok());
        let cases: [fn(&mut LauncherSettings); 5] = [
            |s| s.appearance.accent = Some("red".to_owned()),
            |s| s.appearance.accent = Some("#GGGGGG".to_owned()),
            |s| s.download.max_concurrency = 0,
            |s| s.download.max_concurrency = MAX_CONCURRENCY + 1,
            |s| s.memory.max_mb = 256,
        ];
        for change in cases {
            let mut settings = sample();
            change(&mut settings);
            let err = validate_settings(&settings).unwrap_err();
            assert_eq!(err.code, ErrorCode::CommonInternal);
        }
    }
}
