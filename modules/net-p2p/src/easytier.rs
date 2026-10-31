//! 运行时下载并校验 EasyTier。
//!
//! EasyTier（<https://github.com/EasyTier/EasyTier>，LGPL-3.0）作为独立程序使用：
//! 首次联机时从官方 GitHub Releases 下载对应平台的压缩包，按内置的 SHA-256 校验，
//! 只解压 `easytier-core`、`easytier-cli`（Windows 另有 `Packet.dll`）到
//! `<data_root>/p2p/easytier/<版本>/`，最后写入 `.complete` 标记。

use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tmcl_common::{AppError, AppResult, DataPaths, ErrorCode, TaskCtx};
use tmcl_net::{DownloadItem, Downloader};

/// 使用的 EasyTier 版本。
pub const EASYTIER_VERSION: &str = "v2.6.4";

/// 各平台压缩包的 SHA-256 与大小（2026-10-30 从官方 Releases 下载后核对）。
const ASSETS: [(&str, &str, u64); 6] = [
    (
        "windows-x86_64",
        "27af91e270e554709b048bd32327fefd2dfce5062ae1e8701af7550c6f525f84",
        32_657_720,
    ),
    (
        "windows-arm64",
        "37023f8a3451c9234b17ee2089a03dc344ce90d803b5b359cb6c46682b0549b4",
        30_136_892,
    ),
    (
        "linux-x86_64",
        "61b659eaedba658fa66fe47d17e1426cdd77e5d02fa15fed447bb4357c09dfd6",
        25_506_523,
    ),
    (
        "linux-aarch64",
        "f533ec25a7ea714e09f645615012200278058525795cc3bb690ff011aec1a70f",
        20_487_230,
    ),
    (
        "macos-x86_64",
        "89fc28a6e6995259d76ce3f11775220e8a21c760e94df91a6a9db30a69b6982e",
        33_733_545,
    ),
    (
        "macos-aarch64",
        "4be1882d1aa36d31c1d6ba0596f2cf8a097e371f8da124212324b2e0f8df7e4b",
        30_174_017,
    ),
];

/// 安装完成标记，内容为压缩包的 SHA-256。
const MARKER: &str = ".complete";
/// 进度阶段名。
const STAGE: &str = "p2p_easytier";

/// 一个平台的 EasyTier 压缩包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EasyTierAsset {
    /// 平台标识，如 `windows-x86_64`，也是压缩包内顶层目录名的后缀。
    pub platform: String,
    /// 下载地址。
    pub url: String,
    /// 压缩包 SHA-256（小写十六进制）。
    pub sha256: String,
    /// 压缩包大小（字节）。
    pub size: u64,
}

impl EasyTierAsset {
    /// 当前操作系统与架构对应的压缩包；不支持时返回 `P2P_UNSUPPORTED_PLATFORM`。
    pub fn current() -> AppResult<Self> {
        let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", "x86_64") => "windows-x86_64",
            ("windows", "aarch64") => "windows-arm64",
            ("linux", "x86_64") => "linux-x86_64",
            ("linux", "aarch64") => "linux-aarch64",
            ("macos", "x86_64") => "macos-x86_64",
            ("macos", "aarch64") => "macos-aarch64",
            (os, arch) => {
                return Err(AppError::new(ErrorCode::P2pUnsupportedPlatform)
                    .with_param("platform", format!("{os}-{arch}")));
            }
        };
        let (_, sha256, size) = ASSETS
            .iter()
            .find(|(p, _, _)| *p == platform)
            .copied()
            .ok_or_else(|| {
                AppError::new(ErrorCode::P2pUnsupportedPlatform).with_param("platform", platform)
            })?;
        Ok(Self {
            platform: platform.to_owned(),
            url: format!(
                "https://github.com/EasyTier/EasyTier/releases/download/{EASYTIER_VERSION}/easytier-{platform}-{EASYTIER_VERSION}.zip"
            ),
            sha256: sha256.to_owned(),
            size,
        })
    }

    fn is_windows(&self) -> bool {
        self.platform.starts_with("windows-")
    }

    /// 需要从压缩包中取出的文件名。
    fn files(&self) -> &'static [&'static str] {
        if self.is_windows() {
            &["easytier-core.exe", "easytier-cli.exe", "Packet.dll"]
        } else {
            &["easytier-core", "easytier-cli"]
        }
    }
}

