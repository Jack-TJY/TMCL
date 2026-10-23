//! `appearance_*` 命令：导入与读取自定义背景图片（PLAN.md M1-19）。
//!
//! 图片复制到 `<data_root>/ui/backgrounds/`，文件名由启动器生成；`config.json` 的
//! `appearance.background.image` 只保存文件名，界面经 `appearance_read_background`
//! 取回图片内容，不直接访问文件系统。

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Response;
use tmcl_common::config::BackgroundConfig;
use tmcl_common::{AppError, AppResult, Config, DataPaths, ErrorCode, write_atomic};
use ts_rs::TS;

use crate::state::AppState;

/// 背景图片的大小上限（字节）。
pub const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
/// 背景模糊半径上限（px）。
pub const MAX_BLUR: u8 = 40;

/// `appearance_import_background` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct AppearanceImportArgs {
    /// 用户选中的图片的绝对路径。
    pub path: String,
}

/// `appearance_import_background` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct AppearanceImported {
    /// 复制后的文件名，写入 `appearance.background.image`。
    pub image: String,
}

/// `appearance_read_background` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct AppearanceImageArgs {
    /// `appearance.background.image` 中的文件名。
    pub image: String,
}

/// 把图片复制到背景目录，返回新文件名；同时清理除新图片和当前设置所用图片之外的旧图片。
#[tauri::command]
pub async fn appearance_import_background(
    state: State<'_, AppState>,
    args: AppearanceImportArgs,
) -> Result<AppearanceImported, AppError> {
    let paths = state.data()?;
    let _guard = state.config_lock.lock().await;
    import_background(paths, Path::new(&args.path))
}

/// 读取背景图片，以二进制返回（前端收到 `ArrayBuffer`）。
#[tauri::command]
pub async fn appearance_read_background(
    state: State<'_, AppState>,
    args: AppearanceImageArgs,
) -> Result<Response, AppError> {
    read_background(state.data()?, &args.image).map(Response::new)
}

/// 按文件头识别图片格式，返回扩展名。
fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// 只接受启动器生成的文件名，防止路径穿越。
fn is_image_name(name: &str) -> bool {
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    stem.strip_prefix("bg-").is_some_and(|rest| {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    }) && matches!(ext, "png" | "jpg" | "webp")
}

fn unsupported(path: &Path) -> AppError {
    AppError::new(ErrorCode::AppearanceImageUnsupported)
        .with_param("max_mb", MAX_IMAGE_BYTES / 1024 / 1024)
        .with_detail(format!("not a supported image: {}", path.display()))
}

fn import_background(paths: &DataPaths, source: &Path) -> AppResult<AppearanceImported> {
    let size = fs::metadata(source)
        .map_err(|e| AppError::from(e).with_param("path", source.display()))?
        .len();
    if size > MAX_IMAGE_BYTES {
        return Err(unsupported(source));
    }
    let bytes =
        fs::read(source).map_err(|e| AppError::from(e).with_param("path", source.display()))?;
    let ext = image_extension(&bytes).ok_or_else(|| unsupported(source))?;

    let dir = paths.ui_backgrounds_dir();
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let mut name = format!("bg-{millis}.{ext}");
    let mut n = 1;
    while dir.join(&name).exists() {
        name = format!("bg-{millis}-{n}.{ext}");
        n += 1;
    }
    let target = dir.join(&name);
    write_atomic(&target, &bytes).map_err(|e| e.with_param("path", target.display()))?;

    let current = Config::load(&paths.config_file())?
        .config
        .appearance
        .background
        .image;
    prune(&dir, &[Some(name.as_str()), current.as_deref()]);
    Ok(AppearanceImported { image: name })
}

/// 删除背景目录中除 `keep` 之外的启动器图片；删除失败只记录日志。
fn prune(dir: &Path, keep: &[Option<&str>]) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_image_name(&name) || keep.contains(&Some(name.as_str())) {
            continue;
        }
        if let Err(e) = fs::remove_file(entry.path()) {
            tracing::warn!(file = %name, error = %e, "删除旧背景图片失败");
        }
    }
}

