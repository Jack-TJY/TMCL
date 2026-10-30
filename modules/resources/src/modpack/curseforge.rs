//! CurseForge 整合包（zip，根目录 `manifest.json` + `overrides/`）。
//!
//! `manifest.json` 只列出 `projectID` / `fileID`，下载地址、SHA1 与目标目录要向 CurseForge API
//! 查询（[`resolve`]）。作者禁止第三方分发的文件（`downloadUrl` 为空）不下载，列入
//! 依次尝试：Modrinth 上 SHA1 相同的同一文件 → CurseForge 文件镜像（[`MIRROR_BASE`]，
//! 维护者已知悉这会绕过作者的分发设置）→ 列入 [`ManualDownload`]，由界面提供网页链接让用户
//! 手动下载后放进对应目录。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_instance::{LoaderKind, LoaderSpec};
use ts_rs::TS;
use zip::ZipArchive;

use super::mrpack::{PackFile, is_safe_segment, safe_relative};
use crate::curseforge::{ClassId, CurseForgeClient, Mod};
use crate::modrinth::{HashAlgorithm, ModrinthClient};

/// 包内清单文件名。
pub const MANIFEST_FILE: &str = "manifest.json";

/// 每次批量查询的 ID 数。
const LOOKUP_BATCH: usize = 500;

/// 需要用户手动下载的文件（作者禁止第三方分发）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ManualDownload {
    /// 项目名称。
    pub project_name: String,
    /// 文件名。
    pub file_name: String,
    /// 文件的 CurseForge 网页，用户在这里下载。
    pub page_url: String,
    /// 下载后应放进的目录，相对实例的 `game/`，如 `mods`。
    pub target_dir: String,
}

/// 读取并校验后的清单。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// 整合包名称。
    pub name: String,
    /// 整合包版本号。
    pub version: String,
    /// 作者。
    pub author: String,
    /// 游戏版本。
    pub game_version: String,
    /// 模组加载器，原版为 `None`。
    pub loader: Option<LoaderSpec>,
    /// 必需文件的 `(projectID, fileID)`；`required: false` 的可选文件已排除。
    pub files: Vec<(u32, u32)>,
    /// 覆盖文件目录名（通常为 `overrides`）。
    pub overrides: String,
}

/// 作者禁止第三方分发的文件改从哪里下载。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum DownloadVia {
    /// Modrinth 上 SHA1 相同的同一文件。
    Modrinth,
    /// CurseForge 镜像（MCIM）；镜像不可用时需要手动下载。
    Mirror,
}

/// 作者禁止第三方分发、改从其他来源下载的文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RedirectedDownload {
    /// 文件信息（镜像下载失败时按它手动下载）。
    pub file: ManualDownload,
    /// 下载来源。
    pub via: DownloadVia,
}

/// 作者禁止第三方分发的文件的备选来源，按顺序尝试；都为 `None` 时全部列为手动下载。
#[derive(Debug, Clone, Default)]
pub struct Fallbacks {
    /// 按 SHA1 在 Modrinth 查找同一文件。
    pub modrinth: Option<ModrinthClient>,
    /// CurseForge 文件镜像的根地址，如 [`MIRROR_BASE`]。
    pub mirror: Option<String>,
}

/// MCIM 提供的 CurseForge 文件镜像（`edge.forgecdn.net` → `mod.mcimirror.top`）。
pub const MIRROR_BASE: &str = "https://mod.mcimirror.top";

/// 按 API 解析出的文件。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Resolved {
    /// 必须下载成功的文件（含从 Modrinth 补下载的）。
    pub files: Vec<PackFile>,
    /// 从镜像尝试下载的文件；失败时只记录，按对应的说明手动下载。
    pub optional: Vec<(PackFile, ManualDownload)>,
    /// 改从 Modrinth 或镜像下载的文件。
    pub redirected: Vec<RedirectedDownload>,
    /// 需要手动下载的文件。
    pub manual: Vec<ManualDownload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawManifest {
    minecraft: RawMinecraft,
    manifest_type: String,
    manifest_version: u32,
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    files: Vec<RawFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<RawLoader>,
}

#[derive(Deserialize)]
struct RawLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(rename = "projectID")]
    project_id: u32,
    #[serde(rename = "fileID")]
    file_id: u32,
    #[serde(default = "default_true")]
    required: bool,
}

