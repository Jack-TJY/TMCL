//! 本地皮肤库：每张皮肤是一对文件 `<sha256>.png` 与 `<sha256>.json`。
//!
//! 皮肤以文件内容的 SHA-256 为 ID，同一张图片只保存一次；导入时校验 PNG 尺寸。
//!
//! 皮肤库可以放在网盘同步文件夹里，供多台电脑或多个安装目录的 TMCL 共用，所以：
//! - 不用共享的索引文件，每次增删改只动这一张皮肤自己的文件，两边同时改不会互相覆盖；
//! - 有图片但描述文件缺失或损坏时，仍然列出这张皮肤（名称取 ID 前 8 位，模型 classic）；
//! - 文件名不是 `<64 位小写十六进制>.png` 的文件（例如同步盘的冲突副本）一律忽略。
//!
//! 旧版（schema 1）的 `index.json` 在首次读取时拆成各自的描述文件，原文件改名为 `index.json.bak`。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
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

/// 旧版（schema 1）的索引文件。
const LEGACY_INDEX_FILE: &str = "index.json";
/// 描述文件的结构版本。
const META_SCHEMA: u32 = 2;
/// PNG 文件签名。
pub const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// 串行化同一进程内对描述文件的“读取—修改—写回”。
static LOCK: Mutex<()> = Mutex::new(());

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

/// 描述文件 `<id>.json` 的内容；ID 就是文件名，不重复保存。
#[derive(Debug, Serialize, Deserialize)]
struct Meta {
    schema: u32,
    name: String,
    variant: SkinVariant,
    height: u32,
    added_at: u64,
}

impl Meta {
    fn from_entry(entry: &SkinEntry) -> Self {
        Self {
            schema: META_SCHEMA,
            name: entry.name.clone(),
            variant: entry.variant,
            height: entry.height,
            added_at: entry.added_at,
        }
    }

    fn into_entry(self, id: &str) -> SkinEntry {
        SkinEntry {
            id: id.to_owned(),
            name: self.name,
            variant: self.variant,
            height: self.height,
            added_at: self.added_at,
        }
    }
}

/// 旧版 `index.json`。
#[derive(Debug, Default, Deserialize)]
struct LegacyIndex {
    #[serde(default)]
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

fn millis_of(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn io_error(path: &Path) -> impl FnOnce(std::io::Error) -> AppError + '_ {
    move |e| AppError::from(e).with_param("path", path.display())
}

/// 本地皮肤库。
#[derive(Debug, Clone)]
pub struct SkinLibrary {
    dir: PathBuf,
}

impl SkinLibrary {
    /// 以 `dir` 为皮肤库目录（默认 `DataPaths::skins_dir()`，也可以是用户选的文件夹），
    /// 目录在首次写入时创建。
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// 皮肤库目录。
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn lock() -> MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.png"))
    }

    fn meta_file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn write_meta(&self, entry: &SkinEntry) -> AppResult<()> {
        let path = self.meta_file(&entry.id);
        let mut bytes = serde_json::to_vec_pretty(&Meta::from_entry(entry))?;
        bytes.push(b'\n');
        write_atomic(&path, &bytes).map_err(|e| e.with_param("path", path.display()))
    }

