//! Modrinth 整合包（`.mrpack`）：读取并校验 `modrinth.index.json`，解压 `overrides/`。
//!
//! 格式见 <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>。
//! 包内路径一律视为不可信：必须是不含 `..`、盘符与反斜杠的相对路径，否则整个包被拒绝
//! （`files[].path`）或跳过该条目（`overrides/` 中的 zip 条目）。

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};
use tmcl_instance::{LoaderKind, LoaderSpec};
use tmcl_net::DownloadItem;
use ts_rs::TS;
use zip::ZipArchive;

use super::curseforge::ManualDownload;

/// 包内索引文件名。
pub const INDEX_FILE: &str = "modrinth.index.json";

/// 解压阶段名。
pub const OVERRIDES_STAGE: &str = "modpack_overrides";

/// 先解压 `overrides/`，再解压 `client-overrides/`（同名文件以后者为准）。
pub const OVERRIDE_DIRS: [&str; 2] = ["overrides", "client-overrides"];

/// 整合包格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ModpackFormat {
    /// Modrinth `.mrpack`。
    Mrpack,
    /// CurseForge zip（`manifest.json`）。
    Curseforge,
}

/// 整合包概要，用于安装前展示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ModpackInfo {
    /// 格式。
    pub format: ModpackFormat,
    /// 整合包名称。
    pub name: String,
    /// 整合包版本号（`versionId`）。
    pub version: String,
    /// 简介。
    pub summary: Option<String>,
    /// 游戏版本。
    pub game_version: String,
    /// 模组加载器，原版为 `null`。
    pub loader: Option<LoaderSpec>,
    /// 要下载的文件数（已排除仅服务端的文件）。
    pub file_count: u32,
    /// 要下载的总字节数。
    #[ts(type = "number")]
    pub download_size: u64,
    /// 作者禁止第三方分发、需要用户手动下载的文件（只有 CurseForge 整合包会有）。
    pub manual_downloads: Vec<ManualDownload>,
}

/// 要下载到实例 `game/` 下的一个文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile {
    /// 相对 `game/` 的路径，已校验。
    pub path: PathBuf,
    /// 下载地址（优先第一个 `https` 地址）。
    pub url: String,
    /// SHA1（小写十六进制）；为空表示来源没有提供，只按大小校验。
    pub sha1: String,
    /// 大小（字节）。
    pub size: u64,
}

impl PackFile {
    /// 下载到 `game_dir` 下的下载项。
    pub fn download_item(&self, game_dir: &Path) -> DownloadItem {
        DownloadItem {
            url: self.url.clone(),
            path: game_dir.join(&self.path),
            size: (self.size > 0).then_some(self.size),
            sha1: (!self.sha1.is_empty()).then(|| self.sha1.clone()),
        }
    }
}

