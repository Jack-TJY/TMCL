//! 启动前检查与准备：补全游戏文件、选择 Java、解压 natives、整理旧版资源，最后拼装命令。
//!
//! 账号令牌的有效性检查与刷新由 M1-08 接入；目前只有离线账号，无需检查。

use std::path::PathBuf;

use tmcl_common::{AppError, AppResult, DataPaths, ErrorCode, TaskCtx};
use tmcl_game::{
    AssetIndex, GameFiles, InstallOptions, extract_natives, game_assets_dir, new_natives_dir,
    prepare_legacy_assets, repair_version, resolve, verify_version,
};
use tmcl_instance::{InstanceConfig, JavaSetting};
use tmcl_java::{Arch, JavaInstall, RuntimeInstaller, Sources, check_manual, ensure, probe};
use tmcl_net::Downloader;

use crate::args::{LaunchAccount, LaunchCommand, LaunchSpec, build_command, rule_env};

/// 准备启动所需的服务。
#[derive(Debug, Clone, Copy)]
pub struct LaunchEnv<'a> {
    /// 数据目录。
    pub paths: &'a DataPaths,
    /// 补全游戏文件用的下载引擎。
    pub downloader: &'a Downloader,
    /// 自动模式下没有合适 Java 时下载运行时。
    pub java: &'a RuntimeInstaller,
    /// 版本清单与资源文件地址。
    pub install: &'a InstallOptions,
}

/// 准备好的一次启动。
#[derive(Debug, Clone)]
pub struct PreparedLaunch {
    /// 启动命令。
    pub command: LaunchCommand,
    /// 本次启动的 natives 目录，游戏退出后由调用方删除。
    pub natives_dir: PathBuf,
    /// 使用的 Java。
    pub java: JavaInstall,
    /// 不阻止启动的警告，例如手动指定的 Java 版本不符（`JAVA_VERSION_MISMATCH`）。
    pub warnings: Vec<AppError>,
    /// 启动前补全（下载）的文件数。
    pub repaired: u64,
}

/// 实例要启动的版本号：原版为游戏版本号。
///
/// 加载器版本由 M1-09 安装后才有，在此之前带加载器的实例返回 `LAUNCH_VERSION_NOT_INSTALLED`。
pub fn version_id(instance: &InstanceConfig) -> AppResult<&str> {
    match &instance.loader {
        None => Ok(&instance.game_version),
        Some(loader) => Err(AppError::new(ErrorCode::LaunchVersionNotInstalled)
            .with_param("version", &instance.game_version)
            .with_param("loader", loader.kind.as_str())),
    }
}

/// 启动前检查并准备 `instance`：
///
/// 1. 校验游戏文件（阶段 `verify`），缺失或损坏时按 [`repair_version`] 补全（原版尚未下载时即为完整安装）；
/// 2. 选择 Java：自动模式按版本要求探测，没有时下载运行时；手动指定时只给警告；
/// 3. 解压 natives 到新目录，旧版本（`legacy` / `pre-1.6` 资源索引）复制资源到虚拟目录；
/// 4. 按实例的内存、自定义参数与窗口设置拼装命令。
///
/// 可取消；取消或失败时已创建的 natives 目录会被删除。
pub async fn prepare(
    env: LaunchEnv<'_>,
    instance: &InstanceConfig,
    account: LaunchAccount,
    ctx: &TaskCtx,
) -> AppResult<PreparedLaunch> {
    let id = version_id(instance)?;
    let repaired = ensure_files(env, id, ctx).await?;
    ctx.check_cancelled()?;

    let files = GameFiles::new(env.paths.clone());
    let version = resolve(id, |v| files.load_version(v))?;
    let (java, warnings) = select_java(env, instance, &version, ctx).await?;
    ctx.check_cancelled()?;

    let game_dir = env.paths.instance_game_dir(&instance.id);
    let index_id = version
        .asset_index
        .as_ref()
        .map(|i| i.id.clone())
        .or_else(|| version.assets.clone());
    let game_assets = match &index_id {
        Some(index_id) => {
            let index = AssetIndex::load(&files, index_id)?;
            let dir = game_assets_dir(&files, index_id, &index, &game_dir);
            let (files, index_id, game_dir) = (files.clone(), index_id.clone(), game_dir.clone());
            blocking(move || prepare_legacy_assets(&files, &index_id, &index, &game_dir)).await?;
            dir
        }
        None => files.assets_dir(),
    };

    let window = &instance.window;
    let spec = LaunchSpec {
        java: java.path.clone(),
        version,
        game_dir,
        game_assets_dir: game_assets,
        natives_dir: new_natives_dir(env.paths, id),
        account,
        min_memory_mb: instance.memory.min_mb.min(instance.memory.max_mb),
        max_memory_mb: instance.memory.max_mb,
        jvm_args: instance.jvm_args.clone(),
        game_args: instance.game_args.clone(),
        window: (window.width > 0 && window.height > 0).then_some((window.width, window.height)),
        fullscreen: window.fullscreen,
    };
    let natives_dir = spec.natives_dir.clone();
    let extracted = {
        let (files, version, env, dest) = (
            files.clone(),
            spec.version.clone(),
            rule_env(&spec),
            natives_dir.clone(),
        );
        blocking(move || extract_natives(&files, &version, &env, &dest)).await
    };
    if let Err(e) = extracted.and_then(|_| ctx.check_cancelled()) {
        let _ = std::fs::remove_dir_all(&natives_dir);
        return Err(e);
    }
    Ok(PreparedLaunch {
        command: build_command(&files, &spec),
        natives_dir,
        java,
        warnings,
        repaired,
    })
}

/// 校验并在需要时补全，返回下载的文件数。
async fn ensure_files(env: LaunchEnv<'_>, id: &str, ctx: &TaskCtx) -> AppResult<u64> {
    let intact = match verify_version(env.paths, id, ctx).await {
        Ok(report) => report.is_ok(),
        // 版本 JSON 链读不到（尚未安装）或已损坏：交给补全重新获取。
        Err(e)
            if matches!(
                e.code,
                ErrorCode::CommonIo | ErrorCode::GameVersionJsonInvalid
            ) =>
        {
            false
        }
        Err(e) => return Err(e),
    };
    if intact {
        return Ok(0);
    }
    let report = repair_version(env.downloader, env.paths, id, env.install, ctx).await?;
    Ok(report.downloaded)
}

async fn select_java(
    env: LaunchEnv<'_>,
    instance: &InstanceConfig,
    version: &tmcl_game::VersionJson,
    ctx: &TaskCtx,
) -> AppResult<(JavaInstall, Vec<AppError>)> {
    let major = version.java_major();
    match &instance.java {
        JavaSetting::Auto => {
            let component = version
                .java_version
                .as_ref()
                .and_then(|j| j.component.as_deref());
            let sources = Sources::system(env.java.java_dir());
            let java = ensure(env.java, &sources, major, component, ctx).await?;
            Ok((java, Vec::new()))
        }
        JavaSetting::Path { path } => {
            let java = probe(std::path::Path::new(path), false).await?;
            let warnings = check_manual(&java, major, Arch::host());
            Ok((java, warnings))
        }
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?
}
