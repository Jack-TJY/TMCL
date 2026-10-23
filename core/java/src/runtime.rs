//! 下载 Java 运行时到 `<data_root>/java/`。
//!
//! 先用 Mojang 官方启动器的 java-runtime 清单（逐文件下载、SHA1 校验，
//! 装到 `java/<component>/`）；当前平台没有对应组件或下载失败时，
//! 回退 Adoptium API 下载 Temurin JRE 压缩包（SHA256 校验，解压到 `java/temurin-<major>/`）。
//! 地址登记在 `docs/contracts/endpoints.md`。

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::{DownloadItem, Downloader};

use crate::detect::{Sources, detect, scan};
use crate::probe::{Arch, JavaInstall, probe};
use crate::select::select;

/// Mojang 官方启动器使用的 java-runtime 总索引。
pub const MOJANG_RUNTIME_INDEX: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
/// Adoptium API 根地址。
pub const ADOPTIUM_API: &str = "https://api.adoptium.net/v3";

/// 进度阶段名，前端以 `task.stage.java_runtime` 渲染。
pub const STAGE: &str = "java_runtime";

/// 运行时下载使用的地址，测试中可指向本地服务器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// java-runtime 总索引（`all.json`）。
    pub mojang_index: String,
    /// Adoptium API 根地址（不含末尾 `/`）。
    pub adoptium_api: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            mojang_index: MOJANG_RUNTIME_INDEX.to_owned(),
            adoptium_api: ADOPTIUM_API.to_owned(),
        }
    }
}

/// 运行时所在的平台。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    /// `windows`、`macos` 或 `linux`（与 `std::env::consts::OS` 一致）。
    pub os: &'static str,
    /// 架构。
    pub arch: Arch,
}

impl Platform {
    /// 当前程序运行的平台。
    pub fn host() -> Self {
        Self {
            os: std::env::consts::OS,
            arch: Arch::host(),
        }
    }

    /// Mojang 清单中的平台键，例如 `windows-x64`、`mac-os-arm64`；不支持时为 `None`。
    pub fn mojang_key(self) -> Option<&'static str> {
        Some(match (self.os, self.arch) {
            ("windows", Arch::X64) => "windows-x64",
            ("windows", Arch::X86) => "windows-x86",
            ("windows", Arch::Arm64) => "windows-arm64",
            ("macos", Arch::X64) => "mac-os",
            ("macos", Arch::Arm64) => "mac-os-arm64",
            ("linux", Arch::X64) => "linux",
            ("linux", Arch::X86) => "linux-i386",
            _ => return None,
        })
    }

    /// Adoptium API 的 `os` 与 `architecture` 参数；不支持时为 `None`。
    pub fn adoptium_params(self) -> Option<(&'static str, &'static str)> {
        let os = match self.os {
            "windows" => "windows",
            "macos" => "mac",
            "linux" => "linux",
            _ => return None,
        };
        let arch = match self.arch {
            Arch::X64 => "x64",
            Arch::X86 => "x86-32",
            Arch::Arm64 => "aarch64",
            Arch::Arm32 => "arm",
            Arch::Unknown => return None,
        };
        Some((os, arch))
    }

    fn label(self) -> String {
        format!("{}-{}", self.os, self.arch.as_str())
    }
}

/// 主版本号对应的官方运行时组件名。
pub fn mojang_component(major: u32) -> Option<&'static str> {
    Some(match major {
        8 => "jre-legacy",
        16 => "java-runtime-alpha",
        17 => "java-runtime-gamma",
        21 => "java-runtime-delta",
        25 => "java-runtime-epsilon",
        _ => return None,
    })
}

/// 下载运行时到 `java_dir`（`<data_root>/java/`）。
#[derive(Debug, Clone)]
pub struct RuntimeInstaller {
    downloader: Downloader,
    java_dir: PathBuf,
    endpoints: Endpoints,
    platform: Platform,
}

impl RuntimeInstaller {
    /// 使用官方地址与当前平台。
    pub fn new(downloader: Downloader, java_dir: impl Into<PathBuf>) -> Self {
        Self {
            downloader,
            java_dir: java_dir.into(),
            endpoints: Endpoints::default(),
            platform: Platform::host(),
        }
    }

