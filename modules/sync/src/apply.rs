//! 把主配置应用到目标实例的 `options.txt`。
//!
//! 只应用选中的类别；按键与语言代码按目标实例的游戏版本转换。写入前把原文件备份为
//! `options.txt.tmcl-bak-<unix 毫秒>`，每个实例最多保留 [`MAX_BACKUPS`] 份，
//! [`restore_latest`] 用最近一份备份撤销。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tmcl_common::{AppError, AppResult, ErrorCode, write_atomic};
use ts_rs::TS;

use crate::keymap::{self, KeyFormat};
use crate::options::OptionsFile;
use crate::profile::{Category, SyncOption, SyncProfile, category_of};

/// 游戏目录下的选项文件名。
pub const OPTIONS_FILE: &str = "options.txt";
/// 备份文件名前缀，后接 unix 毫秒。
pub const BACKUP_PREFIX: &str = "options.txt.tmcl-bak-";
/// 每个实例保留的备份数。
pub const MAX_BACKUPS: usize = 10;

/// 一项将要发生的修改。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SyncOptionChange")]
pub struct OptionChange {
    /// 选项键。
    pub key: String,
    /// 所属类别。
    pub category: Category,
    /// 目标中的原值；原来没有该选项时为 `null`。
    pub old: Option<String>,
    /// 写入的新值（已按目标版本转换）。
    pub new: String,
}

/// 一次应用的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// 修改的选项数；为 0 时文件未被改动。
    pub changed: usize,
    /// 原文件的备份；没有修改或原文件不存在时为 `None`。
    pub backup: Option<PathBuf>,
}

/// 把主配置中的一条选项转换为目标格式的值。
fn target_value(option: &SyncOption, format: KeyFormat, lowercase_lang: bool) -> String {
    if option.key.starts_with("key_") {
        keymap::convert_value(&option.value, format)
    } else if option.key == "lang" && !lowercase_lang {
        // 1.11 以前地区部分为大写：`zh_CN`。
        match option.value.split_once('_') {
            Some((lang, region)) => format!("{lang}_{}", region.to_ascii_uppercase()),
            None => option.value.clone(),
        }
    } else {
        option.value.clone()
    }
}

/// 计算把主配置的选中类别应用到 `target` 时会发生的修改，按主配置中的顺序。
///
/// 按键格式取决于 `game_version`；版本号无法识别时按目标文件中已有的按键判断，
/// 仍无法判断时按 1.13 以后的格式。
pub fn plan(
    master: &SyncProfile,
    target: &OptionsFile,
    game_version: &str,
    categories: &[Category],
) -> Vec<OptionChange> {
    let format = keymap::format_for_version(game_version)
        .or_else(|| {
            target
                .entries()
                .filter(|(key, _)| key.starts_with("key_"))
                .find_map(|(_, value)| keymap::detect_value_format(value))
        })
        .unwrap_or(KeyFormat::Named);
    let lowercase_lang =
        keymap::version_at_least(game_version, 11, (16, 32)).unwrap_or(format == KeyFormat::Named);
    let mut changes: Vec<OptionChange> = Vec::new();
    for option in &master.options {
        let Some(category) = category_of(&option.key).filter(|c| categories.contains(c)) else {
            continue;
        };
        let new = target_value(option, format, lowercase_lang);
        let old = target.get(&option.key);
        if old == Some(new.as_str()) {
            continue;
        }
        let change = OptionChange {
            key: option.key.clone(),
            category,
            old: old.map(str::to_owned),
            new,
        };
        // 主配置中重复的键只保留最后一次。
        changes.retain(|c| c.key != change.key);
        changes.push(change);
    }
    changes
}

/// 读取游戏目录下的 `options.txt`；不存在时为空文件。
pub fn read_options(game_dir: &Path) -> AppResult<OptionsFile> {
    read_existing(game_dir).map(Option::unwrap_or_default)
}

/// 读取游戏目录下的 `options.txt`；不存在时为 `None`，不是 UTF-8 时返回 `COMMON_IO`。
pub fn read_existing(game_dir: &Path) -> AppResult<Option<OptionsFile>> {
    let path = game_dir.join(OPTIONS_FILE);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppError::from(e).with_param("path", path.display())),
    };
    let text = String::from_utf8(bytes).map_err(|_| {
        AppError::new(ErrorCode::CommonIo)
            .with_param("path", path.display())
            .with_detail("options.txt is not valid UTF-8")
    })?;
    Ok(Some(OptionsFile::parse(&text)))
}

