//! `p2p_*` 命令：P2P 直连联机（PLAN.md M2-12）。
//!
//! 开房与加入是后台任务：先按需下载 EasyTier（上报字节进度），再启动联机；任务结束后界面
//! 轮询 `p2p_status` 取房间码、玩家列表与本机连接地址。同一时间只有一个房间。
//! 应用退出时 `lib.rs` 调用 [`stop_on_exit`] 结束 EasyTier；强制结束后残留的进程在下次
//! 启动时由 [`cleanup_on_start`] 清理。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tmcl_common::config::Config;
use tmcl_common::{AppError, AppResult, DataPaths, ErrorCode, TaskCtx};
use tmcl_p2p::{
    PlayerKind, PlayerProfile, Role, RoomCode, Session, SessionOptions, effective_nodes,
    ensure_installed, signal,
};
use tokio::sync::Mutex;
use ts_rs::TS;

use crate::events::{TaskStarted, spawn_task};
use crate::state::AppState;

/// 开房任务的种类名。
pub const P2P_HOST_KIND: &str = "p2p_host";
/// 加入房间任务的种类名。
pub const P2P_JOIN_KIND: &str = "p2p_join";
/// 监听局域网广播的时长。
const LAN_SCAN: Duration = Duration::from_secs(2);
/// 玩家名长度上限（字符）。
const MAX_NAME: usize = 64;
/// 应用退出时等待 EasyTier 结束的最长时间。
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);

/// 当前的联机会话与“正在开房 / 加入”标记。
#[derive(Default)]
pub struct P2pState {
    session: Mutex<Option<Session>>,
    starting: AtomicBool,
}

impl std::fmt::Debug for P2pState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P2pState")
            .field("starting", &self.starting)
            .finish_non_exhaustive()
    }
}

/// `p2p_info` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct P2pInfo {
    /// 运行时下载的 EasyTier 版本，如 `v2.6.4`。
    pub easytier_version: String,
    /// 内置的默认公共节点，按使用顺序排列。
    pub default_nodes: Vec<String>,
}

/// `p2p_host` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct P2pHostArgs {
    /// 房主已对局域网开放的世界端口。
    pub mc_port: u16,
    /// 房主显示在玩家列表中的名字。
    pub player_name: String,
}

/// `p2p_join` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct P2pJoinArgs {
    /// 房间码 `U/NNNN-NNNN-SSSS-SSSS`。
    pub room_code: String,
    /// 本机玩家名。
    pub player_name: String,
}

/// 本机对局域网开放的一个世界。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LanWorld {
    pub motd: String,
    pub port: u16,
}

/// `p2p_lan_worlds` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LanWorldList {
    pub worlds: Vec<LanWorld>,
}

/// 本机在房间中的身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum P2pRole {
    Host,
    Guest,
}

/// 房间里的一个玩家。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct P2pPlayer {
    pub name: String,
    /// 对方启动器，如 `TMCL 0.1.0`、`HMCL 3.6`。
    pub vendor: String,
    pub kind: P2pRole,
    /// 是否为本机。
    pub is_self: bool,
}

/// 当前房间。
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct P2pRoom {
    pub role: P2pRole,
    /// 完整房间码。
    pub code: String,
    /// 房主上的游戏端口。
    pub mc_port: u16,
    /// 房客在游戏中直接连接的 `127.0.0.1:<端口>`。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub local_address: Option<String>,
    /// 房主在前，其余按名字排序。
    pub players: Vec<P2pPlayer>,
    /// 房客到房主的延迟（毫秒）。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub latency_ms: Option<f64>,
    /// 房客与房主是否直连。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub direct: Option<bool>,
    /// 本机 NAT 类型（EasyTier 原始名称）。
    pub nat_type: String,
}

/// `p2p_status` 的返回值。
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct P2pStatus {
    /// 开房或加入的任务仍在进行。
    pub starting: bool,
    /// EasyTier 在运行，且房客最近收到过房主响应；有房间但为 `false` 时应调用 `p2p_stop`。
    pub running: bool,
    pub room: Option<P2pRoom>,
}

/// 读取 P2P 联机的静态信息。
#[tauri::command]
pub fn p2p_info() -> P2pInfo {
    P2pInfo {
        easytier_version: tmcl_p2p::EASYTIER_VERSION.to_owned(),
        default_nodes: tmcl_p2p::DEFAULT_PUBLIC_NODES
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
    }
}

/// 本机已对局域网开放的世界。
#[tauri::command]
pub async fn p2p_lan_worlds(state: State<'_, AppState>) -> Result<LanWorldList, AppError> {
    let exclude: Vec<u16> = state
        .p2p
        .session
        .lock()
        .await
        .as_ref()
        .and_then(Session::announced_port)
        .into_iter()
        .collect();
    let worlds = tmcl_p2p::lan::scan(LAN_SCAN, &exclude).await?;
    Ok(LanWorldList {
        worlds: worlds
            .into_iter()
            .map(|w| LanWorld {
                motd: w.motd,
                port: w.port,
            })
            .collect(),
    })
}

/// 开房（后台任务）。
#[tauri::command]
pub async fn p2p_host<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: P2pHostArgs,
) -> Result<TaskStarted, AppError> {
    if args.mc_port == 0 {
        return Err(AppError::new(ErrorCode::CommonInternal).with_param("mc_port", false));
    }
    let options = options(&state, &args.player_name)?;
    let mc_port = args.mc_port;
    start(&app, &state, P2P_HOST_KIND, move |bins, ctx| async move {
        Session::host(&bins, mc_port, options, &ctx).await
    })
    .await
}

