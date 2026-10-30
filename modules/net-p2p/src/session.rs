//! 一次 P2P 联机：房主开房或房客加入，把 EasyTier、Scaffolding 信令、端口转发与
//! 局域网广播串起来（PLAN.md M2-12 的完整流程）。
//!
//! - 房主：确认游戏端口已开放 → 生成房间码 → 起信令服务 → 以联机中心身份启动 EasyTier，
//!   入站只放行游戏端口与信令端口。
//! - 房客：以 DHCP、入站全拒的方式启动 EasyTier → 找联机中心 → 经端口转发连上信令服务
//!   （连不上即打洞失败）→ 取游戏端口 → 本机转发并发局域网广播 → 每 5 秒心跳。

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};
use tokio::task::JoinHandle;

use crate::easytier::EasyTierBinaries;
use crate::lan::Announcer;
use crate::punch::{Peer, find_center};
use crate::room::RoomCode;
use crate::signal::{
    HEARTBEAT_INTERVAL, PLAYER_TIMEOUT, PlayerKind, PlayerProfile, SignalClient, SignalServer,
    probe,
};
use crate::tunnel::{PortForward, Proto, Tunnel, TunnelConfig, TunnelState};

/// 加入房间时寻找联机中心的最长时间。
pub const CENTER_TIMEOUT: Duration = Duration::from_secs(60);
/// 找到联机中心后与其建立直连的最长时间。
pub const DIRECT_TIMEOUT: Duration = Duration::from_secs(30);
/// 轮询 EasyTier 节点列表、重试连接的间隔。
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// 记录 `easytier-core` 进程号的文件，用于清理强制结束 TMCL 后残留的进程。
const PID_FILE: &str = "easytier.pid";

/// 开房或加入时共用的参数。
#[derive(Debug, Clone)]
pub struct SessionOptions {
    /// 本机玩家：名字、设备 ID、启动器名称（`easytier_id` 与 `kind` 由会话填写）。
    pub player: PlayerProfile,
    /// EasyTier 公共节点。
    pub peers: Vec<String>,
    /// 写入进程号文件的目录（`DataPaths::p2p_dir()`）。
    pub state_dir: PathBuf,
}

/// 本机在房间中的身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Host,
    Guest,
}

/// [`Session::status`] 的结果。
#[derive(Debug, Clone, PartialEq)]
pub struct SessionStatus {
    pub role: Role,
    /// 完整房间码。
    pub code: String,
    /// 房主上的游戏端口。
    pub mc_port: u16,
    /// 房客在游戏中直接连接的本机地址。
    pub local_address: Option<SocketAddr>,
    /// 玩家与是否为本机，房主在前。
    pub players: Vec<(PlayerProfile, bool)>,
    /// 房客到房主的延迟与是否直连。
    pub latency_ms: Option<f64>,
    pub direct: Option<bool>,
    /// 本机 NAT 类型。
    pub nat_type: String,
    /// EasyTier 仍在运行，且房客最近收到过房主响应。
    pub running: bool,
}

/// 房客一侧与房主的信令状态，由心跳任务更新。
struct GuestLink {
    players: Vec<PlayerProfile>,
    last_ok: Instant,
}

/// 一次进行中的联机。被丢弃时 EasyTier、信令服务与广播都会停止。
pub struct Session {
    role: Role,
    room: RoomCode,
    mc_port: u16,
    me: String,
    tunnel: Arc<tokio::sync::Mutex<Tunnel>>,
    state_dir: PathBuf,
    server: Option<SignalServer>,
    guest: Option<GuestState>,
}

struct GuestState {
    center: Ipv4Addr,
    local_port: u16,
    link: Arc<Mutex<GuestLink>>,
    heartbeat: JoinHandle<()>,
    _announcer: Option<Announcer>,
}