fn default_true() -> bool {
    true
}

/// 读取 zip 中的 `manifest.json`；不是 zip 或损坏时返回 `RESOURCES_MODPACK_INVALID`，
/// 没有清单时返回 `None`。
pub fn read(pack: &Path) -> AppResult<Option<Manifest>> {
    let file = File::open(pack).map_err(|e| crate::install::io_error(pack, &e))?;
    let mut archive = ZipArchive::new(BufReader::new(file)).map_err(|e| invalid(e.to_string()))?;
    let mut entry = match archive.by_name(MANIFEST_FILE) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(invalid(e.to_string())),
    };
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .map_err(|e| invalid(format!("{MANIFEST_FILE}: {e}")))?;
    parse_manifest(&text).map(Some)
}

/// 解析并校验 `manifest.json` 的内容。
///
/// 格式错误返回 `RESOURCES_MODPACK_INVALID`；清单类型、版本或加载器不受支持时返回
/// `RESOURCES_MODPACK_UNSUPPORTED`。
pub fn parse_manifest(text: &str) -> AppResult<Manifest> {
    // 有些整合包带 UTF-8 BOM。
    let text = text.trim_start_matches('\u{feff}');
    let raw: RawManifest =
        serde_json::from_str(text).map_err(|e| invalid(format!("{MANIFEST_FILE}: {e}")))?;
    if raw.manifest_type != "minecraftModpack" {
        return Err(unsupported(raw.manifest_type));
    }
    if raw.manifest_version != 1 {
        return Err(unsupported(format!(
            "manifestVersion {}",
            raw.manifest_version
        )));
    }
    if !is_safe_segment(&raw.minecraft.version) {
        return Err(invalid(format!("minecraft {}", raw.minecraft.version)));
    }
    let loader = raw
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| raw.minecraft.mod_loaders.first())
        .map(|l| parse_loader(&l.id))
        .transpose()?;
    let overrides = raw
        .overrides
        .map(|o| o.trim().trim_matches('/').to_owned())
        .filter(|o| !o.is_empty())
        .unwrap_or_else(|| "overrides".to_owned());
    if safe_relative(&overrides).is_none() {
        return Err(invalid(format!("overrides {overrides}")));
    }
    let mut files = Vec::new();
    for file in raw.files.into_iter().filter(|f| f.required) {
        let id = (file.project_id, file.file_id);
        if !files.contains(&id) {
            files.push(id);
        }
    }
    Ok(Manifest {
        name: raw.name,
        version: raw.version,
        author: raw.author,
        game_version: raw.minecraft.version,
        loader,
        files,
        overrides,
    })
}

/// `forge-47.2.0` → Forge 47.2.0。
fn parse_loader(id: &str) -> AppResult<LoaderSpec> {
    let (name, version) = id
        .split_once('-')
        .ok_or_else(|| invalid(format!("modLoader {id}")))?;
    let kind = match name.to_ascii_lowercase().as_str() {
        "forge" => LoaderKind::Forge,
        "neoforge" => LoaderKind::Neoforge,
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        _ => return Err(unsupported(name)),
    };
    if !is_safe_segment(version) {
        return Err(invalid(format!("modLoader {id}")));
    }
    Ok(LoaderSpec {
        kind,
        version: version.to_owned(),
    })
}

