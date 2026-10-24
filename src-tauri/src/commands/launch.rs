//! `launch_*` 命令：启动游戏、强制结束、列出运行中的游戏。
//!
//! 启动是一个任务（种类 `launch`）：启动前检查与准备（补全文件、选择 Java、解压 natives）
//! 的进度经 `task:progress` 推送，游戏进程创建后任务即以 `succeeded` 结束；
//! 之后游戏输出以 `game:log` 批量推送，退出时推送一次 `game:exited`。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime, State};
use tmcl_auth::accounts::{self, Account, AccountKind};
use tmcl_auth::yggdrasil::Yggdrasil;
use tmcl_auth::{MsClient, OfflineAccount, SecretStore, authlib, store};
use tmcl_common::{AppError, AppResult, DataPaths, ErrorCode, TaskCtx};
use tmcl_game::InstallOptions;
use tmcl_instance::{InstanceConfig, InstanceStore};
use tmcl_java::RuntimeInstaller;
use tmcl_launch::{KillHandle, LaunchAccount, LaunchEnv, LogStream, prepare, spawn};
use tmcl_net::{Downloader, HttpClient};
use ts_rs::TS;

use crate::events::{
    GAME_EXITED, GAME_LOG, GameExited, GameLog, GameLogStream, TaskStarted, emit, spawn_task,
};
use crate::state::AppState;

/// 启动任务的种类名。
pub const LAUNCH_KIND: &str = "launch";

/// 游戏日志批量推送的间隔。
const LOG_BATCH_INTERVAL: Duration = Duration::from_millis(100);

/// 启动时使用的账号。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchAccountArgs {
    /// 离线账号（仅限本地测试）。用户名不合法时返回 `AUTH_OFFLINE_INVALID_NAME`。
    Offline {
        /// 玩家名。
        name: String,
    },
    /// 已保存的账号（`auth_list` 中的 ID）。不存在时返回 `AUTH_ACCOUNT_NOT_FOUND`；
    /// 外置登录账号在任务中刷新令牌并加载 authlib-injector。
    Account {
        /// 账号 ID。
        id: String,
    },
}

/// 解析后的启动账号：离线账号可直接使用，外置登录账号需在任务中联网准备。
enum ResolvedAccount {
    Ready(LaunchAccount),
    Authlib(Account),
    Microsoft(Account),
}

/// `launch_start` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct LaunchStartArgs {
    /// 要启动的实例。
    pub instance_id: String,
    /// 账号。
    pub account: LaunchAccountArgs,
}

/// 以实例 ID 为参数的 `launch_*` 命令参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct LaunchInstanceArgs {
    /// 实例 ID。
    pub instance_id: String,
}

/// `launch_kill` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LaunchKillResult {
    /// 是否向运行中的游戏发出了结束请求；未在运行（或仍在启动前准备中）时为 `false`。
    pub killed: bool,
}

/// 一个运行中的游戏。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct RunningGame {
    /// 实例 ID。
    pub instance_id: String,
    /// 游戏进程 ID。
    pub pid: u32,
}

/// `launch_running` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LaunchRunning {
    /// 运行中的游戏，按实例 ID 排序；不含仍在启动前准备中的实例。
    pub games: Vec<RunningGame>,
}

/// 各实例的启动状态：同一实例同时只能有一个游戏进程。
#[derive(Debug, Default)]
pub struct GameRegistry {
    games: Mutex<BTreeMap<String, Slot>>,
}

#[derive(Debug)]
enum Slot {
    /// 启动任务正在准备。
    Starting,
    /// 游戏进程运行中。
    Running { pid: u32, kill: KillHandle },
}

