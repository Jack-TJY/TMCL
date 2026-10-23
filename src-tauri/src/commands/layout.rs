//! `layout_*` 命令：读写主页组件布局 `<data_root>/ui/layout.json`（PLAN.md M1-18）。
//!
//! 后端不认识具体组件，只保证布局结构合法；未知组件类型由前端跳过。
//! 文件格式见 `docs/contracts/widgets.md`。

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tauri::State;
use tmcl_common::{AppError, AppResult, DataPaths, ErrorCode, write_atomic};
use ts_rs::TS;

use crate::state::AppState;

/// `layout.json` 的当前版本。
pub const LAYOUT_SCHEMA: u64 = 1;
/// 栅格列数。
pub const GRID_COLUMNS: u32 = 12;
/// 组件的最大底边行号（`y + h`），防止异常数据撑出极长的页面。
pub const MAX_ROWS: u32 = 256;
/// 一个布局最多包含的组件数。
pub const MAX_WIDGETS: usize = 64;
/// `uid` 与 `type` 的最大长度。
const MAX_ID_LEN: usize = 64;

/// 主页上的一个组件实例及其位置，单位为栅格格数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct WidgetPlacement {
    /// 组件实例 ID，布局内唯一：字母、数字、`-`、`_`。
    pub uid: String,
    /// 组件类型 ID（`ui/src/widgets/registry.ts` 中登记的 `id`）：小写字母、数字、`-`。
    #[serde(rename = "type")]
    pub kind: String,
    /// 左上角所在列，从 0 开始。
    pub x: u32,
    /// 左上角所在行，从 0 开始。
    pub y: u32,
    /// 宽度（列数），`x + w` 不超过 12。
    pub w: u32,
    /// 高度（行数）。
    pub h: u32,
    /// 组件自己的设置，结构由组件定义。
    #[serde(default)]
    #[ts(type = "Record<string, unknown>")]
    pub settings: Map<String, Value>,
}

/// `layout_get` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LayoutSnapshot {
    /// 已保存的组件；从未保存过或整个文件无法使用时为 `null`，由前端使用默认布局。
    pub widgets: Option<Vec<WidgetPlacement>>,
    /// 因字段缺失或越界而跳过的组件数。
    pub skipped: u32,
    /// 文件无法使用时已备份到此路径；否则为 `null`。
    pub backup: Option<String>,
}

/// `layout_save` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct LayoutSaveArgs {
    /// 完整的组件列表，整体替换已保存的布局。
    pub widgets: Vec<WidgetPlacement>,
}

/// 读取主页布局。
#[tauri::command]
pub async fn layout_get(state: State<'_, AppState>) -> Result<LayoutSnapshot, AppError> {
    load_layout(&state.data()?.ui_layout_file())
}

/// 整体保存主页布局。
#[tauri::command]
pub async fn layout_save(state: State<'_, AppState>, args: LayoutSaveArgs) -> Result<(), AppError> {
    validate_layout(&args.widgets)?;
    save_layout(state.data()?, &args.widgets)
}

fn load_layout(path: &Path) -> AppResult<LayoutSnapshot> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Ok(LayoutSnapshot {
                widgets: None,
                skipped: 0,
                backup: None,
            });
        }
        Err(e) => return Err(AppError::from(e).with_param("path", path.display())),
    };

    let Some(items) = parse_items(&bytes) else {
        let backup = backup_path(path);
        fs::rename(path, &backup)
            .map_err(|e| AppError::from(e).with_param("path", path.display()))?;
        tracing::warn!(backup = %backup.display(), "layout.json 无法使用，已备份并回退默认布局");
        return Ok(LayoutSnapshot {
            widgets: None,
            skipped: 0,
            backup: Some(backup.display().to_string()),
        });
    };

    let total = items.len();
    let mut seen = HashSet::new();
    let widgets: Vec<WidgetPlacement> = items
        .into_iter()
        .filter_map(|item| serde_json::from_value::<WidgetPlacement>(item).ok())
        .filter(|w| is_valid(w) && seen.insert(w.uid.clone()))
        .take(MAX_WIDGETS)
        .collect();
    Ok(LayoutSnapshot {
        skipped: u32::try_from(total - widgets.len()).unwrap_or(u32::MAX),
        widgets: Some(widgets),
        backup: None,
    })
}

/// 取出 `widgets` 数组；不是 JSON、版本不对或结构不对时返回 `None`。
fn parse_items(bytes: &[u8]) -> Option<Vec<Value>> {
    let Value::Object(mut root) = serde_json::from_slice(bytes).ok()? else {
        return None;
    };
    if root.get("schema").and_then(Value::as_u64) != Some(LAYOUT_SCHEMA) {
        return None;
    }
    match root.remove("widgets")? {
        Value::Array(items) => Some(items),
        _ => None,
    }
}

fn save_layout(paths: &DataPaths, widgets: &[WidgetPlacement]) -> AppResult<()> {
    let doc = json!({ "schema": LAYOUT_SCHEMA, "widgets": widgets });
    let bytes = serde_json::to_vec_pretty(&doc).map_err(AppError::from)?;
    let path = paths.ui_layout_file();
    write_atomic(&path, &bytes).map_err(|e| e.with_param("path", path.display()))
}

