//! 本地皮肤库：`<data_root>/skins/` 下的 `<sha256>.png` 与描述它们的 `index.json`。
//!
//! 皮肤以文件内容的 SHA-256 为 ID，同一张图片只保存一次；导入时校验 PNG 尺寸。

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tmcl_common::{AppError, AppResult, ErrorCode, write_atomic};
use ts_rs::TS;

use crate::SkinVariant;

/// 皮肤文件的大小上限（字节）；64×64 的 PNG 通常只有几 KB。
pub const MAX_SKIN_BYTES: usize = 1024 * 1024;
/// 皮肤名称的最大字符数。
pub const MAX_NAME_CHARS: usize = 64;

const INDEX_FILE: &str = "index.json";
const INDEX_SCHEMA: u32 = 1;
/// PNG 文件签名。
pub const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// 串行化 `index.json` 的“读取—修改—写回”。
static INDEX_LOCK: Mutex<()> = Mutex::new(());

/// 皮肤库中的一个皮肤。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SkinEntry {
    /// 文件内容的 SHA-256（小写十六进制），同时是文件名 `<id>.png`。
    pub id: String,
    pub name: String,
    pub variant: SkinVariant,
    /// 图片高度：64 或 32（旧版 64×32 皮肤）。
    pub height: u32,
    /// 加入皮肤库的时间（unix 毫秒）。
    #[ts(type = "number")]
    pub added_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    schema: u32,
    skins: Vec<SkinEntry>,
}

fn invalid_image() -> AppError {
    AppError::new(ErrorCode::SkinInvalidImage)
}

/// 校验皮肤图片：必须是 PNG，且尺寸为 64×64 或 64×32。返回图片高度。
///
/// 只读取 PNG 签名与 IHDR 头，不解码像素。
pub fn validate_png(bytes: &[u8]) -> AppResult<u32> {
    if bytes.len() > MAX_SKIN_BYTES
        || bytes.len() < 24
        || !bytes.starts_with(PNG_SIGNATURE)
        || &bytes[12..16] != b"IHDR"
    {
        return Err(invalid_image());
    }
    let be =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let (width, height) = (be(16), be(20));
    if width == 64 && (height == 64 || height == 32) {
        Ok(height)
    } else {
        Err(invalid_image()
            .with_param("width", width)
            .with_param("height", height))
    }
}

/// 皮肤 ID 只能是 64 位小写十六进制，防止路径穿越。
fn is_skin_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// 去掉首尾空白；空名称或过长返回 `COMMON_INTERNAL`（`params.name`）。
fn normalize_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(AppError::new(ErrorCode::CommonInternal).with_param("name", name));
    }
    Ok(name.to_owned())
}

