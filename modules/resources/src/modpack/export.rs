//! 把实例的 `game/` 导出为 Modrinth 整合包（`.mrpack`）。
//!
//! 步骤：收集选中的 `game/` 顶层条目 → 计算 `mods/`、`resourcepacks/`、`shaderpacks/` 下文件的 SHA1
//! → 批量向 Modrinth 查询（`POST /version_files`）→ 查到的文件写成 `files[]` 下载链接，
//! 其余文件放进 `overrides/` → 写出 `<dest>.part`，完成后改名为 `dest`。失败或取消时删除半成品。

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tmcl_common::{AppError, AppResult, ProgressUpdate, TaskCtx};
use tmcl_instance::{LoaderKind, LoaderSpec};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::mrpack::{INDEX_FILE, safe_relative};
use crate::modrinth::{HashAlgorithm, ModrinthClient, VersionFile};

/// 计算哈希阶段名。
pub const HASH_STAGE: &str = "modpack_hash";

/// 查询 Modrinth 阶段名。
pub const LOOKUP_STAGE: &str = "modpack_lookup";

/// 打包阶段名。
pub const EXPORT_STAGE: &str = "modpack_export";

/// 这些目录下直接存放的、带对应扩展名的文件会先在 Modrinth 上按 SHA1 查找。
const LOOKUP_DIRS: [(&str, &str); 3] = [
    ("mods", ".jar"),
    ("resourcepacks", ".zip"),
    ("shaderpacks", ".zip"),
];

/// 每次批量查询的哈希数。
const LOOKUP_BATCH: usize = 500;

/// 导出选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModpackExport {
    /// 整合包名称。
    pub name: String,
    /// 整合包版本号（`versionId`）。
    pub version: String,
    /// 简介，空串视为没有。
    pub summary: Option<String>,
    /// 游戏版本。
    pub game_version: String,
    /// 模组加载器，原版为 `None`。
    pub loader: Option<LoaderSpec>,
    /// 要包含的 `game/` 顶层条目名，不存在的名字被忽略。
    pub include: Vec<String>,
}

/// 导出结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExportSummary {
    /// 写成 Modrinth 下载链接的文件数。
    pub linked: u32,
    /// 放进 `overrides/` 的文件数。
    pub overrides: u32,
}

/// 待打包的一个文件。
struct LocalFile {
    /// 相对 `game/` 的路径。
    rel: PathBuf,
    /// 包内路径（`/` 分隔）。
    name: String,
    size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Index<'a> {
    format_version: u32,
    game: &'a str,
    version_id: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<&'a str>,
    files: Vec<IndexFile>,
    dependencies: HashMap<&'static str, &'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    path: String,
    hashes: HashMap<&'static str, String>,
    downloads: Vec<String>,
    file_size: u64,
}

