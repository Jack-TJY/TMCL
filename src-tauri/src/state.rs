//! 应用全局状态：在 `lib.rs` 中用 `manage` 注册，命令通过 `State<'_, AppState>` 取用。

use std::sync::{Arc, Mutex as StdMutex, RwLock};

use tmcl_common::config::DownloadConfig;
use tmcl_common::{AppResult, Config, DataPaths, Db};
use tmcl_game::InstallOptions;
use tmcl_instance::InstanceStore;
use tmcl_net::{DownloadOptions, Downloader, HttpClient};
use tokio::sync::Mutex;

use crate::tasks::TaskManager;

/// 命令层共享的状态。
#[derive(Debug)]
pub struct AppState {
    /// 数据目录；解析失败时保留错误，由用到它的命令返回。
    data: AppResult<DataPaths>,
    /// 串行化 `config.json` 的“读取—修改—写回”，避免并发命令互相覆盖。
    pub config_lock: Mutex<()>,
    /// 全局任务管理器。
    pub tasks: Arc<TaskManager>,
    /// 全局唯一的下载引擎，所有下载共享同一个连接数上限。
    downloader: RwLock<AppResult<Downloader>>,
    /// 游戏文件的下载地址（版本清单、资源前缀）；测试中指向本地服务器。
    pub game: InstallOptions,
    /// 共享的数据库连接，首次使用时打开。长耗时任务另开连接，避免长时间占用。
    db: StdMutex<Option<Db>>,
}

impl AppState {
    /// 按启动器安装目录解析数据目录。
    pub fn new() -> Self {
        Self::with_data(DataPaths::resolve())
    }

    /// 使用给定的数据目录（测试用）。
    ///
    /// 下载设置取自其中的 `config.json`；文件缺失或损坏时用默认值（此处只读不备份，
    /// 损坏文件留给设置命令按 `Config::load` 的规则处理）。
    pub fn with_data(data: AppResult<DataPaths>) -> Self {
        let download = data
            .as_ref()
            .ok()
            .and_then(|paths| std::fs::read(paths.config_file()).ok())
            .and_then(|bytes| Config::parse(&bytes).ok())
            .map(|config| config.download)
            .unwrap_or_default();
        Self {
            data,
            config_lock: Mutex::new(()),
            tasks: Arc::new(TaskManager::new()),
            downloader: RwLock::new(build_downloader(&download)),
            game: InstallOptions::default(),
            db: StdMutex::new(None),
        }
    }

    /// 数据目录；无法确定时返回 `COMMON_DATA_DIR_UNAVAILABLE`。
    pub fn data(&self) -> AppResult<&DataPaths> {
        self.data.as_ref().map_err(Clone::clone)
    }

    /// 在共享数据库连接上执行 `f`。
    ///
    /// 首次调用时打开 `tmcl.db`，并让实例索引与磁盘对账、清理过期回收站
    /// （PLAN.md M1-06：启动时对账）；这两步失败只记日志，下次调用不再重试。
    pub fn with_db<T>(&self, f: impl FnOnce(&DataPaths, &Db) -> AppResult<T>) -> AppResult<T> {
        let paths = self.data()?;
        let mut guard = self
            .db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let db = match guard.take() {
            Some(db) => guard.insert(db),
            None => guard.insert(open_db(paths)?),
        };
        f(paths, db)
    }

    /// 当前的下载引擎；克隆体共享同一个连接数上限。
    pub fn downloader(&self) -> AppResult<Downloader> {
        self.downloader
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

/// 打开数据库，并完成启动时的实例对账与回收站清理。
fn open_db(paths: &DataPaths) -> AppResult<Db> {
    let db = Db::open(&paths.db_file())?;
    let store = InstanceStore::new(paths, &db);
    if let Err(err) = store.reconcile() {
        tracing::warn!(error = %err, "实例索引对账失败");
    }
    if let Err(err) = store.purge_trash() {
        tracing::warn!(error = %err, "清理实例回收站失败");
    }
    Ok(db)
}

fn build_downloader(download: &DownloadConfig) -> AppResult<Downloader> {
    Ok(Downloader::new(
        HttpClient::new()?,
        DownloadOptions::from_config(download),
    ))
}
