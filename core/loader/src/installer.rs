//! Forge / NeoForge 安装器 jar 的通用安装流程。
//!
//! 两种 `install_profile.json`：
//!
//! - 旧格式（Forge 1.7.10 至 1.12.2 的早期构建）：`install` + `versionInfo`，`versionInfo` 就是版本 JSON，
//!   通用 jar 放在安装器根目录（`install.filePath`），复制到 `install.path` 坐标对应的位置；
//! - 新格式（`spec` 0/1）：版本 JSON 在 `json` 指向的条目中，安装器内 `maven/` 下的构件直接解压，
//!   `libraries` 列出处理步骤需要的库，`processors` 生成补丁后的客户端。
//!
//! 安装器下载到 `cache/loader/` 下的临时目录，结束后删除。

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};
use tmcl_game::{GameFiles, Library, maven_path};
use tmcl_net::DownloadItem;

use crate::processors::{Processor, ProcessorEnv, Variables, run_processors};
use crate::profile::{
    add, check_local_files, download_libraries, install_game, invalid, not_found_on_missing,
    prepare_json, write_version,
};
use crate::{InstalledLoader, LoaderInstaller, LoaderKind, version_id};

#[derive(Deserialize)]
struct LegacyProfile {
    install: LegacyInstall,
    #[serde(rename = "versionInfo")]
    version_info: Value,
}

#[derive(Deserialize)]
struct LegacyInstall {
    path: String,
    #[serde(rename = "filePath")]
    file_path: String,
    minecraft: String,
}

#[derive(Deserialize)]
struct NewProfile {
    minecraft: String,
    json: String,
    #[serde(default)]
    data: BTreeMap<String, SidedValue>,
    #[serde(default)]
    processors: Vec<Processor>,
    #[serde(default)]
    libraries: Vec<Library>,
}

#[derive(Deserialize)]
struct SidedValue {
    client: String,
}

/// 安装器 jar（zip）的只读访问，每次操作重新打开文件，供 `spawn_blocking` 使用。
struct InstallerJar(PathBuf);

impl InstallerJar {
    fn archive(&self) -> Result<zip::ZipArchive<std::fs::File>, String> {
        let file = std::fs::File::open(&self.0).map_err(|e| e.to_string())?;
        zip::ZipArchive::new(file).map_err(|e| e.to_string())
    }

    /// 读取条目；不存在时为 `None`。
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let mut archive = self.archive()?;
        let mut entry = match archive.by_name(name.trim_start_matches('/')) {
            Ok(entry) => entry,
            Err(zip::result::ZipError::FileNotFound) => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        Ok(Some(bytes))
    }

    /// 把条目写到 `dest`。
    fn extract(&self, name: &str, dest: &Path) -> Result<(), String> {
        let bytes = self
            .read(name)?
            .ok_or_else(|| format!("{name} not found in installer"))?;
        write_file(dest, &bytes)
    }

    /// 把 `prefix` 下的全部文件解压到 `dest` 下（去掉前缀），跳过不安全的路径。
    fn extract_dir(&self, prefix: &str, dest: &Path) -> Result<usize, String> {
        let mut archive = self.archive()?;
        let mut count = 0;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            let Some(rel) = entry
                .enclosed_name()
                .and_then(|p| p.strip_prefix(prefix).ok().map(Path::to_path_buf))
            else {
                continue;
            };
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            write_file(&dest.join(rel), &bytes)?;
            count += 1;
        }
        Ok(count)
    }
}

fn write_file(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(dest, bytes).map_err(|e| format!("{}: {e}", dest.display()))
}

/// 在线程池中操作安装器 jar，失败时返回 `LOADER_INSTALLER_INVALID`。
async fn with_jar<T: Send + 'static>(
    jar: &Path,
    kind: LoaderKind,
    version: &str,
    f: impl FnOnce(&InstallerJar) -> Result<T, String> + Send + 'static,
) -> AppResult<T> {
    let jar = InstallerJar(jar.to_path_buf());
    tokio::task::spawn_blocking(move || f(&jar))
        .await
        .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?
        .map_err(|e| invalid(kind, version, e))
}