impl Session {
    /// 开房。`mc_port` 为本机已对局域网开放的游戏端口。
    pub async fn host(
        binaries: &EasyTierBinaries,
        mc_port: u16,
        options: SessionOptions,
        ctx: &TaskCtx,
    ) -> AppResult<Self> {
        ctx.report(ProgressUpdate::new("p2p_start"));
        if !probe(mc_port).await {
            return Err(AppError::new(ErrorCode::P2pServerNotRunning).with_param("port", mc_port));
        }
        cleanup_orphan(&options.state_dir).await;
        let room = RoomCode::generate()?;
        let mut player = options.player;
        player.kind = PlayerKind::Host;
        let me = player.machine_id.clone();
        let server = SignalServer::start(player, mc_port).await?;
        let config = TunnelConfig::host(&room, options.peers, server.port(), mc_port);
        let tunnel = Tunnel::start(binaries, &config).await?;
        write_pid(&options.state_dir, &tunnel);
        tracing::info!(?room, mc_port, "已开房");
        Ok(Self {
            role: Role::Host,
            room,
            mc_port,
            me,
            tunnel: Arc::new(tokio::sync::Mutex::new(tunnel)),
            state_dir: options.state_dir,
            server: Some(server),
            guest: None,
        })
    }

    /// 加入房间。
    pub async fn join(
        binaries: &EasyTierBinaries,
        room: RoomCode,
        options: SessionOptions,
        ctx: &TaskCtx,
    ) -> AppResult<Self> {
        ctx.report(ProgressUpdate::new("p2p_connect"));
        cleanup_orphan(&options.state_dir).await;
        let mut tunnel =
            Tunnel::start(binaries, &TunnelConfig::guest(&room, options.peers)).await?;
        write_pid(&options.state_dir, &tunnel);
        // 出错时结束 EasyTier 再返回；`?` 之外的路径都显式停止。
        match connect(&mut tunnel, options.player, ctx).await {
            Ok(joined) => {
                let me = joined.me.machine_id.clone();
                let link = Arc::new(Mutex::new(GuestLink {
                    players: joined.players,
                    last_ok: Instant::now(),
                }));
                let heartbeat = spawn_heartbeat(joined.client, joined.me, link.clone());
                let announcer = Announcer::start("TMCL 联机房间".to_owned(), joined.local_port)
                    .await
                    .inspect_err(|e| tracing::warn!("局域网广播启动失败：{e}"))
                    .ok();
                tracing::info!(?room, mc_port = joined.mc_port, "已加入房间");
                Ok(Self {
                    role: Role::Guest,
                    room,
                    mc_port: joined.mc_port,
                    me,
                    tunnel: Arc::new(tokio::sync::Mutex::new(tunnel)),
                    state_dir: options.state_dir,
                    server: None,
                    guest: Some(GuestState {
                        center: joined.center,
                        local_port: joined.local_port,
                        link,
                        heartbeat,
                        _announcer: announcer,
                    }),
                })
            }
            Err(e) => {
                let _ = tunnel.stop().await;
                remove_pid(&options.state_dir);
                Err(e)
            }
        }
    }

    /// 本机作为房客时，游戏中直接连接的地址。
    pub fn local_address(&self) -> Option<SocketAddr> {
        self.guest
            .as_ref()
            .map(|g| SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, g.local_port)))
    }

    /// 本机广播给游戏的端口，`p2p_lan_worlds` 扫描时排除。
    pub fn announced_port(&self) -> Option<u16> {
        self.guest.as_ref().map(|g| g.local_port)
    }

    /// 当前状态；会调用一次 `easytier-cli peer`。
    pub async fn status(&self) -> SessionStatus {
        let (alive, peers) = {
            let mut tunnel = self.tunnel.lock().await;
            let alive = tunnel.state() == TunnelState::Running;
            let peers = if alive {
                tunnel.peers().await.unwrap_or_default()
            } else {
                Vec::new()
            };
            (alive, peers)
        };
        let nat_type = peers
            .iter()
            .find(|p| p.is_local)
            .map(|p| p.nat_type.clone())
            .unwrap_or_else(|| "Unknown".to_owned());
        let (players, latency_ms, direct, running) = match (&self.server, &self.guest) {
            (Some(server), _) => (server.players(), None, None, alive),
            (_, Some(guest)) => {
                let (players, fresh) = guest
                    .link
                    .lock()
                    .map(|l| (l.players.clone(), l.last_ok.elapsed() < PLAYER_TIMEOUT))
                    .unwrap_or_default();
                let center = center_peer(&peers, guest.center);
                (
                    players,
                    center.and_then(|p| p.latency_ms),
                    center.map(|p| p.direct),
                    alive && fresh,
                )
            }
            _ => (Vec::new(), None, None, alive),
        };
        SessionStatus {
            role: self.role,
            code: self.room.code().to_owned(),
            mc_port: self.mc_port,
            local_address: self.local_address(),
            players: players
                .into_iter()
                .map(|p| {
                    let is_self = p.machine_id == self.me;
                    (p, is_self)
                })
                .collect(),
            latency_ms,
            direct,
            nat_type,
            running,
        }
    }

    /// 结束联机：停止心跳、广播与信令服务，结束 EasyTier。
    pub async fn stop(mut self) -> AppResult<()> {
        if let Some(guest) = self.guest.take() {
            guest.heartbeat.abort();
        }
        self.server.take();
        let result = match Arc::try_unwrap(self.tunnel) {
            Ok(tunnel) => tunnel.into_inner().stop().await,
            // 状态查询仍持有引用时，丢弃引用后进程随 `kill_on_drop` 结束。
            Err(_) => Ok(()),
        };
        remove_pid(&self.state_dir);
        tracing::info!("已结束联机");
        result
    }
}