/// 把 `game_dir` 导出为 `dest`（`.mrpack`）。
///
/// 无法读取文件时返回 `COMMON_IO`；查询 Modrinth 失败时返回网络错误，不会退化为把所有文件
/// 塞进 `overrides/`。
pub async fn export(
    client: &ModrinthClient,
    game_dir: &Path,
    dest: &Path,
    pack: &ModpackExport,
    ctx: &TaskCtx,
) -> AppResult<ExportSummary> {
    let part = part_path(dest);
    let result = run(client, game_dir, dest, &part, pack, ctx).await;
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

async fn run(
    client: &ModrinthClient,
    game_dir: &Path,
    dest: &Path,
    part: &Path,
    pack: &ModpackExport,
    ctx: &TaskCtx,
) -> AppResult<ExportSummary> {
    let files = {
        let (game_dir, include) = (game_dir.to_owned(), pack.include.clone());
        let skip = [dest.to_owned(), part.to_owned()];
        blocking(move || collect(&game_dir, &include, &skip)).await?
    };

    let candidates: Vec<&LocalFile> = files.iter().filter(|f| is_candidate(&f.name)).collect();
    let total = candidates.len() as u64;
    ctx.report(ProgressUpdate::new(HASH_STAGE).items(0, total));
    let mut hashes = HashMap::new();
    for (done, file) in candidates.iter().enumerate() {
        ctx.check_cancelled()?;
        let sha1 = tmcl_net::sha1_file(&game_dir.join(&file.rel)).await?;
        hashes.insert(file.name.clone(), sha1);
        ctx.report(ProgressUpdate::new(HASH_STAGE).items(done as u64 + 1, total));
    }

    let linked = lookup(client, hashes, ctx).await?;
    let (index_files, overrides): (Vec<_>, Vec<_>) = files
        .into_iter()
        .partition(|f| linked.contains_key(&f.name));
    let summary = ExportSummary {
        linked: count(index_files.len()),
        overrides: count(overrides.len()),
    };

    let mut entries: Vec<IndexFile> = index_files
        .iter()
        .filter_map(|f| linked.get(&f.name).map(|v| index_file(&f.name, v)))
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let index = index_json(pack, entries)?;

    {
        let (game_dir, part, ctx) = (game_dir.to_owned(), part.to_owned(), ctx.clone());
        blocking(move || write_zip(&part, &index, &game_dir, &overrides, &ctx)).await?;
    }
    fs::rename(part, dest).map_err(|e| io_error(dest, &e))?;
    tracing::info!(
        linked = summary.linked,
        overrides = summary.overrides,
        "已导出整合包"
    );
    Ok(summary)
}

/// 按 SHA1 批量查询，返回“包内路径 → Modrinth 上的同一文件”。
async fn lookup(
    client: &ModrinthClient,
    hashes: HashMap<String, String>,
    ctx: &TaskCtx,
) -> AppResult<HashMap<String, VersionFile>> {
    let all: Vec<String> = hashes.values().cloned().collect();
    let batches = all.len().div_ceil(LOOKUP_BATCH) as u64;
    let mut found = HashMap::new();
    ctx.report(ProgressUpdate::new(LOOKUP_STAGE).items(0, batches));
    for (done, batch) in all.chunks(LOOKUP_BATCH).enumerate() {
        ctx.check_cancelled()?;
        let versions = client
            .versions_from_hashes(batch, HashAlgorithm::Sha1, ctx)
            .await?;
        for (hash, version) in versions {
            let file = version
                .files
                .into_iter()
                .find(|f| f.hashes.sha1.eq_ignore_ascii_case(&hash));
            if let Some(file) = file.filter(is_linkable) {
                found.insert(hash.to_ascii_lowercase(), file);
            }
        }
        ctx.report(ProgressUpdate::new(LOOKUP_STAGE).items(done as u64 + 1, batches));
    }
    Ok(hashes
        .into_iter()
        .filter_map(|(name, hash)| found.get(&hash).map(|f| (name, f.clone())))
        .collect())
}

/// `.mrpack` 要求 SHA1 与 SHA512，下载地址必须是 https。
fn is_linkable(file: &VersionFile) -> bool {
    file.url.starts_with("https://")
        && file.hashes.sha1.len() == 40
        && file.hashes.sha512.len() == 128
}

fn is_candidate(name: &str) -> bool {
    LOOKUP_DIRS.iter().any(|(dir, ext)| {
        name.strip_prefix(dir)
            .and_then(|r| r.strip_prefix('/'))
            .is_some_and(|r| !r.contains('/') && r.to_ascii_lowercase().ends_with(ext))
    })
}

fn index_file(path: &str, file: &VersionFile) -> IndexFile {
    IndexFile {
        path: path.to_owned(),
        hashes: HashMap::from([
            ("sha1", file.hashes.sha1.to_ascii_lowercase()),
            ("sha512", file.hashes.sha512.to_ascii_lowercase()),
        ]),
        downloads: vec![file.url.clone()],
        file_size: file.size,
    }
}

fn index_json(pack: &ModpackExport, files: Vec<IndexFile>) -> AppResult<String> {
    let mut dependencies = HashMap::from([("minecraft", pack.game_version.as_str())]);
    if let Some(loader) = &pack.loader {
        dependencies.insert(dependency_key(loader.kind), loader.version.as_str());
    }
    let index = Index {
        format_version: 1,
        game: "minecraft",
        version_id: &pack.version,
        name: &pack.name,
        summary: pack.summary.as_deref().filter(|s| !s.trim().is_empty()),
        files,
        dependencies,
    };
    serde_json::to_string_pretty(&index).map_err(AppError::from)
}

/// 加载器在 `dependencies` 中的键名，与 [`super::mrpack::parse_index`] 对应。
fn dependency_key(kind: LoaderKind) -> &'static str {
    match kind {
        LoaderKind::Forge => "forge",
        LoaderKind::Neoforge => "neoforge",
        LoaderKind::Fabric => "fabric-loader",
        LoaderKind::Quilt => "quilt-loader",
    }
}

