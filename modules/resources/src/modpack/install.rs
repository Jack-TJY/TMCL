//! 从本地 `.mrpack` 或 CurseForge 整合包 zip 安装为新实例。
//!
//! 步骤：读取并校验索引（CurseForge 整合包还要向 API 查询文件）→ 新建实例 → 补全原版并安装加载器 → 并行下载 `files`（按 SHA1 校验）
//! → 解压 `overrides/`、`client-overrides/`（CurseForge 为清单指定的目录）。
//! CurseForge 整合包中作者禁止第三方分发的文件不下载，见 [`ModpackInfo::manual_downloads`]。任一步失败或取消时删除新实例（回滚），
//! 已下载到 `shared/` 的原版与加载器文件保留以便重试。

use std::path::{Path, PathBuf};

use tmcl_common::{AppError, AppResult, DataPaths, Db, ErrorCode, TaskCtx};
use tmcl_game::{InstallOptions, install_version};
use tmcl_instance::{InstanceConfig, InstanceStore, LoaderSpec};
use tmcl_loader::LoaderInstaller;

use super::curseforge as cf_pack;
use super::mrpack::{self, ModpackFormat, ModpackInfo, PackFile};
use crate::curseforge::CurseForgeClient;

/// 下载阶段名。
pub const FILES_STAGE: &str = "modpack_files";

/// 安装整合包所需的环境。新实例的 Java 为自动，因此 `loader` 应使用 [`tmcl_loader::JavaChoice::Auto`]。
#[derive(Debug, Clone)]
pub struct ModpackInstaller {
    loader: LoaderInstaller,
    game: InstallOptions,
    curseforge: Option<CurseForgeClient>,
    fallbacks: cf_pack::Fallbacks,
}

/// 读取、校验并解析好的整合包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedPack {
    /// 概要。
    pub info: ModpackInfo,
    /// 要下载的文件。
    pub files: Vec<PackFile>,
    /// 尝试下载、失败也不影响安装的文件（CurseForge 镜像），附手动下载说明。
    pub optional: Vec<(PackFile, cf_pack::ManualDownload)>,
    /// 要依次解压到 `game/` 的包内目录。
    pub overrides: Vec<String>,
}

impl ModpackInstaller {
    /// `loader` 提供下载引擎、数据目录与加载器端点；`game` 为补全原版的选项。
    pub fn new(loader: LoaderInstaller, game: InstallOptions) -> Self {
        Self {
            loader,
            game,
            curseforge: None,
            fallbacks: cf_pack::Fallbacks::default(),
        }
    }

    /// 支持 CurseForge 整合包；没有设置时安装 CurseForge 整合包返回 `RESOURCES_MODPACK_UNSUPPORTED`。
    ///
    /// 下载引擎应由 [`crate::curseforge::with_cdn_key`] 创建，否则 CDN 会拒绝下载。
    pub fn with_curseforge(mut self, client: Option<CurseForgeClient>) -> Self {
        self.curseforge = client;
        self
    }

    /// CurseForge 整合包中作者禁止第三方分发的文件的备选来源，见 [`cf_pack::Fallbacks`]。
    pub fn with_fallbacks(mut self, fallbacks: cf_pack::Fallbacks) -> Self {
        self.fallbacks = fallbacks;
        self
    }

    /// 读取并校验整合包，CurseForge 整合包还会向 API 查询文件；不写任何文件。
    ///
    /// 按包内是否有 `modrinth.index.json` / `manifest.json` 判断格式，都没有时返回
    /// `RESOURCES_MODPACK_INVALID`。
    pub async fn inspect(&self, pack: &Path) -> AppResult<PreparedPack> {
        let path = pack.to_owned();
        let parsed = blocking(move || match mrpack::read(&path) {
            Ok(pack) => Ok(Ok(pack)),
            Err(err) if err.code == ErrorCode::ResourcesModpackInvalid => {
                match cf_pack::read(&path)? {
                    Some(manifest) => Ok(Err(manifest)),
                    None => Err(err),
                }
            }
            Err(err) => Err(err),
        })
        .await?;
        let manifest = match parsed {
            Ok(pack) => {
                return Ok(PreparedPack {
                    info: pack.info,
                    files: pack.files,
                    optional: Vec::new(),
                    overrides: mrpack::OVERRIDE_DIRS.map(str::to_owned).to_vec(),
                });
            }
            Err(manifest) => manifest,
        };
        let client = self.curseforge.as_ref().ok_or_else(|| {
            AppError::new(ErrorCode::ResourcesModpackUnsupported).with_param("name", "curseforge")
        })?;
        let ctx = TaskCtx::noop("modpack_inspect");
        let resolved = cf_pack::resolve(&manifest, client, &self.fallbacks, &ctx).await?;
        let info = ModpackInfo {
            format: ModpackFormat::Curseforge,
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            summary: None,
            game_version: manifest.game_version.clone(),
            loader: manifest.loader.clone(),
            file_count: u32::try_from(resolved.files.len()).unwrap_or(u32::MAX),
            download_size: resolved.files.iter().map(|f| f.size).sum(),
            manual_downloads: resolved.manual,
            redirected_downloads: resolved.redirected,
        };
        Ok(PreparedPack {
            info,
            files: resolved.files,
            optional: resolved.optional,
            overrides: cf_pack::override_dirs(&manifest),
        })
    }