fn center_peer(peers: &[Peer], center: Ipv4Addr) -> Option<&Peer> {
    peers.iter().find(|p| !p.is_local && p.ipv4 == Some(center))
}

struct Joined {
    client: SignalClient,
    me: PlayerProfile,
    players: Vec<PlayerProfile>,
    center: Ipv4Addr,
    mc_port: u16,
    local_port: u16,
}

/// 房客：找联机中心、建立信令连接、取游戏端口并建立本机转发。
async fn connect(tunnel: &mut Tunnel, mut me: PlayerProfile, ctx: &TaskCtx) -> AppResult<Joined> {
    let started = Instant::now();
    let (center, signal_port, local_id) = loop {
        ctx.check_cancelled()?;
        let peers = tunnel.peers().await.unwrap_or_default();
        if let Some((ip, port)) = find_center(&peers) {
            let local_id = peers.iter().find(|p| p.is_local).map(|p| p.id.clone());
            break (ip, port, local_id);
        }
        if started.elapsed() > CENTER_TIMEOUT {
            return Err(AppError::new(ErrorCode::P2pCenterNotFound));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    };
    tracing::info!(%center, signal_port, elapsed = ?started.elapsed(), "找到联机中心");
    me.kind = PlayerKind::Guest;
    me.easytier_id = local_id.filter(|id| !id.is_empty());

    let signal_local = free_port()?;
    tunnel
        .add_port_forward(&PortForward {
            proto: Proto::Tcp,
            local: localhost(signal_local),
            remote: SocketAddr::V4(SocketAddrV4::new(center, signal_port)),
        })
        .await?;
    let started = Instant::now();
    let mut client = loop {
        ctx.check_cancelled()?;
        if let Ok(mut client) = SignalClient::connect(localhost(signal_local)).await
            && client.ping().await.is_ok()
        {
            break client;
        }
        if started.elapsed() > DIRECT_TIMEOUT {
            return Err(AppError::new(ErrorCode::P2pDirectFailed));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    };

    // 规范的标准流程：先心跳，再协商协议，再取游戏端口。
    client.player_ping(&me).await?;
    client.protocols().await?;
    client.player_ping(&me).await?;
    let mc_port = client.server_port().await?;
    let players = client.players().await.unwrap_or_default();

    let local_port = free_port()?;
    for proto in [Proto::Tcp, Proto::Udp] {
        tunnel
            .add_port_forward(&PortForward {
                proto,
                local: localhost(local_port),
                remote: SocketAddr::V4(SocketAddrV4::new(center, mc_port)),
            })
            .await?;
    }
    Ok(Joined {
        client,
        me,
        players,
        center,
        mc_port,
        local_port,
    })
}

/// 每 5 秒发送心跳并刷新玩家列表；连接断开时重连（同一转发端口）。
fn spawn_heartbeat(
    mut client: SignalClient,
    me: PlayerProfile,
    link: Arc<Mutex<GuestLink>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let addr = client.peer_addr();
        loop {
            tokio::time::sleep(HEARTBEAT_INTERVAL).await;
            let beat = async {
                client.player_ping(&me).await?;
                client.players().await
            };
            match beat.await {
                Ok(players) => {
                    if let Ok(mut link) = link.lock() {
                        link.players = players;
                        link.last_ok = Instant::now();
                    }
                }
                Err(e) => {
                    tracing::debug!("心跳失败：{e}");
                    if let Some(addr) = addr
                        && let Ok(mut fresh) = SignalClient::connect(addr).await
                        && fresh.protocols().await.is_ok()
                    {
                        client = fresh;
                    }
                }
            }
        }
    })
}