/// 向 CurseForge 查询清单中的文件，得到下载项与需要手动下载的文件。
///
/// 清单中的文件在 CurseForge 上找不到（已删除）时返回 `RESOURCES_VERSION_NOT_FOUND`。
pub async fn resolve(
    manifest: &Manifest,
    client: &CurseForgeClient,
    fallbacks: &Fallbacks,
    ctx: &TaskCtx,
) -> AppResult<Resolved> {
    let file_ids: Vec<u32> = manifest.files.iter().map(|&(_, f)| f).collect();
    let mut files = HashMap::new();
    for chunk in file_ids.chunks(LOOKUP_BATCH) {
        for file in client.files(chunk, ctx).await? {
            files.insert(file.id, file);
        }
    }
    let mut project_ids: Vec<u32> = manifest.files.iter().map(|&(p, _)| p).collect();
    project_ids.sort_unstable();
    project_ids.dedup();
    let mut projects: HashMap<u32, Mod> = HashMap::new();
    for chunk in project_ids.chunks(LOOKUP_BATCH) {
        for project in client.projects(chunk, ctx).await? {
            projects.insert(project.id, project);
        }
    }

    let mut out = Resolved::default();
    // 作者禁止第三方分发的文件：(包内路径, 文件, 需手动下载时的说明)。
    let mut blocked = Vec::new();
    for &(project_id, file_id) in &manifest.files {
        let file = files
            .get(&file_id)
            .filter(|f| f.mod_id == project_id)
            .ok_or_else(|| {
                AppError::new(ErrorCode::ResourcesVersionNotFound).with_param("version", file_id)
            })?;
        let project = projects.get(&project_id);
        let class = project.and_then(|p| p.class_id);
        let dir = target_dir(class);
        let rel = format!("{dir}/{}", file.file_name);
        let path = safe_relative(&rel).ok_or_else(|| invalid(format!("file {rel}")))?;
        match file.usable_download_url() {
            Some(url) => out.files.push(PackFile {
                path,
                url: url.to_owned(),
                sha1: file.sha1().unwrap_or_default(),
                size: file.file_length,
            }),
            None => {
                let page_url = match project {
                    Some(p) => p.file_page_url(file_id),
                    None => format!("https://www.curseforge.com/projects/{project_id}"),
                };
                let manual = ManualDownload {
                    project_name: project
                        .map_or_else(|| file.display_name.clone(), |p| p.name.clone()),
                    file_name: file.file_name.clone(),
                    page_url,
                    target_dir: dir.to_owned(),
                };
                blocked.push((path, file, manual));
            }
        }
    }

    // 1. 同一文件（SHA1 相同）也发布在 Modrinth 上的，从 Modrinth 下载。
    let mut on_modrinth = HashMap::new();
    if let Some(modrinth) = &fallbacks.modrinth {
        let hashes: Vec<String> = blocked.iter().filter_map(|(_, f, _)| f.sha1()).collect();
        for chunk in hashes.chunks(LOOKUP_BATCH) {
            match modrinth
                .versions_from_hashes(chunk, HashAlgorithm::Sha1, ctx)
                .await
            {
                Ok(found) => {
                    for (hash, version) in found {
                        let hash = hash.to_ascii_lowercase();
                        let file = version
                            .files
                            .into_iter()
                            .find(|f| f.hashes.sha1.eq_ignore_ascii_case(&hash))
                            // 下载后按 SHA1 校验，http 也可以接受。
                            .filter(|f| f.url.starts_with("https://") || f.url.starts_with("http://"));
                        if let Some(file) = file {
                            on_modrinth.insert(hash, file.url);
                        }
                    }
                }
                Err(err) if err.is_cancelled() => return Err(err),
                // Modrinth 查询失败不影响安装，相关文件继续走镜像或手动下载。
                Err(err) => tracing::warn!(%err, "在 Modrinth 按哈希查找禁止分发的文件失败"),
            }
        }
    }

    for (path, file, manual) in blocked {
        let sha1 = file.sha1();
        if let Some(url) = sha1.as_ref().and_then(|h| on_modrinth.get(h)) {
            out.files.push(PackFile {
                path,
                url: url.clone(),
                sha1: sha1.clone().unwrap_or_default(),
                size: file.file_length,
            });
            out.redirected.push(RedirectedDownload {
                file: manual,
                via: DownloadVia::Modrinth,
            });
            continue;
        }
        // 2. 镜像：只用于有 SHA1 的文件，下载后按 SHA1 校验；镜像失败时不影响安装。
        let mirror = fallbacks
            .mirror
            .as_deref()
            .zip(sha1)
            .and_then(|(base, sha1)| Some((mirror_url(base, file.id, &file.file_name)?, sha1)));
        match mirror {
            Some((url, sha1)) => {
                out.optional.push((
                    PackFile {
                        path,
                        url,
                        sha1,
                        size: file.file_length,
                    },
                    manual.clone(),
                ));
                out.redirected.push(RedirectedDownload {
                    file: manual,
                    via: DownloadVia::Mirror,
                });
            }
            // 3. 都没有：手动下载。
            None => out.manual.push(manual),
        }
    }
    Ok(out)
}