/// 收集 `root` 下顶层名在 `include` 中的所有文件，按包内路径排序。
///
/// 符号链接、`skip` 中的文件（导出目标本身），以及包内路径不安全（含反斜杠、冒号等，导入时会被拒绝）
/// 的文件被跳过。
fn collect(root: &Path, include: &[String], skip: &[PathBuf]) -> AppResult<Vec<LocalFile>> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = read_dir_names(root)?
        .into_iter()
        .filter(|n| include.contains(n))
        .map(PathBuf::from)
        .collect();
    while let Some(rel) = stack.pop() {
        let path = root.join(&rel);
        let meta = fs::symlink_metadata(&path).map_err(|e| io_error(&path, &e))?;
        if meta.is_dir() {
            stack.extend(read_dir_names(&path)?.into_iter().map(|n| rel.join(n)));
        } else if meta.is_file() && !skip.contains(&path) {
            let name = pack_name(&rel);
            if safe_relative(&name).is_none() {
                tracing::warn!(path = %name, "跳过路径不能放进整合包的文件");
                continue;
            }
            out.push(LocalFile {
                rel,
                name,
                size: meta.len(),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// 列出目录下名称为 UTF-8 的条目；目录不存在时返回空列表。
fn read_dir_names(dir: &Path) -> AppResult<Vec<String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_error(dir, &e)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io_error(dir, &e))?;
        match entry.file_name().into_string() {
            Ok(name) => names.push(name),
            Err(name) => tracing::warn!(?name, "跳过名称不是 UTF-8 的文件"),
        }
    }
    Ok(names)
}

fn write_zip(
    part: &Path,
    index: &str,
    game_dir: &Path,
    overrides: &[LocalFile],
    ctx: &TaskCtx,
) -> AppResult<()> {
    let total = overrides.len() as u64;
    let bytes_total = overrides.iter().map(|f| f.size).sum();
    let mut bytes_done = 0;
    let file = File::create(part).map_err(|e| io_error(part, &e))?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file(INDEX_FILE, options).map_err(zip_error)?;
    io::Write::write_all(&mut zip, index.as_bytes()).map_err(|e| io_error(part, &e))?;
    ctx.report(
        ProgressUpdate::new(EXPORT_STAGE)
            .items(0, total)
            .bytes(0, bytes_total),
    );
    for (done, local) in overrides.iter().enumerate() {
        ctx.check_cancelled()?;
        let options = options.large_file(local.size > u64::from(u32::MAX));
        zip.start_file(format!("overrides/{}", local.name), options)
            .map_err(zip_error)?;
        let path = game_dir.join(&local.rel);
        let mut src = File::open(&path).map_err(|e| io_error(&path, &e))?;
        io::copy(&mut src, &mut zip).map_err(|e| io_error(&path, &e))?;
        bytes_done += local.size;
        ctx.report(
            ProgressUpdate::new(EXPORT_STAGE)
                .items(done as u64 + 1, total)
                .bytes(bytes_done, bytes_total),
        );
    }
    let mut writer = zip.finish().map_err(zip_error)?;
    io::Write::flush(&mut writer).map_err(|e| io_error(part, &e))?;
    writer
        .into_inner()
        .map_err(|e| io_error(part, e.error()))?
        .sync_all()
        .map_err(|e| io_error(part, &e))
}

fn pack_name(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

async fn blocking<T, F>(f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|e| {
        AppError::new(tmcl_common::ErrorCode::CommonInternal).with_detail(e.to_string())
    })?
}

fn zip_error(err: zip::result::ZipError) -> AppError {
    AppError::new(tmcl_common::ErrorCode::CommonIo).with_detail(err.to_string())
}

fn io_error(path: &Path, err: &io::Error) -> AppError {
    crate::install::io_error(path, err)
}