fn localhost(port: u16) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
}

/// 向系统要一个本机空闲端口（随后交给 EasyTier 监听）。
fn free_port() -> AppResult<u16> {
    Ok(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()?
        .port())
}

fn write_pid(dir: &Path, tunnel: &Tunnel) {
    let Some(pid) = tunnel.pid() else { return };
    if let Err(e) = std::fs::create_dir_all(dir)
        .and_then(|()| std::fs::write(dir.join(PID_FILE), pid.to_string()))
    {
        tracing::warn!("无法记录 EasyTier 进程号：{e}");
    }
}

fn remove_pid(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(PID_FILE));
}

/// 结束上次 TMCL 被强制结束时留下的 `easytier-core`：读取进程号文件，确认该进程
/// 仍是 `easytier-core` 后才结束它（进程号可能已被别的程序复用）。
pub async fn cleanup_orphan(dir: &Path) {
    let path = dir.join(PID_FILE);
    let Some(pid) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
    else {
        return;
    };
    if process_is_easytier(pid).await {
        tracing::info!(pid, "结束残留的 EasyTier 进程");
        kill(pid).await;
    }
    let _ = std::fs::remove_file(path);
}

#[cfg(windows)]
async fn process_is_easytier(pid: u32) -> bool {
    let mut command = tokio::process::Command::new("tasklist");
    command.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
    crate::tunnel::hide_console(&mut command);
    command
        .output()
        .await
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains("easytier-core"))
}

#[cfg(windows)]
async fn kill(pid: u32) {
    let mut command = tokio::process::Command::new("taskkill");
    command.args(["/PID", &pid.to_string(), "/F"]);
    crate::tunnel::hide_console(&mut command);
    let _ = command.output().await;
}

#[cfg(not(windows))]
async fn process_is_easytier(pid: u32) -> bool {
    tokio::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .await
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains("easytier-core"))
}

#[cfg(not(windows))]
async fn kill(pid: u32) {
    let _ = tokio::process::Command::new("kill")
        .arg(pid.to_string())
        .output()
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cleanup_ignores_foreign_processes() {
        let dir = tempfile::tempdir().unwrap();
        // 一个确实存在但不是 easytier-core 的进程：自己。
        std::fs::write(dir.path().join(PID_FILE), std::process::id().to_string()).unwrap();
        cleanup_orphan(dir.path()).await;
        assert!(!dir.path().join(PID_FILE).exists());
        // 没有文件、内容无效时什么也不做。
        cleanup_orphan(dir.path()).await;
        std::fs::write(dir.path().join(PID_FILE), "garbage").unwrap();
        cleanup_orphan(dir.path()).await;
    }

    #[test]
    fn center_peer_matches_by_ip() {
        let peer = |ip: Option<Ipv4Addr>, local: bool| Peer {
            id: "1".into(),
            hostname: "h".into(),
            ipv4: ip,
            is_local: local,
            direct: true,
            latency_ms: Some(12.0),
            nat_type: "FullCone".into(),
            version: "2.6.4".into(),
        };
        let center = Ipv4Addr::new(10, 144, 144, 1);
        let peers = [
            peer(Some(center), true),
            peer(None, false),
            peer(Some(center), false),
        ];
        assert!(std::ptr::eq(
            center_peer(&peers, center).unwrap(),
            &peers[2]
        ));
        assert!(center_peer(&peers[..2], center).is_none());
    }
}