/// 本次安装的临时目录，离开作用域时删除。
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 下载 `url` 处的安装器并安装 `kind` `loader_version`，必要时先补全原版。
///
/// 阶段依次为原版补全的各阶段、`loader_installer`、`loader_libraries`、`loader_processors`。
/// 安装器不存在（HTTP 400/404）时返回 `LOADER_VERSION_NOT_FOUND`；失败或取消时不写入版本 JSON。
pub(crate) async fn install_from_installer(
    installer: &LoaderInstaller,
    kind: LoaderKind,
    game_version: &str,
    loader_version: &str,
    url: &str,
    ctx: &TaskCtx,
) -> AppResult<InstalledLoader> {
    let id = version_id(kind, game_version, loader_version);
    let mut report = install_game(installer, game_version, ctx).await?;
    let files = GameFiles::new(installer.paths().clone());

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let temp = TempDir(
        installer
            .paths()
            .cache_dir()
            .join("loader")
            .join(format!("{id}-{}-{nanos}", std::process::id())),
    );
    tokio::fs::create_dir_all(&temp.0).await?;
    let jar = temp.0.join("installer.jar");

    ctx.report(ProgressUpdate::new("loader_installer").items(0, 1));
    let item = DownloadItem {
        url: url.to_owned(),
        path: jar.clone(),
        size: None,
        sha1: None,
    };
    installer
        .downloader()
        .download_one(&item, ctx)
        .await
        .map_err(|e| not_found_on_missing(e, kind, game_version, loader_version))?;
    let profile = with_jar(&jar, kind, loader_version, |j| {
        j.read("install_profile.json")?
            .ok_or_else(|| "install_profile.json not found".to_owned())
    })
    .await?;
    ctx.report(ProgressUpdate::new("loader_installer").items(1, 1));

    let check_game = |minecraft: &str| {
        if minecraft == game_version {
            Ok(())
        } else {
            Err(invalid(
                kind,
                loader_version,
                format!("installer is for {minecraft}, expected {game_version}"),
            ))
        }
    };

    let value: Value = serde_json::from_slice(&profile)
        .map_err(|e| invalid(kind, loader_version, e.to_string()))?;
    let version_value = if value.get("install").is_some() {
        let legacy: LegacyProfile = serde_json::from_value(value)
            .map_err(|e| invalid(kind, loader_version, e.to_string()))?;
        check_game(&legacy.install.minecraft)?;
        let path = maven_path(&legacy.install.path).ok_or_else(|| {
            invalid(
                kind,
                loader_version,
                format!("bad path {}", legacy.install.path),
            )
        })?;
        let dest = files.library(&path);
        let entry = legacy.install.file_path.clone();
        with_jar(&jar, kind, loader_version, move |j| {
            j.extract(&entry, &dest)
        })
        .await?;
        let (value, version) = prepare_json(
            kind,
            game_version,
            loader_version,
            &id,
            &serde_json::to_vec(&legacy.version_info)?,
        )?;
        add(
            &mut report,
            download_libraries(installer, &version, "loader_libraries", ctx).await?,
        );
        check_local_files(installer, &version).await?;
        value
    } else {
        let profile: NewProfile = serde_json::from_value(value)
            .map_err(|e| invalid(kind, loader_version, e.to_string()))?;
        check_game(&profile.minecraft)?;
        let entry = profile.json.clone();
        let bytes = with_jar(&jar, kind, loader_version, move |j| {
            j.read(&entry)?
                .ok_or_else(|| format!("{entry} not found in installer"))
        })
        .await?;
        let (value, version) = prepare_json(kind, game_version, loader_version, &id, &bytes)?;

        // 安装器自带的构件（如 1.12.2–1.16.5 的 Forge 本体）先解压，已存在的会被覆盖为安装器中的版本。
        let libraries = files.libraries_dir();
        with_jar(&jar, kind, loader_version, move |j| {
            j.extract_dir("maven", &libraries)
        })
        .await?;
        let mut items: Vec<DownloadItem> = profile
            .libraries
            .iter()
            .filter_map(|l| l.file())
            .filter_map(|f| {
                Some(DownloadItem {
                    url: f.url?,
                    path: files.library(&f.path),
                    size: f.size.filter(|&s| s > 0),
                    sha1: f.sha1,
                })
            })
            .collect();
        items.dedup_by(|a, b| a.path == b.path);
        add(
            &mut report,
            installer
                .downloader()
                .download_all(items, "loader_libraries", ctx)
                .await?,
        );
        add(
            &mut report,
            download_libraries(installer, &version, "loader_libraries", ctx).await?,
        );

        if profile.processors.iter().any(Processor::runs_on_client) {
            let vars = processor_variables(
                &profile,
                &files,
                &jar,
                &temp.0,
                game_version,
                kind,
                loader_version,
            )
            .await?;
            let parent = files.load_version(game_version)?;
            let java = installer.java_for(&parent, ctx).await?;
            let work_dir = temp.0.join("work");
            tokio::fs::create_dir_all(&work_dir).await?;
            let env = ProcessorEnv {
                java: &java,
                work_dir: &work_dir,
                kind,
                version: loader_version,
            };
            run_processors(&profile.processors, &vars, env, ctx).await?;
        }
        check_local_files(installer, &version).await?;
        value
    };

    write_version(installer, &id, &version_value)?;
    Ok(InstalledLoader {
        version_id: id,
        report,
    })
}

/// 处理步骤的变量：内置变量加上 `data` 段的客户端取值。
///
/// `data` 取值规则：`[坐标]` 为构件路径，`'文字'` 为文字本身，`/路径` 为安装器中的文件（解压到临时目录），
/// 其余原样使用。
async fn processor_variables(
    profile: &NewProfile,
    files: &GameFiles,
    jar: &Path,
    temp: &Path,
    game_version: &str,
    kind: LoaderKind,
    loader_version: &str,
) -> AppResult<Variables> {
    let mut vars = Variables::new(files.libraries_dir());
    vars.insert("SIDE", "client");
    vars.insert(
        "MINECRAFT_JAR",
        files.client_jar(game_version).display().to_string(),
    );
    vars.insert("MINECRAFT_VERSION", game_version);
    vars.insert("ROOT", temp.display().to_string());
    vars.insert("INSTALLER", jar.display().to_string());
    vars.insert("LIBRARY_DIR", files.libraries_dir().display().to_string());
    for (key, value) in &profile.data {
        let raw = value.client.as_str();
        let resolved = if let Some(coords) = raw.strip_prefix('[').and_then(|s| s.strip_suffix(']'))
        {
            vars.artifact(coords)
                .map_err(|e| invalid(kind, loader_version, e))?
                .display()
                .to_string()
        } else if let Some(text) = raw.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            text.to_owned()
        } else if let Some(entry) = raw.strip_prefix('/') {
            let dest = entry
                .split('/')
                .filter(|p| !p.is_empty() && *p != "..")
                .fold(temp.join("data"), |acc, p| acc.join(p));
            let (entry, target) = (entry.to_owned(), dest.clone());
            with_jar(jar, kind, loader_version, move |j| {
                j.extract(&entry, &target)
            })
            .await?;
            dest.display().to_string()
        } else {
            raw.to_owned()
        };
        vars.insert(key.clone(), resolved);
    }
    Ok(vars)
}