fn validate_layout(widgets: &[WidgetPlacement]) -> AppResult<()> {
    let mut seen = HashSet::new();
    let valid = widgets.len() <= MAX_WIDGETS
        && widgets
            .iter()
            .all(|w| is_valid(w) && seen.insert(w.uid.as_str()));
    if !valid {
        return Err(AppError::new(ErrorCode::CommonInternal)
            .with_detail("layout_save: invalid or duplicate widget placement"));
    }
    Ok(())
}

fn is_valid(w: &WidgetPlacement) -> bool {
    let id_ok = |s: &str, extra: fn(char) -> bool| {
        !s.is_empty() && s.len() <= MAX_ID_LEN && s.chars().all(extra)
    };
    id_ok(&w.uid, |c| {
        c.is_ascii_alphanumeric() || c == '-' || c == '_'
    }) && id_ok(&w.kind, |c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
    }) && w.w >= 1
        && w.h >= 1
        && w.x.checked_add(w.w).is_some_and(|r| r <= GRID_COLUMNS)
        && w.y.checked_add(w.h).is_some_and(|b| b <= MAX_ROWS)
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

    fn widget(uid: &str, kind: &str, x: u32, y: u32, w: u32, h: u32) -> WidgetPlacement {
        WidgetPlacement {
            uid: uid.to_owned(),
            kind: kind.to_owned(),
            x,
            y,
            w,
            h,
            settings: Map::new(),
        }
    }

    #[test]
    fn missing_file_means_default_layout() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        let snapshot = load_layout(&paths.ui_layout_file()).unwrap();
        assert_eq!(snapshot.widgets, None);
        assert_eq!(snapshot.backup, None);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        let mut first = widget("a1", "start-game", 0, 0, 6, 4);
        first.settings.insert("instance".to_owned(), json!("abc"));
        let widgets = vec![first, widget("b_2", "quick-actions", 6, 0, 6, 2)];
        validate_layout(&widgets).unwrap();
        save_layout(&paths, &widgets).unwrap();

        let saved: Value =
            serde_json::from_slice(&fs::read(paths.ui_layout_file()).unwrap()).unwrap();
        assert_eq!(saved["schema"], 1);
        assert_eq!(saved["widgets"][0]["type"], "start-game");

        let snapshot = load_layout(&paths.ui_layout_file()).unwrap();
        assert_eq!(snapshot.widgets, Some(widgets));
        assert_eq!(snapshot.skipped, 0);
    }

    #[test]
    fn invalid_items_are_skipped_others_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("layout.json");
        let doc = json!({ "schema": 1, "widgets": [
            { "uid": "ok", "type": "news", "x": 0, "y": 0, "w": 4, "h": 3 },
            { "uid": "wide", "type": "news", "x": 10, "y": 0, "w": 4, "h": 3 },
            { "uid": "nosize", "type": "news", "x": 0, "y": 0 },
            { "uid": "ok", "type": "account", "x": 4, "y": 0, "w": 4, "h": 3 },
            { "uid": "bad type", "type": "News", "x": 0, "y": 3, "w": 4, "h": 3 },
            "garbage"
        ]});
        fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();

        let snapshot = load_layout(&path).unwrap();
        let widgets = snapshot.widgets.unwrap();
        assert_eq!(widgets.len(), 1);
        assert_eq!(widgets[0].uid, "ok");
        assert_eq!(widgets[0].settings, Map::new());
        assert_eq!(snapshot.skipped, 5);
        assert_eq!(snapshot.backup, None);
    }

    #[test]
    fn unusable_file_is_backed_up() {
        for bad in [
            &b"{ not json"[..],
            br#"{"schema": 2, "widgets": []}"#,
            br#"{"schema": 1, "widgets": {}}"#,
            b"[]",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("layout.json");
            fs::write(&path, bad).unwrap();
            let snapshot = load_layout(&path).unwrap();
            assert_eq!(snapshot.widgets, None);
            let backup = snapshot.backup.expect("应当产生备份");
            assert_eq!(fs::read(&backup).unwrap(), bad);
            assert!(!path.exists());
        }
    }

    #[test]
    fn save_rejects_invalid_layouts() {
        let ok = widget("a", "news", 0, 0, 12, 2);
        assert!(validate_layout(std::slice::from_ref(&ok)).is_ok());
        let cases = [
            vec![widget("a", "news", 8, 0, 6, 2)],
            vec![widget("a", "news", 0, 0, 0, 2)],
            vec![widget("a", "news", 0, MAX_ROWS, 1, 1)],
            vec![widget("", "news", 0, 0, 1, 1)],
            vec![widget("a", "../x", 0, 0, 1, 1)],
            vec![ok.clone(), ok.clone()],
            vec![widget("a", "news", u32::MAX, 0, 1, 1)],
            (0..=MAX_WIDGETS)
                .map(|i| widget(&format!("w{i}"), "news", 0, 0, 1, 1))
                .collect(),
        ];
        for widgets in cases {
            let err = validate_layout(&widgets).unwrap_err();
            assert_eq!(err.code, ErrorCode::CommonInternal);
        }
    }
}