/// 把主配置的选中类别应用到游戏目录：有修改时先备份原文件，再原子写入。
pub fn apply(
    game_dir: &Path,
    game_version: &str,
    master: &SyncProfile,
    categories: &[Category],
) -> AppResult<Applied> {
    let existing = read_existing(game_dir)?;
    let mut file = existing.clone().unwrap_or_default();
    let changes = plan(master, &file, game_version, categories);
    if changes.is_empty() {
        return Ok(Applied {
            changed: 0,
            backup: None,
        });
    }
    let path = game_dir.join(OPTIONS_FILE);
    let backup = match existing {
        Some(_) => Some(backup(game_dir, &path)?),
        None => None,
    };
    for change in &changes {
        file.set(&change.key, &change.new);
    }
    write_atomic(&path, file.to_text().as_bytes())
        .map_err(|e| e.with_param("path", path.display()))?;
    prune_backups(game_dir);
    Ok(Applied {
        changed: changes.len(),
        backup,
    })
}

/// 复制原文件为新的备份。
fn backup(game_dir: &Path, path: &Path) -> AppResult<PathBuf> {
    let mut millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    // 备份序号须严格递增，同一毫秒内多次应用时顺延。
    if let Some((latest, _)) = backups(game_dir).last() {
        millis = millis.max(latest + 1);
    }
    let target = game_dir.join(format!("{BACKUP_PREFIX}{millis}"));
    fs::copy(path, &target).map_err(|e| AppError::from(e).with_param("path", target.display()))?;
    Ok(target)
}

/// 游戏目录下的备份，按时间从旧到新。
fn backups(game_dir: &Path) -> Vec<(u128, PathBuf)> {
    let Ok(entries) = fs::read_dir(game_dir) else {
        return Vec::new();
    };
    let mut list: Vec<(u128, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let stamp = name.to_str()?.strip_prefix(BACKUP_PREFIX)?.parse().ok()?;
            Some((stamp, entry.path()))
        })
        .collect();
    list.sort();
    list
}

/// 删除超出 [`MAX_BACKUPS`] 的旧备份；失败只记日志。
fn prune_backups(game_dir: &Path) {
    let list = backups(game_dir);
    let excess = list.len().saturating_sub(MAX_BACKUPS);
    for (_, path) in &list[..excess] {
        if let Err(e) = fs::remove_file(path) {
            tracing::warn!(error = %e, "删除旧的 options.txt 备份失败");
        }
    }
}

/// 最近一份备份。
pub fn latest_backup(game_dir: &Path) -> Option<PathBuf> {
    backups(game_dir).pop().map(|(_, path)| path)
}