/// 镜像上的文件地址：`<base>/files/<id 前 4 位>/<id 后 3 位，去掉前导 0>/<文件名>`，
/// 与 `edge.forgecdn.net` 的路径规则相同。
pub fn mirror_url(base: &str, file_id: u32, file_name: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(base).ok()?;
    url.path_segments_mut().ok()?.pop_if_empty().extend([
        "files",
        &(file_id / 1000).to_string(),
        &(file_id % 1000).to_string(),
        file_name,
    ]);
    Some(url.into())
}

/// 文件按项目的顶级类别放进 `game/` 下的目录；未知类别按 mod 处理。
fn target_dir(class: Option<ClassId>) -> &'static str {
    match class {
        Some(ClassId::ResourcePacks) => "resourcepacks",
        Some(ClassId::Shaders) => "shaderpacks",
        _ => "mods",
    }
}

/// 包内 `<overrides>/` 前缀，见 [`super::mrpack::extract_dirs`]。
pub fn override_dirs(manifest: &Manifest) -> Vec<String> {
    vec![manifest.overrides.clone()]
}

fn invalid(detail: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ResourcesModpackInvalid).with_detail(detail.into())
}

fn unsupported(name: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ResourcesModpackUnsupported).with_param("name", name.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
        "minecraft": {
            "version": "1.20.1",
            "modLoaders": [{ "id": "forge-47.2.0", "primary": true }]
        },
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": "Test Pack",
        "version": "1.0.0",
        "author": "someone",
        "files": [
            { "projectID": 238222, "fileID": 9352806, "required": true },
            { "projectID": 1, "fileID": 2, "required": false },
            { "projectID": 238222, "fileID": 9352806, "required": true }
        ],
        "overrides": "overrides"
    }"#;

    #[test]
    fn parses_manifest_and_skips_optional_files() {
        let manifest = parse_manifest(MANIFEST).unwrap();
        assert_eq!(manifest.name, "Test Pack");
        assert_eq!(manifest.game_version, "1.20.1");
        assert_eq!(
            manifest.loader,
            Some(LoaderSpec {
                kind: LoaderKind::Forge,
                version: "47.2.0".into()
            })
        );
        assert_eq!(manifest.files, vec![(238_222, 9_352_806)]);
        assert_eq!(override_dirs(&manifest), vec!["overrides".to_owned()]);
    }

    #[test]
    fn accepts_bom_and_vanilla() {
        let text = "\u{feff}{\"minecraft\":{\"version\":\"1.12.2\"},\"manifestType\":\"minecraftModpack\",\"manifestVersion\":1,\"name\":\"V\"}";
        let manifest = parse_manifest(text).unwrap();
        assert_eq!(manifest.loader, None);
        assert_eq!(manifest.overrides, "overrides");
    }

    #[test]
    fn parses_each_loader_kind() {
        for (id, kind) in [
            ("neoforge-21.1.77", LoaderKind::Neoforge),
            ("fabric-0.16.9", LoaderKind::Fabric),
            ("quilt-0.27.1", LoaderKind::Quilt),
        ] {
            assert_eq!(parse_loader(id).unwrap().kind, kind);
        }
        let err = parse_loader("rift-1.0").unwrap_err();
        assert_eq!(err.code, ErrorCode::ResourcesModpackUnsupported);
        assert_eq!(
            parse_loader("forge-../x").unwrap_err().code,
            ErrorCode::ResourcesModpackInvalid
        );
    }

    #[test]
    fn rejects_unsafe_overrides_and_wrong_type() {
        let bad = MANIFEST.replace("\"overrides\": \"overrides\"", "\"overrides\": \"../x\"");
        assert_eq!(
            parse_manifest(&bad).unwrap_err().code,
            ErrorCode::ResourcesModpackInvalid
        );
        let other = MANIFEST.replace("minecraftModpack", "somethingElse");
        assert_eq!(
            parse_manifest(&other).unwrap_err().code,
            ErrorCode::ResourcesModpackUnsupported
        );
    }

    #[test]
    fn class_decides_target_dir() {
        assert_eq!(target_dir(Some(ClassId::Mods)), "mods");
        assert_eq!(target_dir(Some(ClassId::ResourcePacks)), "resourcepacks");
        assert_eq!(target_dir(Some(ClassId::Shaders)), "shaderpacks");
        assert_eq!(target_dir(None), "mods");
    }
}