    /// 替换下载地址（测试用）。
    pub fn with_endpoints(mut self, endpoints: Endpoints) -> Self {
        self.endpoints = endpoints;
        self
    }

    /// 替换目标平台（测试用）。
    pub fn with_platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }

    /// 运行时根目录。
    pub fn java_dir(&self) -> &Path {
        &self.java_dir
    }

    /// 安装主版本为 `major` 的运行时；`component` 为版本 JSON 的 `javaVersion.component`。
    ///
    /// 已安装且文件完整时只做校验。官方源失败（取消除外）时回退 Adoptium；
    /// 两者都没有该平台的运行时返回 `JAVA_RUNTIME_UNAVAILABLE`。
    pub async fn install(
        &self,
        major: u32,
        component: Option<&str>,
        ctx: &TaskCtx,
    ) -> AppResult<JavaInstall> {
        match self.install_mojang(major, component, ctx).await {
            Ok(Some(install)) => return Ok(install),
            Ok(None) => {}
            Err(e) if e.is_cancelled() => return Err(e),
            Err(e) => tracing::warn!(error = %e, major, "官方 Java 运行时下载失败，改用 Adoptium"),
        }
        ctx.check_cancelled()?;
        match self.install_adoptium(major, ctx).await? {
            Some(install) => Ok(install),
            None => Err(AppError::new(ErrorCode::JavaRuntimeUnavailable)
                .with_param("major", major)
                .with_param("platform", self.platform.label())),
        }
    }

    async fn install_mojang(
        &self,
        major: u32,
        component: Option<&str>,
        ctx: &TaskCtx,
    ) -> AppResult<Option<JavaInstall>> {
        let Some(platform) = self.platform.mojang_key() else {
            return Ok(None);
        };
        let Some(component) = component.or_else(|| mojang_component(major)) else {
            return Ok(None);
        };
        let index: BTreeMap<String, BTreeMap<String, Vec<IndexEntry>>> = self
            .downloader
            .client()
            .get_json(&self.endpoints.mojang_index, ctx)
            .await?;
        let Some(entry) = index
            .get(platform)
            .and_then(|p| p.get(component))
            .and_then(|e| e.first())
        else {
            return Ok(None);
        };
        if crate::probe::parse_major(&entry.version.name) != Some(major) {
            // 组件名与要求的主版本不符（例如版本 JSON 写错），交给 Adoptium。
            return Ok(None);
        }

        let manifest_path = self
            .java_dir
            .join(".manifests")
            .join(format!("{component}-{platform}.json"));
        let manifest_item = DownloadItem {
            url: entry.manifest.url.clone(),
            path: manifest_path.clone(),
            size: Some(entry.manifest.size),
            sha1: Some(entry.manifest.sha1.clone()),
        };
        self.downloader.download_one(&manifest_item, ctx).await?;
        let manifest: Manifest = serde_json::from_slice(&tokio::fs::read(&manifest_path).await?)?;

        let root = self.java_dir.join(component);
        let mut items = Vec::new();
        let mut executables = Vec::new();
        let mut links = Vec::new();
        for (name, file) in &manifest.files {
            let rel = safe_relative(name)?;
            let path = root.join(&rel);
            match file {
                ManifestFile::Directory => tokio::fs::create_dir_all(&path).await?,
                ManifestFile::File {
                    downloads,
                    executable,
                } => {
                    items.push(DownloadItem {
                        url: downloads.raw.url.clone(),
                        path: path.clone(),
                        size: Some(downloads.raw.size),
                        sha1: Some(downloads.raw.sha1.clone()),
                    });
                    if *executable {
                        executables.push(path);
                    }
                }
                ManifestFile::Link { target } => links.push((path, PathBuf::from(target))),
            }
        }
        self.downloader.download_all(items, STAGE, ctx).await?;
        finish_unix(&executables, &links)?;
        tokio::fs::write(root.join(".version"), &entry.version.name).await?;
        Ok(Some(probe_managed(&root, major).await?))
    }

    async fn install_adoptium(&self, major: u32, ctx: &TaskCtx) -> AppResult<Option<JavaInstall>> {
        let Some((os, arch)) = self.platform.adoptium_params() else {
            return Ok(None);
        };
        let mut package = None;
        for image in ["jre", "jdk"] {
            let url = format!(
                "{}/assets/latest/{major}/hotspot?architecture={arch}&image_type={image}&os={os}&vendor=eclipse",
                self.endpoints.adoptium_api
            );
            let assets: Vec<AdoptiumAsset> =
                match self.downloader.client().get_json(&url, ctx).await {
                    Ok(assets) => assets,
                    // 不存在的主版本返回 404。
                    Err(e) if e.code == ErrorCode::NetHttpStatus => Vec::new(),
                    Err(e) => return Err(e),
                };
            if let Some(asset) = assets.into_iter().next() {
                package = Some(asset.binary.package);
                break;
            }
        }
        let Some(package) = package else {
            return Ok(None);
        };

        let archive = self
            .java_dir
            .join(".download")
            .join(file_name(&package.name)?);
        let item = DownloadItem {
            url: package.link.clone(),
            path: archive.clone(),
            size: Some(package.size),
            sha1: None,
        };
        self.downloader.download_all(vec![item], STAGE, ctx).await?;
        let actual = sha256_file(&archive).await?;
        if !actual.eq_ignore_ascii_case(&package.checksum) {
            let _ = tokio::fs::remove_file(&archive).await;
            return Err(AppError::new(ErrorCode::NetHashMismatch)
                .with_param("url", &package.link)
                .with_param("expected", &package.checksum)
                .with_param("actual", actual));
        }
        ctx.check_cancelled()?;

        let root = self.java_dir.join(format!("temurin-{major}"));
        let staging = self.java_dir.join(format!(".temurin-{major}.tmp"));
        let (src, dst) = (archive.clone(), staging.clone());
        tokio::task::spawn_blocking(move || extract(&src, &dst))
            .await
            .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))??;
        if tokio::fs::try_exists(&root).await? {
            tokio::fs::remove_dir_all(&root).await?;
        }
        tokio::fs::rename(&staging, &root).await?;
        let _ = tokio::fs::remove_file(&archive).await;
        Ok(Some(probe_managed(&root, major).await?))
    }
}