/// 用最近一份备份覆盖 `options.txt`，并移除该备份（再次调用撤销更早的一次）。
/// 没有备份时返回 `None`，文件不变。
pub fn restore_latest(game_dir: &Path) -> AppResult<Option<PathBuf>> {
    let Some(latest) = latest_backup(game_dir) else {
        return Ok(None);
    };
    let path = game_dir.join(OPTIONS_FILE);
    fs::rename(&latest, &path).map_err(|e| AppError::from(e).with_param("path", path.display()))?;
    Ok(Some(latest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::capture_options;

    const ALL: &[Category] = &[
        Category::Keys,
        Category::Video,
        Category::Audio,
        Category::Language,
        Category::Other,
    ];
    const MODERN: &str = include_str!("../tests/fixtures/options-1.20.1.txt");
    const LEGACY: &str = include_str!("../tests/fixtures/options-1.12.2.txt");

    fn profile_from(text: &str, version: &str) -> SyncProfile {
        SyncProfile {
            source_instance: None,
            source_version: Some(version.into()),
            updated_at: 0,
            options: capture_options(&OptionsFile::parse(text), version),
        }
    }

    fn keys(text: &str) -> Vec<(String, String)> {
        OptionsFile::parse(text)
            .entries()
            .filter(|(k, _)| k.starts_with("key_"))
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect()
    }

    #[test]
    fn roundtrip_1_20_1_to_1_12_2_and_back_keeps_key_bindings() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        let new = dir.path().join("new");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join(OPTIONS_FILE), LEGACY).unwrap();

        let master = profile_from(MODERN, "1.20.1");
        let applied = apply(&old, "1.12.2", &master, &[Category::Keys]).unwrap();
        assert!(applied.changed > 0);
        let legacy = fs::read_to_string(old.join(OPTIONS_FILE)).unwrap();
        assert!(legacy.contains("key_key.forward:200\n"), "{legacy}");
        assert!(legacy.contains("key_key.attack:-100\n"));
        assert!(legacy.contains("key_examplemod.key.dash:47:CONTROL\n"));
        assert!(legacy.contains("lang:en_US\n"), "未选中的类别不应改动");

        let back = profile_from(&legacy, "1.12.2");
        fs::write(new.join(OPTIONS_FILE), "version:3465\n").unwrap();
        apply(&new, "1.20.1", &back, &[Category::Keys]).unwrap();
        let modern = fs::read_to_string(new.join(OPTIONS_FILE)).unwrap();
        // 1.12.2 独有的按键（如 `key.swapHands`）也会被带回，只比较 1.20.1 原有的按键。
        let mut expected = keys(MODERN);
        let mut actual: Vec<(String, String)> = keys(&modern)
            .into_iter()
            .filter(|(k, _)| expected.iter().any(|(e, _)| e == k))
            .collect();
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected);
    }

    #[test]
    fn plan_respects_categories_and_language_case() {
        let master = profile_from(MODERN, "1.20.1");
        let target = OptionsFile::parse(LEGACY);
        let changes = plan(&master, &target, "1.10.2", &[Category::Language]);
        let lang = changes.iter().find(|c| c.key == "lang").unwrap();
        assert_eq!(lang.new, "zh_CN");
        assert_eq!(lang.old.as_deref(), Some("en_US"));
        assert!(changes.iter().all(|c| c.category == Category::Language));

        let all = plan(&master, &target, "1.12.2", ALL);
        assert!(
            all.iter()
                .all(|c| c.key != "version" && c.key != "resourcePacks")
        );
        assert!(all.iter().any(|c| c.category == Category::Audio));
        assert!(plan(&master, &target, "1.12.2", &[]).is_empty());

        // 版本号无法识别时按目标文件中的按键判断格式。
        let changes = plan(&master, &target, "custom", &[Category::Keys]);
        let jump = changes.iter().find(|c| c.key == "key_key.forward").unwrap();
        assert_eq!(jump.new, "200");
    }

    #[test]
    fn apply_backs_up_and_restore_undoes() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        fs::write(game.join(OPTIONS_FILE), LEGACY).unwrap();
        let master = profile_from(MODERN, "1.20.1");

        let first = apply(game, "1.12.2", &master, &[Category::Video]).unwrap();
        let backup = first.backup.unwrap();
        assert_eq!(fs::read_to_string(&backup).unwrap(), LEGACY);
        let after_video = fs::read_to_string(game.join(OPTIONS_FILE)).unwrap();
        // 第二次应用相同内容不产生修改与备份。
        let again = apply(game, "1.12.2", &master, &[Category::Video]).unwrap();
        assert_eq!(
            again,
            Applied {
                changed: 0,
                backup: None
            }
        );

        apply(game, "1.12.2", &master, ALL).unwrap();
        assert_eq!(backups(game).len(), 2);
        assert!(restore_latest(game).unwrap().is_some());
        assert_eq!(
            fs::read_to_string(game.join(OPTIONS_FILE)).unwrap(),
            after_video
        );
        assert!(restore_latest(game).unwrap().is_some());
        assert_eq!(fs::read_to_string(game.join(OPTIONS_FILE)).unwrap(), LEGACY);
        assert_eq!(restore_latest(game).unwrap(), None);
    }

    #[test]
    fn apply_creates_missing_file_and_prunes_backups() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        let mut master = profile_from(MODERN, "1.20.1");
        let created = apply(game, "1.20.1", &master, &[Category::Video]).unwrap();
        assert_eq!(created.backup, None);
        assert!(read_options(game).unwrap().get("renderDistance").is_some());

        for i in 0..MAX_BACKUPS + 3 {
            master.options.push(SyncOption {
                key: "fov".into(),
                value: format!("0.{i}"),
            });
            apply(game, "1.20.1", &master, &[Category::Video]).unwrap();
        }
        assert_eq!(backups(game).len(), MAX_BACKUPS);

        fs::write(game.join(OPTIONS_FILE), [0xff, 0xfe]).unwrap();
        let err = apply(game, "1.20.1", &master, ALL).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonIo);
    }
}
