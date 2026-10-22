//! `settings_*` 命令：读写 `config.json` 中的启动器设置。
//!
//! M0-07 只提供界面语言；M1-10 在此补全其他设置项。

use serde::{Deserialize, Serialize};
use tauri::State;
use tmcl_common::{AppError, AppResult, Config, DataPaths, ErrorCode};
use ts_rs::TS;

use crate::state::AppState;

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
}
