//! 从本地 `.mrpack` 安装为新实例。
//!
//! 步骤：读取并校验索引 → 新建实例 → 补全原版并安装加载器 → 并行下载 `files`（按 SHA1 校验）
//! → 解压 `overrides/`、`client-overrides/`。任一步失败或取消时删除新实例（回滚），
//! 已下载到 `shared/` 的原版与加载器文件保留以便重试。

use std::path::{Path, PathBuf};

use tmcl_common::{AppError, AppResult, DataPaths, Db, ErrorCode, TaskCtx};
use tmcl_game::{InstallOptions, install_version};
use tmcl_instance::{InstanceConfig, InstanceStore, LoaderSpec};
use tmcl_loader::LoaderInstaller;

use super::mrpack::{self, Mrpack};

/// 下载阶段名。
pub const FILES_STAGE: &str = "modpack_files";

/// 安装整合包所需的环境。新实例的 Java 为自动，因此 `loader` 应使用 [`tmcl_loader::JavaChoice::Auto`]。
#[derive(Debug, Clone)]
pub struct ModpackInstaller {
    loader: LoaderInstaller,
    game: InstallOptions,
}

impl ModpackInstaller {
    /// `loader` 提供下载引擎、数据目录与加载器端点；`game` 为补全原版的选项。
    pub fn new(loader: LoaderInstaller, game: InstallOptions) -> Self {
        Self { loader, game }
    }

    /// 读取并校验整合包，不写任何文件。
    pub async fn inspect(&self, pack: &Path) -> AppResult<Mrpack> {
        let pack = pack.to_owned();
        blocking(move || mrpack::read(&pack)).await
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
        parsed: &Mrpack,
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

        let (pack, dir, task) = (pack.to_owned(), game_dir.clone(), ctx.clone());
        blocking(move || mrpack::extract_overrides(&pack, &dir, &task)).await?;
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