/// 加入房间（后台任务）。房间码无效时直接返回 `P2P_ROOM_CODE_INVALID`。
#[tauri::command]
pub async fn p2p_join<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: P2pJoinArgs,
) -> Result<TaskStarted, AppError> {
    let room = RoomCode::parse(&args.room_code)?;
    let options = options(&state, &args.player_name)?;
    start(&app, &state, P2P_JOIN_KIND, move |bins, ctx| async move {
        Session::join(&bins, room, options, &ctx).await
    })
    .await
}

/// 当前房间状态；只访问本机 EasyTier。
#[tauri::command]
pub async fn p2p_status(state: State<'_, AppState>) -> Result<P2pStatus, AppError> {
    let starting = state.p2p.starting.load(Ordering::SeqCst);
    let slot = state.p2p.session.lock().await;
    let Some(session) = slot.as_ref() else {
        return Ok(P2pStatus {
            starting,
            running: false,
            room: None,
        });
    };
    let status = session.status().await;
    let role = |role| match role {
        Role::Host => P2pRole::Host,
        Role::Guest => P2pRole::Guest,
    };
    Ok(P2pStatus {
        starting,
        running: status.running,
        room: Some(P2pRoom {
            role: role(status.role),
            code: status.code,
            mc_port: status.mc_port,
            local_address: status.local_address.map(|a| a.to_string()),
            players: status
                .players
                .into_iter()
                .map(|(p, is_self)| P2pPlayer {
                    name: p.name,
                    vendor: p.vendor,
                    kind: match p.kind {
                        PlayerKind::Host => P2pRole::Host,
                        PlayerKind::Guest => P2pRole::Guest,
                    },
                    is_self,
                })
                .collect(),
            latency_ms: status.latency_ms,
            direct: status.direct,
            nat_type: status.nat_type,
        }),
    })
}

/// 关闭或退出房间；没有房间时什么也不做。
#[tauri::command]
pub async fn p2p_stop(state: State<'_, AppState>) -> Result<(), AppError> {
    let session = state.p2p.session.lock().await.take();
    match session {
        Some(session) => session.stop().await,
        None => Ok(()),
    }
}

/// 本机玩家与节点设置。
fn options(state: &AppState, player_name: &str) -> AppResult<SessionOptions> {
    let name = player_name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(AppError::new(ErrorCode::CommonInternal).with_param("player_name", false));
    }
    let paths = state.data()?;
    let p2p = Config::load(&paths.config_file())?.config.p2p;
    let state_dir = paths.p2p_dir();
    Ok(SessionOptions {
        player: PlayerProfile {
            name: name.to_owned(),
            machine_id: signal::machine_id(&state_dir)?,
            easytier_id: None,
            vendor: format!("TMCL {}", env!("CARGO_PKG_VERSION")),
            kind: PlayerKind::Guest,
        },
        peers: effective_nodes(p2p.use_default_nodes, &p2p.custom_nodes),
        state_dir,
    })
}

/// 检查没有房间、没有正在进行的开房或加入，然后在后台下载 EasyTier 并运行 `job`。
async fn start<R, F, Fut>(
    app: &AppHandle<R>,
    state: &AppState,
    kind: &str,
    job: F,
) -> AppResult<TaskStarted>
where
    R: Runtime,
    F: FnOnce(tmcl_p2p::EasyTierBinaries, TaskCtx) -> Fut + Send + 'static,
    Fut: Future<Output = AppResult<Session>> + Send,
{
    let p2p = state.p2p.clone();
    if p2p.session.lock().await.is_some() || p2p.starting.swap(true, Ordering::SeqCst) {
        return Err(AppError::new(ErrorCode::P2pAlreadyRunning));
    }
    let paths: DataPaths = state.data()?.clone();
    let downloader = match state.downloader() {
        Ok(d) => d,
        Err(e) => {
            p2p.starting.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };
    Ok(spawn_task(app, kind, move |ctx| async move {
        let result = async {
            let bins = ensure_installed(&downloader, &paths, &ctx).await?;
            ctx.check_cancelled()?;
            job(bins, ctx).await
        }
        .await;
        let result = match result {
            Ok(session) => {
                *p2p.session.lock().await = Some(session);
                Ok(())
            }
            Err(e) => Err(e),
        };
        p2p.starting.store(false, Ordering::SeqCst);
        result
    }))
}

/// 应用退出时结束 EasyTier，最多等待 [`EXIT_TIMEOUT`]。
pub fn stop_on_exit(p2p: &P2pState) {
    let Ok(mut slot) = p2p.session.try_lock() else {
        return;
    };
    let Some(session) = slot.take() else {
        return;
    };
    tauri::async_runtime::block_on(async {
        match tokio::time::timeout(EXIT_TIMEOUT, session.stop()).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!("退出时结束 EasyTier 失败：{e}"),
            Err(_) => tracing::warn!("退出时结束 EasyTier 超时"),
        }
    });
}

/// 启动时清理上次被强制结束后残留的 `easytier-core`。
pub fn cleanup_on_start(state: &AppState) {
    if let Ok(paths) = state.data() {
        let dir = paths.p2p_dir();
        tauri::async_runtime::spawn(async move { tmcl_p2p::cleanup_orphan(&dir).await });
    }
}