/// 自动模式的完整流程：探测本机 Java 并选择，没有合适的就下载。
///
/// 只有架构不符的 Java 且当前平台无法下载时，返回 `JAVA_ARCH_MISMATCH` 而不是 `JAVA_RUNTIME_UNAVAILABLE`。
pub async fn ensure(
    installer: &RuntimeInstaller,
    sources: &Sources,
    major: u32,
    component: Option<&str>,
    ctx: &TaskCtx,
) -> AppResult<JavaInstall> {
    let installs = detect(sources).await;
    let not_usable = match select(&installs, major, installer.platform.arch) {
        Ok(install) => return Ok(install.clone()),
        Err(e)
            if matches!(
                e.code,
                ErrorCode::JavaNotFound | ErrorCode::JavaArchMismatch
            ) =>
        {
            e
        }
        Err(e) => return Err(e),
    };
    match installer.install(major, component, ctx).await {
        Err(e)
            if e.code == ErrorCode::JavaRuntimeUnavailable
                && not_usable.code == ErrorCode::JavaArchMismatch =>
        {
            Err(not_usable)
        }
        other => other,
    }
}

async fn probe_managed(root: &Path, major: u32) -> AppResult<JavaInstall> {
    let candidates = scan(root, 4);
    for exe in &candidates {
        if let Ok(install) = probe(exe, true).await
            && install.major == major
        {
            return Ok(install);
        }
    }
    Err(AppError::new(ErrorCode::JavaProbeFailed).with_param("java", root.display()))
}

/// 校验清单或压缩包中的相对路径：不得为绝对路径或含 `..`。
fn safe_relative(name: &str) -> AppResult<PathBuf> {
    let path = Path::new(name);
    let ok = !name.is_empty()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if ok {
        Ok(path.to_path_buf())
    } else {
        Err(AppError::new(ErrorCode::CommonIo)
            .with_param("path", name)
            .with_detail("unsafe path in archive"))
    }
}

