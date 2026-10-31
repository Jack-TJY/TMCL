//! `sakura_*` 命令（PLAN.md M2-14）：樱花穿透的访问密钥、账号、节点、条款与联机会话。
//!
//! 访问密钥只存系统密钥库，任何命令都不把它返回给界面。开始联机是后台任务：先下载并校验
//! frpc（上报进度），再建临时隧道并启动 frpc；任务结束后界面用 `sakura_status` 取联机地址。
//! 应用退出时 `lib.rs` 调用 [`stop_on_exit`] 结束 frpc 并删除临时隧道。

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tmcl_auth::SecretStore;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::HttpClient;
use tmcl_sakura::frpc::START_TIMEOUT;
use tmcl_sakura::tunnel;
use tmcl_sakura::{
    SakuraClient, SakuraNode, SakuraPolicy, SakuraRoom, SakuraUser, Session, StartRequest,
    ensure_frpc,
};
use tokio::sync::Mutex;
use ts_rs::TS;

use crate::events::{TaskStarted, spawn_task};
use crate::state::AppState;

/// 开始联机任务的种类名。
pub const SAKURA_START_KIND: &str = "sakura_start";

/// 应用退出时删除临时隧道最多等待的时间。
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);

/// `sakura_ping` 单次连接最多等待的时间。
const PING_TIMEOUT: Duration = Duration::from_secs(3);

/// `sakura_ping` 连接的次数，取最快的一次。
const PING_ATTEMPTS: usize = 3;

/// `sakura_status` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SakuraStatus {
    /// 是否已保存访问密钥。
    pub has_key: bool,
    /// 当前联机房间；没有联机时为 `null`。
    pub room: Option<SakuraRoom>,
    /// frpc 是否仍在运行；为 `false` 且有房间时说明 frpc 意外退出，需要停止后重新开始。
    pub running: bool,
    /// 开始联机的任务正在进行（下载 frpc、建隧道或等待地址）。
    pub starting: bool,
}

/// `sakura_ping` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SakuraPingArgs {
    /// 联机地址 `主机:端口`，例如 `frp-xxx.com:10086`。
    pub address: String,
}

/// `sakura_ping` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SakuraPing {
    /// 建立 TCP 连接最快一次的耗时（毫秒）；地址无效或连不上时为 `null`。
    pub latency_ms: Option<u32>,
}

/// `sakura_save_key` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SakuraKeyArgs {
    /// 用户从 SakuraFrp 用户信息页复制的访问密钥。
    pub key: String,
}

/// 节点及其可用性。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SakuraNodeInfo {
    #[serde(flatten)]
    pub node: SakuraNode,
    /// 在线且未满载，可以创建隧道。
    pub available: bool,
    /// 允许用于公开游戏服务器。
    pub public_game: bool,
}

/// `sakura_nodes` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SakuraNodeList {
    /// 按节点 ID 排序。
    pub nodes: Vec<SakuraNodeInfo>,
}

fn api(state: &AppState) -> AppResult<SakuraClient> {
    SakuraClient::with_base(&state.sakura_api)
}

/// 是否已保存访问密钥与当前联机状态；不访问网络。
#[tauri::command]
pub async fn sakura_status(state: State<'_, AppState>) -> Result<SakuraStatus, AppError> {
    let has_key = tunnel::load_key(state.secrets.as_ref()).is_ok();
    // 开始联机的任务在整个过程中持有锁，此时只报告“正在开始”。
    let Ok(mut session) = state.sakura.try_lock() else {
        return Ok(SakuraStatus {
            has_key,
            room: None,
            running: false,
            starting: true,
        });
    };
    Ok(SakuraStatus {
        has_key,
        room: session.as_ref().map(|s| s.room.clone()),
        running: session.as_mut().is_some_and(Session::is_running),
        starting: false,
    })
}

/// 校验并保存访问密钥，返回账号信息；密钥无效时不保存。
#[tauri::command]
pub async fn sakura_save_key(
    state: State<'_, AppState>,
    args: SakuraKeyArgs,
) -> Result<SakuraUser, AppError> {
    let ctx = TaskCtx::noop("sakura");
    tunnel::save_key(state.secrets.as_ref(), &api(&state)?, &args.key, &ctx).await
}

/// 删除保存的访问密钥；正在联机时先停止联机。
#[tauri::command]
pub async fn sakura_clear_key(state: State<'_, AppState>) -> Result<(), AppError> {
    stop(&state).await?;
    tunnel::clear_key(state.secrets.as_ref())
}

/// 用保存的访问密钥读取账号信息；`verified` 为 `false` 时界面提示去官网实名。
#[tauri::command]
pub async fn sakura_user(state: State<'_, AppState>) -> Result<SakuraUser, AppError> {
    let key = tunnel::load_key(state.secrets.as_ref())?;
    api(&state)?.user_info(&key, &TaskCtx::noop("sakura")).await
}