    /// 把 `pack` 安装为新实例，返回实例配置；`name` 省略时使用整合包名称。
    pub async fn install(
        &self,
        pack: &Path,
        name: Option<&str>,
        ctx: &TaskCtx,
    ) -> AppResult<InstanceConfig> {
        let parsed = self.inspect(pack).await?;
        ctx.check_cancelled()?;
        let paths = self.loader.paths().clone();
        let name = name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or(&parsed.info.name)
            .to_owned();
        let game_version = parsed.info.game_version.clone();
        let config = with_store(&paths, move |store| {
            store.create(&name, &game_version, None)
        })
        .await?;
        tracing::info!(id = %config.id, pack = %parsed.info.name, "开始安装整合包");

        match self.fill(&config, &parsed, pack, ctx).await {
            Ok(config) => Ok(config),
            Err(err) => {
                rollback(&paths, &config.id).await;
                Err(err)
            }
        }
    }

    async fn fill(
        &self,
        config: &InstanceConfig,
        parsed: &PreparedPack,
        pack: &Path,
        ctx: &TaskCtx,
    ) -> AppResult<InstanceConfig> {
        let info = &parsed.info;
        let paths = self.loader.paths().clone();
        match &info.loader {
            Some(loader) => {
                self.loader
                    .install(loader.kind, &info.game_version, &loader.version, ctx)
                    .await?;
            }
            None => {
                install_version(
                    self.loader.downloader(),
                    &paths,
                    &info.game_version,
                    &self.game,
                    ctx,
                )
                .await?;
            }
        }
        ctx.check_cancelled()?;

        let game_dir = paths.instance_game_dir(&config.id);
        let items: Vec<_> = parsed
            .files
            .iter()
            .map(|f| f.download_item(&game_dir))
            .collect();
        for item in &items {
            if let Some(dir) = item.path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| crate::install::io_error(dir, &e))?;
            }
        }
        self.loader
            .downloader()
            .download_all(items, FILES_STAGE, ctx)
            .await?;
        for (file, manual) in &parsed.optional {
            ctx.check_cancelled()?;
            let item = file.download_item(&game_dir);
            if let Some(dir) = item.path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| crate::install::io_error(dir, &e))?;
            }
            match self.loader.downloader().download_one(&item, ctx).await {
                Ok(_) => {}
                Err(err) if err.is_cancelled() => return Err(err),
                Err(err) => tracing::warn!(
                    %err,
                    file = %manual.file_name,
                    page = %manual.page_url,
                    "镜像下载失败，需要手动下载"
                ),
            }
        }

        let (pack, dir, task) = (pack.to_owned(), game_dir.clone(), ctx.clone());
        let overrides = parsed.overrides.clone();
        blocking(move || mrpack::extract_dirs(&pack, &overrides, &dir, &task)).await?;
        ctx.check_cancelled()?;

        let (id, loader) = (config.id.clone(), info.loader.clone());
        let config = with_store(&paths, move |store| set_loader(store, &id, loader)).await?;
        tracing::info!(id = %config.id, "整合包安装完成");
        Ok(config)
    }
}

/// 重新读取配置后写入加载器，保留安装期间对实例的其他修改。
fn set_loader(
    store: &InstanceStore<'_>,
    id: &str,
    loader: Option<LoaderSpec>,
) -> AppResult<InstanceConfig> {
    let mut config = store.get(id)?;
    config.loader = loader;
    store.save(&config)
}

/// 删除安装失败的新实例：直接删除目录（不进回收站），再让索引与磁盘一致。
async fn rollback(paths: &DataPaths, id: &str) {
    let dir: PathBuf = paths.instance_dir(id);
    let result = with_store(paths, move |store| {
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(crate::install::io_error(&dir, &e)),
        }
        store.reconcile().map(drop)
    })
    .await;
    match result {
        Ok(()) => tracing::info!(id, "整合包安装失败，已删除新实例"),
        Err(err) => tracing::warn!(id, %err, "整合包安装失败，删除新实例时出错"),
    }
}

/// 在阻塞线程上打开数据库并操作实例存储。
async fn with_store<T, F>(paths: &DataPaths, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&InstanceStore<'_>) -> AppResult<T> + Send + 'static,
{
    let paths = paths.clone();
    blocking(move || {
        let db = Db::open(&paths.db_file())?;
        f(&InstanceStore::new(&paths, &db))
    })
    .await
}

async fn blocking<T, F>(f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?
}