fn file_name(name: &str) -> AppResult<PathBuf> {
    let rel = safe_relative(name)?;
    match rel.file_name() {
        Some(file) if rel.components().count() == 1 => Ok(PathBuf::from(file)),
        _ => Err(AppError::new(ErrorCode::CommonIo)
            .with_param("path", name)
            .with_detail("unsafe file name")),
    }
}

#[cfg(unix)]
fn finish_unix(executables: &[PathBuf], links: &[(PathBuf, PathBuf)]) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    for path in executables {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    for (path, target) in links {
        if path.symlink_metadata().is_ok() {
            std::fs::remove_file(path)?;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::os::unix::fs::symlink(target, path)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn finish_unix(_executables: &[PathBuf], links: &[(PathBuf, PathBuf)]) -> AppResult<()> {
    if !links.is_empty() {
        tracing::debug!(count = links.len(), "Windows 上忽略运行时清单中的符号链接");
    }
    Ok(())
}

async fn sha256_file(path: &Path) -> AppResult<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> AppResult<String> {
        let mut file = std::fs::File::open(&path)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect())
    })
    .await
    .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?
}

/// 把 Temurin 压缩包（`.zip` 或 `.tar.gz`）解压到 `dest`，去掉最外层目录。
fn extract(archive: &Path, dest: &Path) -> AppResult<()> {
    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    std::fs::create_dir_all(dest)?;
    let file = std::fs::File::open(archive)?;
    let name = archive.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(file).map_err(archive_error)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(archive_error)?;
            let Some(rel) = strip_top(&entry.name().map_err(archive_error)?.replace('\\', "/"))?
            else {
                continue;
            };
            let out = dest.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::io::copy(&mut entry, &mut std::fs::File::create(&out)?)?;
            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))?;
            }
        }
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
        for entry in tar.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_string_lossy().into_owned();
            let Some(rel) = strip_top(&path)? else {
                continue;
            };
            let out = dest.join(rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            entry.unpack(&out)?;
        }
    } else {
        return Err(AppError::new(ErrorCode::CommonIo)
            .with_param("path", archive.display())
            .with_detail("unknown archive format"));
    }
    Ok(())
}

/// 去掉第一层目录；只有顶层目录本身时返回 `None`。
fn strip_top(name: &str) -> AppResult<Option<PathBuf>> {
    let rel = safe_relative(name.trim_end_matches('/'))?;
    let mut parts = rel.components();
    parts.next();
    let rest = parts.as_path();
    Ok((!rest.as_os_str().is_empty()).then(|| rest.to_path_buf()))
}

fn archive_error(err: zip::result::ZipError) -> AppError {
    AppError::new(ErrorCode::CommonIo).with_detail(err.to_string())
}

#[derive(Debug, Deserialize)]
struct IndexEntry {
    manifest: Remote,
    version: IndexVersion,
}

#[derive(Debug, Deserialize)]
struct IndexVersion {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Remote {
    sha1: String,
    size: u64,
    url: String,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    files: BTreeMap<String, ManifestFile>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ManifestFile {
    Directory,
    File {
        downloads: FileDownloads,
        #[serde(default)]
        executable: bool,
    },
    Link {
        target: String,
    },
}

#[derive(Debug, Deserialize)]
struct FileDownloads {
    raw: Remote,
}

#[derive(Debug, Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    checksum: String,
    link: String,
    name: String,
    size: u64,
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;
    use sha1::Sha1;
    use tmcl_net::testing::TestServer;
    use tmcl_net::{DownloadOptions, HttpClient};

    use super::*;
    use crate::probe::JAVA_EXE;

    const WIN_X64: Platform = Platform {
        os: "windows",
        arch: Arch::X64,
    };

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn sha1_hex(data: &[u8]) -> String {
        hex(&Sha1::digest(data))
    }

    fn release(version: &str) -> Vec<u8> {
        format!("JAVA_VERSION=\"{version}\"\nOS_ARCH=\"amd64\"\nIMPLEMENTOR=\"Test\"\n")
            .into_bytes()
    }