/// 已安装的 EasyTier 可执行文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EasyTierBinaries {
    /// `easytier-core`：组网进程。
    pub core: PathBuf,
    /// `easytier-cli`：查询对端、添加端口转发。
    pub cli: PathBuf,
}

impl EasyTierBinaries {
    fn in_dir(dir: &Path, windows: bool) -> Self {
        let ext = if windows { ".exe" } else { "" };
        Self {
            core: dir.join(format!("easytier-core{ext}")),
            cli: dir.join(format!("easytier-cli{ext}")),
        }
    }
}

/// EasyTier 的安装目录 `<data_root>/p2p/easytier/<版本>/`。
pub fn install_dir(paths: &DataPaths) -> PathBuf {
    paths.p2p_dir().join("easytier").join(EASYTIER_VERSION)
}

/// 已安装且完整时返回可执行文件路径。
pub fn installed(paths: &DataPaths, asset: &EasyTierAsset) -> Option<EasyTierBinaries> {
    let dir = install_dir(paths);
    let marker = std::fs::read_to_string(dir.join(MARKER)).ok()?;
    if marker.trim() != asset.sha256 {
        return None;
    }
    let complete = asset.files().iter().all(|f| dir.join(f).is_file());
    complete.then(|| EasyTierBinaries::in_dir(&dir, asset.is_windows()))
}

/// 确保当前平台的 EasyTier 已安装，必要时下载、校验并解压。
pub async fn ensure_installed(
    downloader: &Downloader,
    paths: &DataPaths,
    ctx: &TaskCtx,
) -> AppResult<EasyTierBinaries> {
    install(downloader, paths, &EasyTierAsset::current()?, ctx).await
}

/// 按给定压缩包安装 EasyTier；已安装时直接返回。
pub async fn install(
    downloader: &Downloader,
    paths: &DataPaths,
    asset: &EasyTierAsset,
    ctx: &TaskCtx,
) -> AppResult<EasyTierBinaries> {
    if let Some(found) = installed(paths, asset) {
        return Ok(found);
    }
    let archive = paths.cache_dir().join(format!(
        "easytier-{}-{EASYTIER_VERSION}.zip",
        asset.platform
    ));
    let item = DownloadItem {
        url: asset.url.clone(),
        path: archive.clone(),
        size: Some(asset.size),
        sha1: None,
    };
    downloader.download_all(vec![item], STAGE, ctx).await?;
    ctx.check_cancelled()?;

    let asset = asset.clone();
    let dir = install_dir(paths);
    let zip_path = archive.clone();
    let result = tokio::task::spawn_blocking(move || {
        let actual = sha256_file(&zip_path)?;
        if actual != asset.sha256 {
            return Err(AppError::new(ErrorCode::P2pBinaryInvalid)
                .with_detail(format!("sha256 expected {} actual {actual}", asset.sha256)));
        }
        extract(&zip_path, &asset, &dir)
    })
    .await
    .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?;
    // 压缩包只是中间产物，无论成功与否都删掉，校验失败时下次重新下载。
    let _ = tokio::fs::remove_file(&archive).await;
    result
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = std::fs::File::open(path)?;
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
}

/// 把所需文件解压到 `<dir>.partial`，齐全后整体换到 `dir` 并写入标记。
fn extract(archive: &Path, asset: &EasyTierAsset, dir: &Path) -> AppResult<EasyTierBinaries> {
    let partial = dir.with_extension("partial");
    if partial.exists() {
        std::fs::remove_dir_all(&partial)?;
    }
    std::fs::create_dir_all(&partial)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)
        .map_err(|e| AppError::new(ErrorCode::P2pBinaryInvalid).with_detail(e.to_string()))?;
    let top = format!("easytier-{}", asset.platform);
    for file in asset.files() {
        let mut entry = zip
            .by_name(&format!("{top}/{file}"))
            .map_err(|_| AppError::new(ErrorCode::P2pBinaryInvalid).with_param("file", file))?;
        let out = partial.join(file);
        std::io::copy(&mut entry, &mut std::fs::File::create(&out)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    std::fs::write(partial.join(MARKER), &asset.sha256)?;
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::rename(&partial, dir)?;
    Ok(EasyTierBinaries::in_dir(dir, asset.is_windows()))
}