/// 节点列表。
#[tauri::command]
pub async fn sakura_nodes(state: State<'_, AppState>) -> Result<SakuraNodeList, AppError> {
    let key = tunnel::load_key(state.secrets.as_ref())?;
    let nodes = api(&state)?.nodes(&key, &TaskCtx::noop("sakura")).await?;
    Ok(SakuraNodeList {
        nodes: nodes
            .into_iter()
            .map(|node| SakuraNodeInfo {
                available: node.available(),
                public_game: node.allows_public_game(),
                node,
            })
            .collect(),
    })
}

/// SakuraFrp 服务条款，首次使用时展示给用户确认。
#[tauri::command]
pub async fn sakura_terms(state: State<'_, AppState>) -> Result<SakuraPolicy, AppError> {
    api(&state)?.terms(&TaskCtx::noop("sakura")).await
}

/// 开始联机（后台任务）：下载并校验 frpc，建临时隧道，启动 frpc 并等到联机地址。
/// 已在联机中返回 `SAKURA_ALREADY_RUNNING`；frpc 意外退出的旧会话先清理掉。
#[tauri::command]
pub async fn sakura_start<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: StartRequest,
) -> Result<TaskStarted, AppError> {
    let api = api(&state)?;
    let frpc_dir = state.data()?.cache_dir().join("sakura-frpc");
    let secrets: Arc<dyn SecretStore> = state.secrets.clone();
    let sessions = state.sakura.clone();
    Ok(spawn_task(&app, SAKURA_START_KIND, move |ctx| async move {
        let mut slot = sessions.lock().await;
        if let Some(mut old) = slot.take() {
            if old.is_running() {
                *slot = Some(old);
                return Err(AppError::new(ErrorCode::SakuraAlreadyRunning));
            }
            if let Err(e) = old.stop(&api, &ctx).await {
                tracing::warn!("清理已退出的樱花会话失败：{e}");
            }
        }
        let exe = ensure_frpc(&api, &HttpClient::new()?, &frpc_dir, &ctx).await?;
        let session =
            Session::start(&api, secrets.as_ref(), &exe, &args, START_TIMEOUT, &ctx).await?;
        *slot = Some(session);
        Ok(())
    }))
}

/// 测量到联机地址的延迟：连续建立 [`PING_ATTEMPTS`] 次 TCP 连接（不发送数据），取最快一次。
/// 连接建立在穿透节点上，因此反映的是到节点的延迟，不代表房主的游戏一定能进。
#[tauri::command]
pub async fn sakura_ping(args: SakuraPingArgs) -> Result<SakuraPing, AppError> {
    Ok(SakuraPing {
        latency_ms: ping(args.address.trim()).await,
    })
}

async fn ping(address: &str) -> Option<u32> {
    let (host, port) = address.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    if host.is_empty() || port == 0 {
        return None;
    }
    let mut best: Option<Duration> = None;
    for _ in 0..PING_ATTEMPTS {
        let started = Instant::now();
        let connect = tokio::net::TcpStream::connect((host, port));
        if let Ok(Ok(_stream)) = tokio::time::timeout(PING_TIMEOUT, connect).await {
            let elapsed = started.elapsed();
            best = Some(best.map_or(elapsed, |b| b.min(elapsed)));
        }
    }
    best.map(|d| u32::try_from(d.as_millis()).unwrap_or(u32::MAX))
}

/// 停止联机：结束 frpc 并删除临时隧道。没有联机时什么也不做。
#[tauri::command]
pub async fn sakura_stop(state: State<'_, AppState>) -> Result<(), AppError> {
    stop(&state).await
}

async fn stop(state: &AppState) -> AppResult<()> {
    let Some(session) = state.sakura.lock().await.take() else {
        return Ok(());
    };
    session.stop(&api(state)?, &TaskCtx::noop("sakura")).await
}

/// 应用退出时结束 frpc 并删除临时隧道，最多等待 [`EXIT_TIMEOUT`]；
/// 删不掉的隧道在下次开始联机时清理。
pub fn stop_on_exit(sessions: &Mutex<Option<Session>>, api_base: &str) {
    let Ok(mut slot) = sessions.try_lock() else {
        return;
    };
    let Some(session) = slot.take() else {
        return;
    };
    let Ok(api) = SakuraClient::with_base(api_base) else {
        return;
    };
    tauri::async_runtime::block_on(async {
        let ctx = TaskCtx::noop("sakura");
        let stop = session.stop(&api, &ctx);
        match tokio::time::timeout(EXIT_TIMEOUT, stop).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!("退出时删除樱花临时隧道失败：{e}"),
            Err(_) => tracing::warn!("退出时删除樱花临时隧道超时"),
        }
    });
}