    fn empty_server_routes() -> Vec<(String, Vec<u8>)> {
        Vec::new()
    }

    fn installer(server: &TestServer, java_dir: &Path) -> RuntimeInstaller {
        let downloader = Downloader::new(HttpClient::new().unwrap(), DownloadOptions::default());
        RuntimeInstaller::new(downloader, java_dir)
            .with_endpoints(Endpoints {
                mojang_index: server.url("/all.json"),
                adoptium_api: server.url("/v3"),
            })
            .with_platform(WIN_X64)
    }

    /// 在服务器上发布一个只含 `release` 与 `bin/java` 的官方运行时。
    fn serve_mojang(server: &TestServer, component: &str, version: &str) {
        let release = release(version);
        let java = b"fake java".to_vec();
        let java_name = format!("bin/{JAVA_EXE}");
        let mut files = serde_json::Map::new();
        files.insert("bin".into(), json!({ "type": "directory" }));
        for (name, body, executable) in [
            ("release", &release, false),
            (java_name.as_str(), &java, true),
        ] {
            let path = format!("/objects/{}", sha1_hex(body));
            server.route(path.clone(), body.clone());
            files.insert(
                name.into(),
                json!({
                    "type": "file",
                    "executable": executable,
                    "downloads": { "raw": { "sha1": sha1_hex(body), "size": body.len(), "url": server.url(&path) } },
                }),
            );
        }
        files.insert(
            "legal/LICENSE".into(),
            json!({ "type": "link", "target": "../release" }),
        );
        let manifest = serde_json::to_vec(&json!({ "files": files })).unwrap();
        server.route("/manifest.json", manifest.clone());
        let index = json!({
            "windows-x64": {
                component: [{
                    "availability": { "group": 1, "progress": 100 },
                    "manifest": { "sha1": sha1_hex(&manifest), "size": manifest.len(), "url": server.url("/manifest.json") },
                    "version": { "name": version, "released": "2025-01-01T00:00:00+00:00" },
                }],
            },
            "linux": {},
        });
        server.route("/all.json", serde_json::to_vec(&index).unwrap());
    }

    /// 构造 Temurin 式 zip：最外层是 `jdk-<version>-jre/`。
    fn temurin_zip(version: &str) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        let top = format!("jdk-{version}-jre");
        zip.add_directory(format!("{top}/"), options).unwrap();
        zip.start_file(format!("{top}/release"), options).unwrap();
        zip.write_all(&release(version)).unwrap();
        zip.start_file(format!("{top}/bin/{JAVA_EXE}"), options)
            .unwrap();
        zip.write_all(b"fake java").unwrap();
        zip.finish().unwrap().into_inner()
    }

    fn adoptium_path(major: u32, image: &str) -> String {
        format!(
            "/v3/assets/latest/{major}/hotspot?architecture=x64&image_type={image}&os=windows&vendor=eclipse"
        )
    }

    fn serve_adoptium(server: &TestServer, major: u32, archive: Vec<u8>, checksum: Option<String>) {
        let name = format!("OpenJDK{major}U-jre_x64_windows_hotspot.zip");
        let checksum = checksum.unwrap_or_else(|| hex(&Sha256::digest(&archive)));
        let assets = json!([{ "binary": { "package": {
            "checksum": checksum,
            "link": server.url(&format!("/dl/{name}")),
            "name": name,
            "size": archive.len(),
        } } }]);
        server.route(format!("/dl/{name}"), archive);
        server.route(
            adoptium_path(major, "jre"),
            serde_json::to_vec(&assets).unwrap(),
        );
    }

    fn fake_home(home: &Path, release: &[u8]) {
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::write(home.join("bin").join(JAVA_EXE), b"").unwrap();
        std::fs::write(home.join("release"), release).unwrap();
    }