/// 读取并校验后的整合包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mrpack {
    /// 概要。
    pub info: ModpackInfo,
    /// 要下载的文件。
    pub files: Vec<PackFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawIndex {
    format_version: u32,
    game: String,
    version_id: String,
    name: String,
    #[serde(default)]
    summary: Option<String>,
    files: Vec<RawFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFile {
    path: String,
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<RawEnv>,
    downloads: Vec<String>,
    file_size: u64,
}

#[derive(Deserialize)]
struct RawEnv {
    #[serde(default)]
    client: Option<String>,
}

/// 打开 `.mrpack` 并读取、校验索引。
///
/// 不是 zip、缺少或损坏索引、文件路径不安全、缺少 SHA1 或 http(s) 下载地址时返回
/// `RESOURCES_MODPACK_INVALID`；格式版本、游戏或依赖不受支持时返回 `RESOURCES_MODPACK_UNSUPPORTED`。
pub fn read(pack: &Path) -> AppResult<Mrpack> {
    let mut archive = open(pack)?;
    let mut text = String::new();
    archive
        .by_name(INDEX_FILE)
        .map_err(zip_invalid)?
        .read_to_string(&mut text)
        .map_err(|e| invalid(format!("{INDEX_FILE}: {e}")))?;
    parse_index(&text)
}

/// 解析并校验 `modrinth.index.json` 的内容，见 [`read`]。
pub fn parse_index(text: &str) -> AppResult<Mrpack> {
    let raw: RawIndex =
        serde_json::from_str(text).map_err(|e| invalid(format!("{INDEX_FILE}: {e}")))?;
    if raw.format_version != 1 {
        return Err(unsupported(format!("formatVersion {}", raw.format_version)));
    }
    if raw.game != "minecraft" {
        return Err(unsupported(raw.game));
    }

    let mut game_version = None;
    let mut loader = None;
    for (key, version) in raw.dependencies {
        if !is_safe_segment(&version) {
            return Err(invalid(format!("dependency {key}: {version}")));
        }
        let kind = match key.as_str() {
            "minecraft" => {
                game_version = Some(version);
                continue;
            }
            "forge" => LoaderKind::Forge,
            "neoforge" => LoaderKind::Neoforge,
            "fabric-loader" => LoaderKind::Fabric,
            "quilt-loader" => LoaderKind::Quilt,
            _ => return Err(unsupported(key)),
        };
        if loader.is_some() {
            return Err(unsupported(key));
        }
        loader = Some(LoaderSpec { kind, version });
    }
    let game_version = game_version.ok_or_else(|| invalid("missing minecraft dependency"))?;

    let mut files = Vec::new();
    for file in raw.files {
        if file.env.and_then(|e| e.client).as_deref() == Some("unsupported") {
            continue;
        }
        let path =
            safe_relative(&file.path).ok_or_else(|| invalid(format!("path {}", file.path)))?;
        let sha1 = file
            .hashes
            .get("sha1")
            .map(|h| h.to_ascii_lowercase())
            .filter(|h| h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| invalid(format!("sha1 of {}", file.path)))?;
        // 文件按 SHA1 校验，因此也接受 http；优先 https，拒绝其他协议。
        let url = file
            .downloads
            .iter()
            .find(|u| u.starts_with("https://"))
            .or_else(|| file.downloads.iter().find(|u| u.starts_with("http://")))
            .cloned()
            .ok_or_else(|| invalid(format!("downloads of {}", file.path)))?;
        files.push(PackFile {
            path,
            url,
            sha1,
            size: file.file_size,
        });
    }

    let info = ModpackInfo {
        format: ModpackFormat::Mrpack,
        name: raw.name,
        version: raw.version_id,
        summary: raw.summary.filter(|s| !s.trim().is_empty()),
        game_version,
        loader,
        file_count: u32::try_from(files.len()).unwrap_or(u32::MAX),
        download_size: files.iter().map(|f| f.size).sum(),
        manual_downloads: Vec::new(),
    };
    Ok(Mrpack { info, files })
}

/// 把 `overrides/` 与 `client-overrides/` 解压到 `game_dir`，返回写入的文件数。
///
/// 路径不安全的条目和符号链接被跳过；已存在的文件被覆盖。
pub fn extract_overrides(pack: &Path, game_dir: &Path, ctx: &TaskCtx) -> AppResult<u64> {
    extract_dirs(pack, &OVERRIDE_DIRS, game_dir, ctx)
}

/// 依次把包内 `dirs` 各目录的内容解压到 `game_dir`（同名文件以后面的目录为准），返回写入的文件数。
///
/// 路径不安全的条目和符号链接被跳过；已存在的文件被覆盖。
pub fn extract_dirs<S: AsRef<str>>(
    pack: &Path,
    dirs: &[S],
    game_dir: &Path,
    ctx: &TaskCtx,
) -> AppResult<u64> {
    let mut archive = open(pack)?;
    let mut targets = Vec::new();
    for prefix in dirs.iter().map(AsRef::as_ref) {
        targets.extend(override_entries(&mut archive, prefix)?);
    }
    let total = targets.iter().filter(|(_, _, is_dir)| !is_dir).count() as u64;
    let mut done = 0;
    ctx.report(ProgressUpdate::new(OVERRIDES_STAGE).items(0, total));
    for (index, rel, is_dir) in targets {
        ctx.check_cancelled()?;
        let target = game_dir.join(&rel);
        if is_dir {
            fs::create_dir_all(&target).map_err(|e| io_error(&target, &e))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| io_error(parent, &e))?;
        }
        let mut entry = archive.by_index(index).map_err(zip_invalid)?;
        let mut out = File::create(&target).map_err(|e| io_error(&target, &e))?;
        io::copy(&mut entry, &mut out).map_err(|e| invalid(format!("{}: {e}", rel.display())))?;
        done += 1;
        ctx.report(ProgressUpdate::new(OVERRIDES_STAGE).items(done, total));
    }
    Ok(done)
}

/// 前缀目录下的条目：(索引, 相对 `game/` 的路径, 是否目录)。
fn override_entries<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    prefix: &str,
) -> AppResult<Vec<(usize, PathBuf, bool)>> {
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(zip_invalid)?;
        let name = entry.name().map_err(zip_invalid)?;
        let Some(rest) = name.strip_prefix(prefix).and_then(|r| r.strip_prefix('/')) else {
            continue;
        };
        let rest = rest.trim_end_matches('/');
        if rest.is_empty() || entry.is_symlink() {
            continue;
        }
        let Some(rel) = safe_relative(rest) else {
            tracing::warn!(entry = %name, "跳过整合包中路径不安全的条目");
            continue;
        };
        entries.push((index, rel, entry.is_dir()));
    }
    Ok(entries)
}

/// 包内的 `/` 分隔相对路径 → 本地相对路径；含 `..`、`.`、空段、盘符、反斜杠或以 `/` 开头时返回 `None`。
pub fn safe_relative(path: &str) -> Option<PathBuf> {
    if path.is_empty() || path.contains(['\\', ':', '\0']) {
        return None;
    }
    let mut out = PathBuf::new();
    for part in path.split('/') {
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(name)), None) if name == part => out.push(name),
            _ => return None,
        }
    }
    Some(out)
}

/// 版本号会出现在 URL 与目录名中：拒绝空串、路径分隔符与 `..`。
pub(crate) fn is_safe_segment(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 64
        && !value.contains(['/', '\\', ':', '?', '#'])
        && !value.contains("..")
}

fn open(pack: &Path) -> AppResult<ZipArchive<BufReader<File>>> {
    let file = File::open(pack).map_err(|e| io_error(pack, &e))?;
    ZipArchive::new(BufReader::new(file)).map_err(zip_invalid)
}

fn invalid(detail: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ResourcesModpackInvalid).with_detail(detail.into())
}

fn unsupported(name: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ResourcesModpackUnsupported).with_param("name", name.into())
}

fn zip_invalid(err: zip::result::ZipError) -> AppError {
    invalid(err.to_string())
}

fn io_error(path: &Path, err: &io::Error) -> AppError {
    crate::install::io_error(path, err)
}