impl GameRegistry {
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Slot>> {
        self.games
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 占用实例；已在启动或运行时返回 `LAUNCH_ALREADY_RUNNING`。
    fn reserve(&self, instance_id: &str) -> AppResult<()> {
        let mut games = self.lock();
        if games.contains_key(instance_id) {
            return Err(
                AppError::new(ErrorCode::LaunchAlreadyRunning).with_param("id", instance_id)
            );
        }
        games.insert(instance_id.to_owned(), Slot::Starting);
        Ok(())
    }

    fn set_running(&self, instance_id: &str, pid: u32, kill: KillHandle) {
        self.lock()
            .insert(instance_id.to_owned(), Slot::Running { pid, kill });
    }

    fn release(&self, instance_id: &str) {
        self.lock().remove(instance_id);
    }

    /// 请求结束实例的游戏进程。
    pub fn kill(&self, instance_id: &str) -> bool {
        match self.lock().get(instance_id) {
            Some(Slot::Running { kill, .. }) => {
                kill.kill();
                true
            }
            _ => false,
        }
    }

    /// 运行中的游戏。
    pub fn running(&self) -> Vec<RunningGame> {
        self.lock()
            .iter()
            .filter_map(|(id, slot)| match slot {
                Slot::Running { pid, .. } => Some(RunningGame {
                    instance_id: id.clone(),
                    pid: *pid,
                }),
                Slot::Starting => None,
            })
            .collect()
    }
}

/// 启动实例，立即返回任务 ID。
///
/// 实例不存在返回 `INSTANCE_NOT_FOUND`，已在启动或运行返回 `LAUNCH_ALREADY_RUNNING`。
/// 任务阶段依次为 `verify`、（需要补全时）`version_json`…`assets`、（需要下载 Java 时）Java 下载阶段；
/// 游戏进程创建后任务成功，并记录实例的最近启动时间。
#[tauri::command]
pub async fn launch_start<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: LaunchStartArgs,
) -> Result<TaskStarted, AppError> {
    let paths = state.data()?.clone();
    let instance =
        state.with_db(|paths, db| InstanceStore::new(paths, db).get(&args.instance_id))?;
    let account = state.with_db(|_, db| launch_account(db, &args.account))?;
    let secrets = state.secrets.clone();
    let ms = state.ms.clone();
    let downloader = state.downloader()?;
    let install = state.game.clone();
    let games = state.games.clone();
    games.reserve(&instance.id)?;
    let handle = app.clone();
    Ok(spawn_task(&app, LAUNCH_KIND, move |ctx| async move {
        let id = instance.id.clone();
        let result = async {
            let (instance, account) =
                with_account(&paths, secrets.as_ref(), &ms, instance, account, &ctx).await?;
            start(
                handle,
                &paths,
                &downloader,
                &install,
                &games,
                instance,
                account,
                &ctx,
            )
            .await
        }
        .await;
        if result.is_err() {
            games.release(&id);
        }
        result
    }))
}

/// 强制结束实例的游戏进程；进程退出后照常推送 `game:exited`（`crashed` 为 `false`）。
#[tauri::command]
pub async fn launch_kill(
    state: State<'_, AppState>,
    args: LaunchInstanceArgs,
) -> Result<LaunchKillResult, AppError> {
    Ok(LaunchKillResult {
        killed: state.games.kill(&args.instance_id),
    })
}

/// 列出运行中的游戏，供界面重新打开后恢复状态。
#[tauri::command]
pub async fn launch_running(state: State<'_, AppState>) -> Result<LaunchRunning, AppError> {
    Ok(LaunchRunning {
        games: state.games.running(),
    })
}

fn offline(name: &str) -> AppResult<LaunchAccount> {
    let account = OfflineAccount::new(name)?;
    Ok(LaunchAccount::offline(
        &account.name,
        &account.uuid.simple().to_string(),
    ))
}

fn launch_account(db: &tmcl_common::Db, args: &LaunchAccountArgs) -> AppResult<ResolvedAccount> {
    match args {
        LaunchAccountArgs::Offline { name } => offline(name).map(ResolvedAccount::Ready),
        LaunchAccountArgs::Account { id } => {
            let account = accounts::get(db, id)?.ok_or_else(|| {
                AppError::new(ErrorCode::AuthAccountNotFound).with_param("id", id)
            })?;
            match account.kind {
                AccountKind::Offline => offline(&account.name).map(ResolvedAccount::Ready),
                AccountKind::Authlib => Ok(ResolvedAccount::Authlib(account)),
                AccountKind::Microsoft => Ok(ResolvedAccount::Microsoft(account)),
            }
        }
    }
}

