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
    SakuraClient, SakuraNode, SakuraPolicy, SakuraRoom, SakuraTunnel, SakuraUser, Session,
    StartRequest, ensure_frpc,
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
    /// 地址后面的 Minecraft 是否回应了服务器列表查询；连不上时为 `null`。
    /// 为 `false` 表示隧道通了，但房主的游戏没开、没有对局域网开放或端口填错。
    pub game_responding: Option<bool>,
}

/// `sakura_tunnels` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SakuraTunnelList {
    /// 账号下的全部隧道，包括不是 TMCL 创建的。
    pub tunnels: Vec<SakuraTunnel>,
}

/// `sakura_delete_tunnel` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct SakuraTunnelArgs {
    #[ts(type = "number")]
    pub id: u64,
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

/// 账号下的全部隧道，用于在隧道数已满时让用户删掉不用的。
#[tauri::command]
pub async fn sakura_tunnels(state: State<'_, AppState>) -> Result<SakuraTunnelList, AppError> {
    let key = tunnel::load_key(state.secrets.as_ref())?;
    let tunnels = api(&state)?.tunnels(&key, &TaskCtx::noop("sakura")).await?;
    Ok(SakuraTunnelList { tunnels })
}

/// 删除一条隧道；是当前联机房间的隧道时等同于停止联机。
#[tauri::command]
pub async fn sakura_delete_tunnel(
    state: State<'_, AppState>,
    args: SakuraTunnelArgs,
) -> Result<(), AppError> {
    let current = state
        .sakura
        .try_lock()
        .ok()
        .and_then(|s| s.as_ref().map(|s| s.room.tunnel_id));
    if current == Some(args.id) {
        return stop(&state).await;
    }
    let key = tunnel::load_key(state.secrets.as_ref())?;
    api(&state)?
        .delete_tunnels(&key, &[args.id], &TaskCtx::noop("sakura"))
        .await
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

/// 测量到联机地址的延迟：连续建立 [`PING_ATTEMPTS`] 次 TCP 连接，取最快一次；
/// 连得上时再发一次 Minecraft 服务器列表查询，看房主的游戏是否回应。
/// TCP 连接落在穿透节点上，只反映到节点的延迟；游戏是否回应才说明朋友能不能进。
#[tauri::command]
pub async fn sakura_ping(args: SakuraPingArgs) -> Result<SakuraPing, AppError> {
    let Some((host, port)) = split_address(args.address.trim()) else {
        return Ok(SakuraPing {
            latency_ms: None,
            game_responding: None,
        });
    };
    let latency_ms = ping(host, port).await;
    let game_responding = match latency_ms {
        Some(_) => Some(game_responds(host, port).await),
        None => None,
    };
    Ok(SakuraPing {
        latency_ms,
        game_responding,
    })
}

fn split_address(address: &str) -> Option<(&str, u16)> {
    let (host, port) = address.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    (!host.is_empty() && port != 0).then_some((host, port))
}

async fn ping(host: &str, port: u16) -> Option<u32> {
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

/// 发送握手（下一状态为 status）与状态请求，[`PING_TIMEOUT`] 内收到任意回应即认为游戏在线。
/// frpc 转发不到本地端口时，节点会接受连接后立即断开，读到的是 0 字节。
async fn game_responds(host: &str, port: u16) -> bool {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let probe = async {
        let mut stream = tokio::net::TcpStream::connect((host, port)).await?;
        stream.write_all(&status_request(host, port)).await?;
        let mut buf = [0u8; 1];
        Ok::<_, std::io::Error>(stream.read(&mut buf).await? > 0)
    };
    matches!(
        tokio::time::timeout(PING_TIMEOUT, probe).await,
        Ok(Ok(true))
    )
}

/// Minecraft 握手包（协议号 -1、下一状态 1）加状态请求包。
fn status_request(host: &str, port: u16) -> Vec<u8> {
    fn varint(mut value: u32, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }
    let host = &host.as_bytes()[..host.len().min(255)];
    let mut body = vec![0x00];
    varint(u32::MAX, &mut body); // 协议号 -1：只查询状态，不关心版本。
    varint(host.len() as u32, &mut body);
    body.extend_from_slice(host);
    body.extend_from_slice(&port.to_be_bytes());
    body.push(0x01);
    let mut packet = Vec::with_capacity(body.len() + 4);
    varint(body.len() as u32, &mut packet);
    packet.extend_from_slice(&body);
    packet.extend_from_slice(&[0x01, 0x00]);
    packet
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[test]
    fn status_request_bytes() {
        let packet = status_request("a.b", 25565);
        // 长度 13：包 ID、协议号 -1（5 字节）、主机名长度与内容、端口、下一状态；随后是状态请求。
        assert_eq!(
            packet,
            [
                13, 0, 0xff, 0xff, 0xff, 0xff, 0x0f, 3, b'a', b'.', b'b', 0x63, 0xdd, 1, 1, 0
            ]
        );
    }

    #[tokio::test]
    async fn game_check_tells_closed_tunnel_from_live_game() {
        // frpc 转发失败时节点接受连接后立即断开。
        let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed_port = closed.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((stream, _)) = closed.accept().await {
                drop(stream);
            }
        });
        assert!(!game_responds("127.0.0.1", closed_port).await);

        let live = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let live_port = live.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = live.accept().await {
                let _ = stream.write_all(&[1, 0]).await;
            }
        });
        assert!(game_responds("127.0.0.1", live_port).await);
    }
}