    #[test]
    fn platform_keys() {
        assert_eq!(WIN_X64.mojang_key(), Some("windows-x64"));
        let mac_arm = Platform {
            os: "macos",
            arch: Arch::Arm64,
        };
        assert_eq!(mac_arm.mojang_key(), Some("mac-os-arm64"));
        assert_eq!(mac_arm.adoptium_params(), Some(("mac", "aarch64")));
        let linux_x86 = Platform {
            os: "linux",
            arch: Arch::X86,
        };
        assert_eq!(linux_x86.mojang_key(), Some("linux-i386"));
        assert_eq!(linux_x86.adoptium_params(), Some(("linux", "x86-32")));
        let bsd = Platform {
            os: "freebsd",
            arch: Arch::X64,
        };
        assert_eq!(bsd.mojang_key(), None);
        assert_eq!(bsd.adoptium_params(), None);
        assert_eq!(mojang_component(8), Some("jre-legacy"));
        assert_eq!(mojang_component(17), Some("java-runtime-gamma"));
        assert_eq!(mojang_component(11), None);
    }

    #[test]
    fn unsafe_paths_are_rejected() {
        assert!(safe_relative("bin/java").is_ok());
        assert!(safe_relative("../evil").is_err());
        assert!(safe_relative("bin/../../evil").is_err());
        assert!(safe_relative("/etc/passwd").is_err());
        assert!(file_name("a/b.zip").is_err());
        assert_eq!(
            strip_top("jdk/bin/java").unwrap(),
            Some(PathBuf::from("bin/java"))
        );
        assert_eq!(strip_top("jdk/").unwrap(), None);
    }

    #[tokio::test]
    async fn installs_official_runtime_and_reuses_it() {
        let server = TestServer::start(empty_server_routes()).await;
        serve_mojang(&server, "java-runtime-gamma", "17.0.15");
        let dir = tempfile::tempdir().unwrap();
        let installer = installer(&server, dir.path());
        let ctx = TaskCtx::noop("java_install");

        let install = installer
            .install(17, Some("java-runtime-gamma"), &ctx)
            .await
            .unwrap();
        assert_eq!(install.major, 17);
        assert_eq!(install.version, "17.0.15");
        assert!(install.managed);
        let root = dir.path().join("java-runtime-gamma");
        assert_eq!(install.path, root.join("bin").join(JAVA_EXE));
        assert_eq!(
            std::fs::read_to_string(root.join(".version")).unwrap(),
            "17.0.15"
        );

        // 第二次只拉索引，清单与文件都已校验通过而跳过。
        let before = server.hits();
        let again = installer.install(17, None, &ctx).await.unwrap();
        assert_eq!(again, install);
        assert_eq!(server.hits() - before, 1);
    }

    #[tokio::test]
    async fn falls_back_to_adoptium_when_official_fails() {
        // 官方索引 404（没有注册路由）。
        let server = TestServer::start(empty_server_routes()).await;
        serve_adoptium(&server, 8, temurin_zip("1.8.0_462"), None);
        let dir = tempfile::tempdir().unwrap();
        let installer = installer(&server, dir.path());

        let install = installer
            .install(8, None, &TaskCtx::noop("java_install"))
            .await
            .unwrap();
        assert_eq!(install.major, 8);
        assert!(install.managed);
        assert_eq!(
            install.path,
            dir.path().join("temurin-8").join("bin").join(JAVA_EXE)
        );
        // 压缩包与临时目录已清理。
        assert!(!dir.path().join(".temurin-8.tmp").exists());
        assert_eq!(
            std::fs::read_dir(dir.path().join(".download"))
                .unwrap()
                .count(),
            0
        );
    }