/// 微软账号：令牌快过期时续期。外置登录账号：刷新令牌、下载 authlib-injector，
/// 并把 `-javaagent` 等参数放到实例 JVM 参数最前。
async fn with_account(
    paths: &DataPaths,
    secrets: &dyn SecretStore,
    ms: &AppResult<MsClient>,
    mut instance: InstanceConfig,
    account: ResolvedAccount,
    ctx: &TaskCtx,
) -> AppResult<(InstanceConfig, LaunchAccount)> {
    let account = match account {
        ResolvedAccount::Ready(account) => return Ok((instance, account)),
        ResolvedAccount::Microsoft(account) => {
            let ms = ms.as_ref().map_err(Clone::clone)?;
            let session = store::prepare_launch(secrets, ms, &account, ctx).await?;
            return Ok((
                instance,
                LaunchAccount {
                    name: session.profile.name,
                    uuid: session.profile.id,
                    access_token: session.access_token,
                    user_type: "msa".to_owned(),
                },
            ));
        }
        ResolvedAccount::Authlib(account) => account,
    };
    let http = HttpClient::new()?;
    let prepared =
        authlib::prepare_launch(secrets, &http, &Yggdrasil::new()?, &account, ctx).await?;
    let jar = authlib::ensure_injector(
        &http,
        &paths.cache_dir().join("authlib-injector"),
        &[authlib::ARTIFACT_URL, authlib::ARTIFACT_MIRROR_URL],
        ctx,
    )
    .await?;
    let api_url = account.server.map(|s| s.api_url).unwrap_or_default();
    let mut jvm_args = authlib::jvm_args(&jar, &api_url, &prepared.metadata);
    jvm_args.append(&mut instance.jvm_args);
    instance.jvm_args = jvm_args;
    let profile = prepared.session.profile;
    Ok((
        instance,
        LaunchAccount {
            name: profile.name,
            uuid: profile.id,
            access_token: prepared.session.access_token,
            user_type: "mojang".to_owned(),
        },
    ))
}

#[allow(clippy::too_many_arguments)]
async fn start<R: Runtime>(
    app: AppHandle<R>,
    paths: &DataPaths,
    downloader: &Downloader,
    install: &InstallOptions,
    games: &Arc<GameRegistry>,
    instance: InstanceConfig,
    account: LaunchAccount,
    ctx: &TaskCtx,
) -> AppResult<()> {
    let java = RuntimeInstaller::new(downloader.clone(), paths.java_dir());
    let env = LaunchEnv {
        paths,
        downloader,
        java: &java,
        install,
    };
    let prepared = prepare(env, &instance, account, ctx).await?;
    for warning in &prepared.warnings {
        tracing::warn!(instance = %instance.id, warning = %warning, "launch warning");
    }
    tracing::info!(
        instance = %instance.id,
        java = %prepared.java.path.display(),
        command = %prepared.command.redacted(),
        "launching game"
    );
    let process = match spawn(&prepared.command) {
        Ok(process) => process,
        Err(err) => {
            let _ = std::fs::remove_dir_all(&prepared.natives_dir);
            return Err(err);
        }
    };
    let pid = process.pid();
    games.set_running(&instance.id, pid, process.kill_handle());
    if let Some(state) = app.try_state::<AppState>()
        && let Err(err) =
            state.with_db(|paths, db| InstanceStore::new(paths, db).mark_played(&instance.id))
    {
        tracing::warn!(instance = %instance.id, error = %err, "failed to record last played");
    }

    let games = games.clone();
    let instance_id = instance.id;
    let natives_dir = prepared.natives_dir;
    tauri::async_runtime::spawn(async move {
        let exit = process
            .run(LOG_BATCH_INTERVAL, |stream, lines| {
                let stream = match stream {
                    LogStream::Stdout => GameLogStream::Stdout,
                    LogStream::Stderr => GameLogStream::Stderr,
                };
                emit(
                    &app,
                    GAME_LOG,
                    &GameLog {
                        instance_id: instance_id.clone(),
                        pid,
                        stream,
                        lines,
                    },
                );
            })
            .await;
        let (exit_code, crashed) = match exit {
            Ok(exit) => (exit.exit_code, exit.crashed),
            Err(err) => {
                tracing::warn!(instance = %instance_id, error = %err, "failed to wait for game");
                (None, true)
            }
        };
        tracing::info!(instance = %instance_id, ?exit_code, crashed, "game exited");
        games.release(&instance_id);
        let _ = std::fs::remove_dir_all(&natives_dir);
        emit(
            &app,
            GAME_EXITED,
            &GameExited {
                instance_id,
                pid,
                exit_code,
                crashed,
            },
        );
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_allows_one_game_per_instance() {
        let games = GameRegistry::default();
        games.reserve("a").unwrap();
        let err = games.reserve("a").unwrap_err();
        assert_eq!(err.code, ErrorCode::LaunchAlreadyRunning);
        assert!(!games.kill("a"), "准备中的实例不能强制结束");
        assert!(games.running().is_empty());

        let kill = tmcl_launch::process::KillHandle::default();
        games.set_running("a", 42, kill.clone());
        assert_eq!(
            games.running(),
            [RunningGame {
                instance_id: "a".into(),
                pid: 42
            }]
        );
        assert!(games.kill("a"));
        assert!(kill.is_killed());
        games.release("a");
        games.reserve("a").unwrap();
    }
}