    /// 把旧版 `index.json` 拆成描述文件，然后改名为 `index.json.bak`。
    /// 已有描述文件的皮肤不覆盖（可能是别处同步来的新内容）。
    fn migrate_legacy(&self) -> AppResult<()> {
        let path = self.dir.join(LEGACY_INDEX_FILE);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(io_error(&path)(e)),
        };
        // 损坏的旧索引不阻止使用皮肤库：图片仍会以默认名称列出。
        let index: LegacyIndex = serde_json::from_slice(&bytes).unwrap_or_default();
        for entry in &index.skins {
            if is_skin_id(&entry.id)
                && self.file(&entry.id).is_file()
                && !self.meta_file(&entry.id).exists()
            {
                self.write_meta(entry)?;
            }
        }
        let backup = self.dir.join(format!("{LEGACY_INDEX_FILE}.bak"));
        fs::rename(&path, &backup).map_err(io_error(&path))?;
        tracing::info!(
            count = index.skins.len(),
            "皮肤库已迁移到每张皮肤一个描述文件"
        );
        Ok(())
    }

    /// 读取描述文件；缺失或损坏（例如同步到一半）时为 `None`。
    fn read_meta(&self, id: &str) -> Option<Meta> {
        fs::read(self.meta_file(id))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Meta>(&bytes).ok())
            .filter(|meta| normalize_name(&meta.name).is_ok())
    }

    /// 读取一张皮肤；图片不存在返回 `None`。描述文件缺失或损坏时由图片推出默认值，
    /// 图片本身不合法时也返回 `None`。
    fn entry(&self, id: &str) -> AppResult<Option<SkinEntry>> {
        if !is_skin_id(id) {
            return Ok(None);
        }
        let file = self.file(id);
        let modified = match fs::metadata(&file) {
            Ok(meta) if meta.is_file() => meta.modified().map(millis_of).unwrap_or(0),
            Ok(_) => return Ok(None),
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_error(&file)(e)),
        };
        if let Some(meta) = self.read_meta(id) {
            return Ok(Some(meta.into_entry(id)));
        }
        let Ok(height) = fs::read(&file)
            .map_err(io_error(&file))
            .and_then(|png| validate_png(&png))
        else {
            return Ok(None);
        };
        Ok(Some(SkinEntry {
            id: id.to_owned(),
            name: id[..8].to_owned(),
            variant: SkinVariant::Classic,
            height,
            added_at: modified,
        }))
    }

    /// 所有皮肤，最近加入的在前。
    pub fn list(&self) -> AppResult<Vec<SkinEntry>> {
        let _guard = Self::lock();
        self.migrate_legacy()?;
        let read_dir = match fs::read_dir(&self.dir) {
            Ok(read_dir) => read_dir,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io_error(&self.dir)(e)),
        };
        let mut skins = Vec::new();
        for item in read_dir {
            let name = item.map_err(io_error(&self.dir))?.file_name();
            let Some(id) = name.to_str().and_then(|n| n.strip_suffix(".png")) else {
                continue;
            };
            if let Some(entry) = self.entry(id)? {
                skins.push(entry);
            }
        }
        skins.sort_by(|a, b| b.added_at.cmp(&a.added_at).then_with(|| a.id.cmp(&b.id)));
        Ok(skins)
    }

    /// 按 ID 查找皮肤；不存在返回 `SKIN_NOT_FOUND`。
    pub fn get(&self, id: &str) -> AppResult<SkinEntry> {
        let _guard = Self::lock();
        self.migrate_legacy()?;
        self.entry(id)?.ok_or_else(|| not_found(id))
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
        self.migrate_legacy()?;
        if self.file(&id).is_file()
            && let Some(meta) = self.read_meta(&id)
        {
            return Ok(meta.into_entry(&id));
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
        self.write_meta(&entry)?;
        Ok(entry)
    }

    /// 修改名称与模型；不存在返回 `SKIN_NOT_FOUND`。
    pub fn update(&self, id: &str, name: &str, variant: SkinVariant) -> AppResult<SkinEntry> {
        let name = normalize_name(name)?;
        let _guard = Self::lock();
        self.migrate_legacy()?;
        let mut entry = self.entry(id)?.ok_or_else(|| not_found(id))?;
        entry.name = name;
        entry.variant = variant;
        self.write_meta(&entry)?;
        Ok(entry)
    }

    /// 删除皮肤的图片与描述文件；不存在返回 `SKIN_NOT_FOUND`。
    pub fn remove(&self, id: &str) -> AppResult<()> {
        let _guard = Self::lock();
        self.migrate_legacy()?;
        if self.entry(id)?.is_none() {
            return Err(not_found(id));
        }
        for path in [self.file(id), self.meta_file(id)] {
            match fs::remove_file(&path) {
                Err(e) if e.kind() != ErrorKind::NotFound => return Err(io_error(&path)(e)),
                _ => {}
            }
        }
        Ok(())
    }

    /// 把本库中 `target` 还没有的皮肤（按 ID）连同名称、模型复制过去，返回复制的数量。
    /// `target` 中已有的皮肤保持不变。
    pub fn copy_into(&self, target: &SkinLibrary) -> AppResult<usize> {
        let mut copied = 0;
        for entry in self.list()? {
            let _guard = Self::lock();
            if target.file(&entry.id).is_file() {
                if target.read_meta(&entry.id).is_none() {
                    target.write_meta(&entry)?;
                }
                continue;
            }
            let png = fs::read(self.file(&entry.id)).map_err(io_error(&self.file(&entry.id)))?;
            let file = target.file(&entry.id);
            write_atomic(&file, &png).map_err(|e| e.with_param("path", file.display()))?;
            target.write_meta(&entry)?;
            copied += 1;
        }
        Ok(copied)
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
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
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

    fn id_of(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[test]
    fn each_skin_has_its_own_meta_file() {
        let dir = tempfile::tempdir().unwrap();
        let lib = SkinLibrary::new(dir.path());
        let a = lib.add(&png(64, 64), "a", SkinVariant::Slim).unwrap();
        let meta: serde_json::Value =
            serde_json::from_slice(&fs::read(lib.meta_file(&a.id)).unwrap()).unwrap();
        assert_eq!(meta["schema"], 2);
        assert_eq!(meta["name"], "a");
        assert_eq!(meta["variant"], "slim");
        assert!(meta.get("id").is_none());
        lib.remove(&a.id).unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn legacy_index_is_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (png(64, 64), png(64, 32));
        let (id_a, id_b) = (id_of(&a), id_of(&b));
        fs::write(dir.path().join(format!("{id_a}.png")), &a).unwrap();
        fs::write(dir.path().join(format!("{id_b}.png")), &b).unwrap();
        let index = serde_json::json!({
            "schema": 1,
            "skins": [
                { "id": id_a, "name": "Steve", "variant": "classic", "height": 64, "added_at": 2 },
                { "id": id_b, "name": "Old", "variant": "slim", "height": 32, "added_at": 1 },
                { "id": "f".repeat(64), "name": "Gone", "variant": "slim", "height": 64, "added_at": 3 }
            ]
        });
        fs::write(dir.path().join("index.json"), index.to_string()).unwrap();

        let lib = SkinLibrary::new(dir.path());
        let skins = lib.list().unwrap();
        let names: Vec<_> = skins.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Steve", "Old"]);
        assert_eq!(skins[1].variant, SkinVariant::Slim);
        assert!(!dir.path().join("index.json").exists());
        assert!(dir.path().join("index.json.bak").is_file());
        assert!(lib.meta_file(&id_a).is_file() && lib.meta_file(&id_b).is_file());
        assert!(!lib.meta_file(&"f".repeat(64)).exists());
        assert_eq!(lib.list().unwrap(), skins, "迁移只做一次");
    }

    #[test]
    fn image_without_valid_meta_is_still_listed() {
        let dir = tempfile::tempdir().unwrap();
        let lib = SkinLibrary::new(dir.path());
        let bytes = png(64, 32);
        let id = id_of(&bytes);
        fs::write(lib.file(&id), &bytes).unwrap();
        let listed = lib.get(&id).unwrap();
        assert_eq!(
            (listed.name.as_str(), listed.variant, listed.height),
            (&id[..8], SkinVariant::Classic, 32)
        );
        fs::write(lib.meta_file(&id), b"{ half-synced").unwrap();
        assert_eq!(lib.get(&id).unwrap().name, &id[..8]);
        // 重新加入同一张图片时补上描述文件。
        let added = lib.add(&bytes, "Named", SkinVariant::Slim).unwrap();
        assert_eq!(added.name, "Named");
        assert_eq!(lib.get(&id).unwrap(), added);
        assert!(lib.update(&id, "Again", SkinVariant::Classic).is_ok());
    }

    #[test]
    fn foreign_and_invalid_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let lib = SkinLibrary::new(dir.path());
        let a = lib.add(&png(64, 64), "a", SkinVariant::Classic).unwrap();
        // 同步盘的冲突副本、用户随手放的文件、不合法的图片。
        fs::copy(
            lib.file(&a.id),
            dir.path().join(format!("{} (1).png", a.id)),
        )
        .unwrap();
        fs::write(dir.path().join("steve.png"), png(64, 64)).unwrap();
        fs::write(
            dir.path().join(format!("{}.png", "e".repeat(64))),
            b"not png",
        )
        .unwrap();
        fs::create_dir(dir.path().join(format!("{}.png", "d".repeat(64)))).unwrap();
        assert_eq!(lib.list().unwrap(), vec![a]);
    }

    #[test]
    fn two_installs_share_one_folder() {
        let dir = tempfile::tempdir().unwrap();
        let (first, second) = (SkinLibrary::new(dir.path()), SkinLibrary::new(dir.path()));
        let a = first.add(&png(64, 64), "a", SkinVariant::Classic).unwrap();
        let b = second.add(&png(64, 32), "b", SkinVariant::Slim).unwrap();
        assert_eq!(first.list().unwrap().len(), 2);
        second.update(&a.id, "renamed", SkinVariant::Slim).unwrap();
        assert_eq!(first.get(&a.id).unwrap().name, "renamed");
        first.remove(&b.id).unwrap();
        assert_eq!(second.list().unwrap().len(), 1);
    }

    #[test]
    fn copy_into_skips_existing() {
        let (src_dir, dst_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (src, dst) = (
            SkinLibrary::new(src_dir.path()),
            SkinLibrary::new(dst_dir.path().join("shared")),
        );
        let a = src.add(&png(64, 64), "a", SkinVariant::Slim).unwrap();
        let b = src.add(&png(64, 32), "b", SkinVariant::Classic).unwrap();
        let kept = dst.add(&png(64, 64), "kept", SkinVariant::Classic).unwrap();
        assert_eq!(kept.id, a.id);

        assert_eq!(src.copy_into(&dst).unwrap(), 1);
        assert_eq!(dst.get(&a.id).unwrap().name, "kept");
        assert_eq!(dst.get(&b.id).unwrap(), b);
        assert_eq!(src.list().unwrap().len(), 2, "源库保持不变");
        assert_eq!(src.copy_into(&dst).unwrap(), 0);
    }
}