fn not_found(id: &str) -> AppError {
    AppError::new(ErrorCode::SkinNotFound).with_param("id", id)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// 本地皮肤库。
#[derive(Debug, Clone)]
pub struct SkinLibrary {
    dir: PathBuf,
}

impl SkinLibrary {
    /// 以 `dir`（通常是 `DataPaths::skins_dir()`）为皮肤库目录，目录在首次写入时创建。
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn lock() -> MutexGuard<'static, ()> {
        INDEX_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.png"))
    }

    /// 读取索引；文件不存在时为空。索引中图片已丢失的条目被忽略。
    fn load(&self) -> AppResult<Index> {
        let path = self.dir.join(INDEX_FILE);
        let mut index: Index = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| AppError::from(e).with_param("path", path.display()))?,
            Err(e) if e.kind() == ErrorKind::NotFound => Index::default(),
            Err(e) => return Err(AppError::from(e).with_param("path", path.display())),
        };
        index
            .skins
            .retain(|s| is_skin_id(&s.id) && self.file(&s.id).is_file());
        Ok(index)
    }

    fn save(&self, mut index: Index) -> AppResult<()> {
        index.schema = INDEX_SCHEMA;
        let path = self.dir.join(INDEX_FILE);
        let bytes = serde_json::to_vec_pretty(&index)?;
        write_atomic(&path, &bytes).map_err(|e| e.with_param("path", path.display()))
    }

    /// 所有皮肤，最近加入的在前。
    pub fn list(&self) -> AppResult<Vec<SkinEntry>> {
        let _guard = Self::lock();
        let mut skins = self.load()?.skins;
        skins.sort_by_key(|s| std::cmp::Reverse(s.added_at));
        Ok(skins)
    }

    /// 按 ID 查找皮肤；不存在返回 `SKIN_NOT_FOUND`。
    pub fn get(&self, id: &str) -> AppResult<SkinEntry> {
        let _guard = Self::lock();
        self.load()?
            .skins
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| not_found(id))
    }

    /// 读取皮肤图片；不存在返回 `SKIN_NOT_FOUND`。
    pub fn read(&self, id: &str) -> AppResult<Vec<u8>> {
        self.get(id)?;
        fs::read(self.file(id)).map_err(|_| not_found(id))
    }

    /// 加入一张皮肤。图片不合法返回 `SKIN_INVALID_IMAGE`；
    /// 同一张图片已在库中时不重复保存，直接返回已有的条目（名称与模型不变）。
    pub fn add(&self, png: &[u8], name: &str, variant: SkinVariant) -> AppResult<SkinEntry> {
        let height = validate_png(png)?;
        let name = normalize_name(name)?;
        let id: String = Sha256::digest(png)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let _guard = Self::lock();
        let mut index = self.load()?;
        if let Some(existing) = index.skins.iter().find(|s| s.id == id) {
            return Ok(existing.clone());
        }
        let file = self.file(&id);
        write_atomic(&file, png).map_err(|e| e.with_param("path", file.display()))?;
        let entry = SkinEntry {
            id,
            name,
            variant,
            height,
            added_at: now_millis(),
        };
        index.skins.push(entry.clone());
        self.save(index)?;
        Ok(entry)
    }

    /// 修改名称与模型；不存在返回 `SKIN_NOT_FOUND`。
    pub fn update(&self, id: &str, name: &str, variant: SkinVariant) -> AppResult<SkinEntry> {
        let name = normalize_name(name)?;
        let _guard = Self::lock();
        let mut index = self.load()?;
        let entry = index
            .skins
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| not_found(id))?;
        entry.name = name;
        entry.variant = variant;
        let updated = entry.clone();
        self.save(index)?;
        Ok(updated)
    }

    /// 删除皮肤及其图片；不存在返回 `SKIN_NOT_FOUND`。
    pub fn remove(&self, id: &str) -> AppResult<()> {
        let _guard = Self::lock();
        let mut index = self.load()?;
        let before = index.skins.len();
        index.skins.retain(|s| s.id != id);
        if index.skins.len() == before {
            return Err(not_found(id));
        }
        self.save(index)?;
        match fs::remove_file(self.file(id)) {
            Err(e) if e.kind() != ErrorKind::NotFound => Err(AppError::from(e)),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// 只有签名与 IHDR 头的“PNG”，足以通过尺寸校验。
    pub(crate) fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }

    #[test]
    fn png_validation() {
        assert_eq!(validate_png(&png(64, 64)).unwrap(), 64);
        assert_eq!(validate_png(&png(64, 32)).unwrap(), 32);
        let err = validate_png(&png(128, 128)).unwrap_err();
        assert_eq!(err.code, ErrorCode::SkinInvalidImage);
        assert_eq!(err.params["width"], "128");
        for bad in [Vec::new(), b"GIF89a".to_vec(), png(64, 64)[..20].to_vec()] {
            assert_eq!(
                validate_png(&bad).unwrap_err().code,
                ErrorCode::SkinInvalidImage
            );
        }
        let mut huge = png(64, 64);
        huge.resize(MAX_SKIN_BYTES + 1, 0);
        assert_eq!(
            validate_png(&huge).unwrap_err().code,
            ErrorCode::SkinInvalidImage
        );
    }

    #[test]
    fn add_list_update_remove() {
        let dir = tempfile::tempdir().unwrap();
        let lib = SkinLibrary::new(dir.path().join("skins"));
        assert!(lib.list().unwrap().is_empty());

        let a = lib
            .add(&png(64, 64), " Steve ", SkinVariant::Classic)
            .unwrap();
        assert_eq!(a.name, "Steve");
        assert!(is_skin_id(&a.id));
        let again = lib.add(&png(64, 64), "Other", SkinVariant::Slim).unwrap();
        assert_eq!(again, a, "相同图片只保存一次");
        let b = lib.add(&png(64, 32), "Old", SkinVariant::Classic).unwrap();
        assert_eq!(b.height, 32);
        let ids: Vec<_> = lib.list().unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&a.id) && ids.contains(&b.id));
        assert_eq!(lib.read(&a.id).unwrap(), png(64, 64));

        let renamed = lib.update(&a.id, "Alex", SkinVariant::Slim).unwrap();
        assert_eq!(
            (renamed.name.as_str(), renamed.variant),
            ("Alex", SkinVariant::Slim)
        );
        assert_eq!(lib.get(&a.id).unwrap(), renamed);
        assert_eq!(
            lib.update(&a.id, "  ", SkinVariant::Slim).unwrap_err().code,
            ErrorCode::CommonInternal
        );

        lib.remove(&a.id).unwrap();
        assert!(
            !dir.path()
                .join("skins")
                .join(format!("{}.png", a.id))
                .exists()
        );
        assert_eq!(lib.list().unwrap(), vec![b]);
        for missing in [a.id.as_str(), "../index", ""] {
            assert_eq!(
                lib.remove(missing).unwrap_err().code,
                ErrorCode::SkinNotFound
            );
            assert_eq!(lib.read(missing).unwrap_err().code, ErrorCode::SkinNotFound);
        }
    }

    #[test]
    fn invalid_image_is_not_saved() {
        let dir = tempfile::tempdir().unwrap();
        let lib = SkinLibrary::new(dir.path());
        let err = lib
            .add(&png(32, 32), "x", SkinVariant::Classic)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::SkinInvalidImage);
        assert!(lib.list().unwrap().is_empty());
        assert!(!dir.path().join(INDEX_FILE).exists());
    }

    #[test]
    fn entries_with_missing_files_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let lib = SkinLibrary::new(dir.path());
        let a = lib.add(&png(64, 64), "a", SkinVariant::Classic).unwrap();
        fs::remove_file(lib.file(&a.id)).unwrap();
        assert!(lib.list().unwrap().is_empty());
        assert_eq!(lib.get(&a.id).unwrap_err().code, ErrorCode::SkinNotFound);
    }
}