    #[tokio::test]
    async fn adoptium_checksum_mismatch_fails() {
        let server = TestServer::start(empty_server_routes()).await;
        serve_adoptium(&server, 17, temurin_zip("17.0.8"), Some("00".repeat(32)));
        let dir = tempfile::tempdir().unwrap();
        let err = installer(&server, dir.path())
            .install(17, None, &TaskCtx::noop("java_install"))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NetHashMismatch);
        assert!(!dir.path().join("temurin-17").exists());
    }

    #[tokio::test]
    async fn unknown_major_is_unavailable() {
        let server = TestServer::start(empty_server_routes()).await;
        // Adoptium 对 jre 与 jdk 都返回空列表。
        server.route(adoptium_path(11, "jre"), b"[]".to_vec());
        server.route(adoptium_path(11, "jdk"), b"[]".to_vec());
        let dir = tempfile::tempdir().unwrap();
        let err = installer(&server, dir.path())
            .install(11, None, &TaskCtx::noop("java_install"))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::JavaRuntimeUnavailable);
        assert_eq!(err.params["platform"], "windows-x64");
    }

    #[tokio::test]
    async fn cancelled_install_does_not_fall_back() {
        let server = TestServer::start(empty_server_routes()).await;
        serve_mojang(&server, "java-runtime-gamma", "17.0.15");
        let dir = tempfile::tempdir().unwrap();
        let ctx = TaskCtx::noop("java_install");
        ctx.cancel.cancel();
        let err = installer(&server, dir.path())
            .install(17, None, &ctx)
            .await
            .unwrap_err();
        assert!(err.is_cancelled());
        assert!(server.requests().iter().all(|r| !r.path.starts_with("/v3")));
    }

    #[tokio::test]
    async fn ensure_prefers_local_then_downloads() {
        let server = TestServer::start(empty_server_routes()).await;
        serve_mojang(&server, "java-runtime-gamma", "17.0.15");
        let dir = tempfile::tempdir().unwrap();
        let installer = installer(&server, &dir.path().join("java"));
        let local = dir.path().join("local-jdk17");
        fake_home(&local, &release("17.0.8"));
        let ctx = TaskCtx::noop("java_install");

        let with_local = Sources {
            managed_dir: Some(installer.java_dir().to_path_buf()),
            homes: vec![local],
            ..Sources::default()
        };
        let found = ensure(&installer, &with_local, 17, None, &ctx)
            .await
            .unwrap();
        assert!(!found.managed);
        assert_eq!(server.hits(), 0);

        let without_local = Sources {
            homes: Vec::new(),
            ..with_local
        };
        let downloaded = ensure(&installer, &without_local, 17, None, &ctx)
            .await
            .unwrap();
        assert!(downloaded.managed);
    }

    #[tokio::test]
    async fn ensure_reports_arch_mismatch_when_nothing_to_download() {
        let server = TestServer::start(empty_server_routes()).await;
        let dir = tempfile::tempdir().unwrap();
        let installer = installer(&server, &dir.path().join("java")).with_platform(Platform {
            os: "freebsd",
            arch: Arch::X64,
        });
        let x86 = dir.path().join("jre8-x86");
        fake_home(&x86, b"JAVA_VERSION=\"1.8.0_202\"\nOS_ARCH=\"i586\"\n");

        let sources = Sources {
            homes: vec![x86],
            ..Sources::default()
        };
        let err = ensure(
            &installer,
            &sources,
            8,
            None,
            &TaskCtx::noop("java_install"),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::JavaArchMismatch);
        assert_eq!(err.params["actual"], "x86");
    }

    #[tokio::test]
    #[ignore = "联网：从官方源下载 Java 17 与 Java 8 到临时目录"]
    async fn online_install_17_and_8() {
        let dir = tempfile::tempdir().unwrap();
        let downloader = Downloader::new(HttpClient::new().unwrap(), DownloadOptions::default());
        let installer = RuntimeInstaller::new(downloader, dir.path());
        let ctx = TaskCtx::noop("java_install");
        for major in [17, 8] {
            let install = installer.install(major, None, &ctx).await.unwrap();
            println!("{install:?}");
            assert_eq!(install.major, major);
        }
    }

    #[tokio::test]
    #[ignore = "联网：从 Adoptium 下载 Java 21"]
    async fn online_adoptium_21() {
        let dir = tempfile::tempdir().unwrap();
        let downloader = Downloader::new(HttpClient::new().unwrap(), DownloadOptions::default());
        let installer = RuntimeInstaller::new(downloader, dir.path()).with_endpoints(Endpoints {
            mojang_index: "http://127.0.0.1:9/unreachable".into(),
            ..Endpoints::default()
        });
        let install = installer
            .install(21, None, &TaskCtx::noop("java_install"))
            .await
            .unwrap();
        println!("{install:?}");
        assert_eq!(install.major, 21);
    }
}