fn read_background(paths: &DataPaths, image: &str) -> AppResult<Vec<u8>> {
    let missing = || AppError::new(ErrorCode::AppearanceImageMissing).with_param("image", image);
    if !is_image_name(image) {
        return Err(missing().with_detail("invalid background file name"));
    }
    fs::read(paths.ui_backgrounds_dir().join(image)).map_err(|e| match e.kind() {
        ErrorKind::NotFound => missing(),
        _ => AppError::from(e).with_param("path", image),
    })
}

/// 检查背景设置的取值范围，供 `settings_set` 使用。
pub(crate) fn background_valid(background: &BackgroundConfig) -> bool {
    let color = &background.color;
    let color_ok = color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    color_ok
        && background.image.as_deref().is_none_or(is_image_name)
        && background.opacity <= 100
        && background.blur <= MAX_BLUR
        && (-100..=100).contains(&background.mask)
        && background.card_opacity <= 100
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    const WEBP: &[u8] = b"RIFF\x10\0\0\0WEBPVP8 ";

    fn write_source(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn detects_supported_formats() {
        assert_eq!(image_extension(PNG), Some("png"));
        assert_eq!(image_extension(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(image_extension(WEBP), Some("webp"));
        assert_eq!(image_extension(b"GIF89a"), None);
        assert_eq!(image_extension(b"RIFF"), None);
    }

    #[test]
    fn image_names_cannot_escape_the_directory() {
        assert!(is_image_name("bg-1700000000000.png"));
        assert!(is_image_name("bg-1700000000000-2.webp"));
        for bad in [
            "../bg-1.png",
            "bg-.png",
            "bg-1.gif",
            "a.png",
            "bg-1/../x.png",
            "bg-1",
        ] {
            assert!(!is_image_name(bad), "{bad}");
        }
    }

    #[test]
    fn import_copies_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("data"));
        let source = write_source(dir.path(), "photo.webp", WEBP);

        let imported = import_background(&paths, &source).unwrap();
        assert!(imported.image.ends_with(".webp"));
        assert_eq!(read_background(&paths, &imported.image).unwrap(), WEBP);
    }

    #[test]
    fn import_keeps_new_and_current_images_only() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("data"));
        let source = write_source(dir.path(), "a.png", PNG);

        let first = import_background(&paths, &source).unwrap().image;
        let mut config = Config::default();
        config.appearance.background.image = Some(first.clone());
        config.save(&paths.config_file()).unwrap();

        let second = import_background(&paths, &source).unwrap().image;
        let third = import_background(&paths, &source).unwrap().image;
        let backgrounds = paths.ui_backgrounds_dir();
        assert!(backgrounds.join(&first).exists());
        assert!(!backgrounds.join(&second).exists());
        assert!(backgrounds.join(&third).exists());
    }

    #[test]
    fn rejects_unsupported_or_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("data"));
        let gif = write_source(dir.path(), "a.gif", b"GIF89a....");
        let err = import_background(&paths, &gif).unwrap_err();
        assert_eq!(err.code, ErrorCode::AppearanceImageUnsupported);
        assert_eq!(err.params.get("max_mb").map(String::as_str), Some("32"));

        let big = dir.path().join("big.png");
        let file = fs::File::create(&big).unwrap();
        file.set_len(MAX_IMAGE_BYTES + 1).unwrap();
        let err = import_background(&paths, &big).unwrap_err();
        assert_eq!(err.code, ErrorCode::AppearanceImageUnsupported);

        let err = import_background(&paths, &dir.path().join("none.png")).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonIo);
    }

    #[test]
    fn deleted_image_is_reported_missing() {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        for image in ["bg-1.png", "../config.json"] {
            let err = read_background(&paths, image).unwrap_err();
            assert_eq!(err.code, ErrorCode::AppearanceImageMissing);
            assert_eq!(err.params.get("image").map(String::as_str), Some(image));
        }
    }

    #[test]
    fn validates_background_ranges() {
        assert!(background_valid(&BackgroundConfig::default()));
        let cases: [fn(&mut BackgroundConfig); 7] = [
            |b| b.color = "blue".to_owned(),
            |b| b.image = Some("../x.png".to_owned()),
            |b| b.opacity = 101,
            |b| b.blur = MAX_BLUR + 1,
            |b| b.mask = -101,
            |b| b.mask = 101,
            |b| b.card_opacity = 101,
        ];
        for change in cases {
            let mut background = BackgroundConfig::default();
            change(&mut background);
            assert!(!background_valid(&background));
        }
    }
}
