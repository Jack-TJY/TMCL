//! Scaffolding 联机信息获取协议：联机中心（房主）的 TCP 服务与联机房客的客户端。
//!
//! 规范见 <https://github.com/Scaffolding-MC/Scaffolding-MC>（MIT）。请求为
//! `u8 类型长度 | 类型 | u32 体长度 | 体`，响应为 `u8 状态 | u32 体长度 | 体`，整数均为大端序。
//! 实现全部基础协议（`c:ping`、`c:protocols`、`c:server_port`、`c:player_ping`、
//! `c:player_profiles_list`）以及协商标识 `c:player_easytier_id`。
//!
//! 信令服务只监听 `127.0.0.1`；房客经 EasyTier 访问房主虚拟 IP 上的端口时，
//! `--no-tun` 模式下由 EasyTier 在房主本机连到这个回环端口。

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

/// 本实现支持的协议，`c:protocols` 协商时发送。
pub const PROTOCOLS: [&str; 6] = [
    "c:ping",
    "c:protocols",
    "c:server_port",
    "c:player_ping",
    "c:player_profiles_list",
    "c:player_easytier_id",
];
/// 房客心跳间隔（规范要求每 5 秒一次）。
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// 超过这么久没有心跳的房客不再出现在玩家列表中。
pub const PLAYER_TIMEOUT: Duration = Duration::from_secs(15);

const STATUS_OK: u8 = 0;
/// `c:server_port`：服务器未启动。
const STATUS_SERVER_NOT_RUNNING: u8 = 32;
const STATUS_UNKNOWN: u8 = 255;
/// 请求体与响应体的上限；玩家列表远小于此。
const MAX_BODY: u32 = 256 * 1024;
/// `c:ping` 请求体须小于 32 字节。
const MAX_PING: usize = 32;
/// 名字、设备 ID 等字符串字段的长度上限。
const MAX_FIELD: usize = 128;
/// 房主同时保留的房客数上限，防止伪造心跳撑爆内存。
const MAX_GUESTS: usize = 64;
/// 单次请求的超时；房客连接空闲超过 [`IDLE_TIMEOUT`] 时房主断开。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// 检查房主游戏端口是否开放时的连接超时。
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// 玩家身份。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PlayerKind {
    Host,
    #[default]
    Guest,
}

/// 一个玩家，即 `c:player_ping` 与 `c:player_profiles_list` 中的 JSON 对象。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerProfile {
    pub name: String,
    /// 设备的稳定标识，联机中心以它区分玩家。
    pub machine_id: String,
    /// EasyTier 节点 ID；双方都支持 `c:player_easytier_id` 时才发送。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub easytier_id: Option<String>,
    /// 启动器名称与版本，如 `TMCL 0.1.0`。
    #[serde(default)]
    pub vendor: String,
    /// 心跳请求里没有这个字段。
    #[serde(default)]
    pub kind: PlayerKind,
}

impl PlayerProfile {
    fn is_valid(&self) -> bool {
        let ok = |s: &str| !s.trim().is_empty() && s.len() <= MAX_FIELD;
        ok(&self.name)
            && ok(&self.machine_id)
            && self.vendor.len() <= MAX_FIELD
            && self
                .easytier_id
                .as_ref()
                .is_none_or(|id| id.len() <= MAX_FIELD)
    }
}

/// 本机的设备标识：首次使用时随机生成并保存在 `dir/machine_id`，之后保持不变。
pub fn machine_id(dir: &Path) -> AppResult<String> {
    let path = dir.join("machine_id");
    if let Ok(id) = std::fs::read_to_string(&path) {
        let id = id.trim();
        if id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(id.to_owned());
        }
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| {
        AppError::new(ErrorCode::CommonInternal).with_detail(format!("getrandom: {e}"))
    })?;
    let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::create_dir_all(dir)?;
    std::fs::write(&path, &id)?;
    Ok(id)
}

// ---- 帧格式 ----

async fn read_body<R: AsyncRead + Unpin>(r: &mut R) -> std::io::Result<Vec<u8>> {
    let len = r.read_u32().await?;
    if len > MAX_BODY {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("body too large: {len}"),
        ));
    }
    let mut body = vec![0; len as usize];
    r.read_exact(&mut body).await?;
    Ok(body)
}

/// 读一个请求；对方正常关闭连接时返回 `None`。
async fn read_request<R: AsyncRead + Unpin>(
    r: &mut R,
) -> std::io::Result<Option<(String, Vec<u8>)>> {
    let mut len = [0u8; 1];
    if r.read(&mut len).await? == 0 {
        return Ok(None);
    }
    let mut kind = vec![0; len[0] as usize];
    r.read_exact(&mut kind).await?;
    let kind = String::from_utf8(kind)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "request type"))?;
    let body = read_body(r).await?;
    Ok(Some((kind, body)))
}

async fn write_frame<W: AsyncWrite + Unpin>(
    w: &mut W,
    head: &[u8],
    body: &[u8],
) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(head.len() + 4 + body.len());
    frame.extend_from_slice(head);
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(body);
    w.write_all(&frame).await?;
    w.flush().await
}

fn join_protocols(list: &[&str]) -> Vec<u8> {
    list.join("\0").into_bytes()
}

fn split_protocols(body: &[u8]) -> Vec<String> {
    body.split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

// ---- 联机中心 ----

struct Room {
    host: PlayerProfile,
    mc_port: u16,
    guests: HashMap<String, (PlayerProfile, Instant)>,
}

impl Room {
    fn players(&self, now: Instant) -> Vec<PlayerProfile> {
        let mut guests: Vec<PlayerProfile> = self
            .guests
            .values()
            .filter(|(_, seen)| now.duration_since(*seen) < PLAYER_TIMEOUT)
            .map(|(p, _)| p.clone())
            .collect();
        guests.sort_by(|a, b| a.name.cmp(&b.name).then(a.machine_id.cmp(&b.machine_id)));
        let mut players = vec![self.host.clone()];
        players.extend(guests);
        players
    }

    fn heartbeat(&mut self, mut player: PlayerProfile, now: Instant) -> bool {
        if !player.is_valid() || player.machine_id == self.host.machine_id {
            return false;
        }
        self.guests
            .retain(|_, (_, seen)| now.duration_since(*seen) < PLAYER_TIMEOUT);
        if !self.guests.contains_key(&player.machine_id) && self.guests.len() >= MAX_GUESTS {
            return false;
        }
        player.kind = PlayerKind::Guest;
        self.guests.insert(player.machine_id.clone(), (player, now));
        true
    }
}

/// 联机中心的 Scaffolding 服务，监听 `127.0.0.1` 上的一个空闲端口。被丢弃时停止。
pub struct SignalServer {
    port: u16,
    room: Arc<Mutex<Room>>,
    task: JoinHandle<()>,
}

impl SignalServer {
    /// 在 `127.0.0.1` 的空闲端口（大于 1024）上启动服务。`host` 为房主自己，
    /// `mc_port` 为房主本机上 Minecraft 对局域网开放的端口。
    pub async fn start(mut host: PlayerProfile, mc_port: u16) -> AppResult<Self> {
        host.kind = PlayerKind::Host;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let room = Arc::new(Mutex::new(Room {
            host,
            mc_port,
            guests: HashMap::new(),
        }));
        let shared = room.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, peer)) = listener.accept().await else {
                    continue;
                };
                let room = shared.clone();
                tokio::spawn(async move {
                    if let Err(e) = serve(stream, room).await {
                        tracing::debug!(%peer, error = %e, "Scaffolding 连接结束");
                    }
                });
            }
        });
        tracing::info!(port, mc_port, "Scaffolding 信令服务已启动");
        Ok(Self { port, room, task })
    }

    /// 信令端口，用于主机名 `scaffolding-mc-server-<端口>`。
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 当前玩家：房主在前，其余为 [`PLAYER_TIMEOUT`] 内有心跳的房客，按名字排序。
    pub fn players(&self) -> Vec<PlayerProfile> {
        self.room
            .lock()
            .map(|room| room.players(Instant::now()))
            .unwrap_or_default()
    }
}

impl Drop for SignalServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(mut stream: TcpStream, room: Arc<Mutex<Room>>) -> std::io::Result<()> {
    let mut easytier_id = false;
    loop {
        let Some((kind, body)) = tokio::time::timeout(IDLE_TIMEOUT, read_request(&mut stream))
            .await
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??
        else {
            return Ok(());
        };
        let (status, body) = handle(&kind, &body, &room, &mut easytier_id).await;
        write_frame(&mut stream, &[status], &body).await?;
    }
}

async fn handle(
    kind: &str,
    body: &[u8],
    room: &Mutex<Room>,
    easytier_id: &mut bool,
) -> (u8, Vec<u8>) {
    let error = |msg: &str| (STATUS_UNKNOWN, msg.as_bytes().to_vec());
    match kind {
        "c:ping" if body.len() < MAX_PING => (STATUS_OK, body.to_vec()),
        "c:ping" => error("ping body too long"),
        "c:protocols" => {
            *easytier_id = split_protocols(body)
                .iter()
                .any(|p| p == "c:player_easytier_id");
            (STATUS_OK, join_protocols(&PROTOCOLS))
        }
        "c:server_port" => {
            let Ok(port) = room.lock().map(|r| r.mc_port) else {
                return error("internal");
            };
            if probe(port).await {
                (STATUS_OK, port.to_be_bytes().to_vec())
            } else {
                (STATUS_SERVER_NOT_RUNNING, Vec::new())
            }
        }
        "c:player_ping" => {
            let Ok(player) = serde_json::from_slice::<PlayerProfile>(body) else {
                return error("invalid player");
            };
            let accepted = room
                .lock()
                .is_ok_and(|mut room| room.heartbeat(player, Instant::now()));
            if accepted {
                (STATUS_OK, Vec::new())
            } else {
                error("invalid player")
            }
        }
        "c:player_profiles_list" => {
            let Ok(mut players) = room.lock().map(|r| r.players(Instant::now())) else {
                return error("internal");
            };
            if !*easytier_id {
                players.iter_mut().for_each(|p| p.easytier_id = None);
            }
            match serde_json::to_vec(&players) {
                Ok(json) => (STATUS_OK, json),
                Err(_) => error("internal"),
            }
        }
        _ => error("unsupported request"),
    }
}

/// 本机 `port` 上是否有服务在监听（Minecraft 已对局域网开放）。
pub async fn probe(port: u16) -> bool {
    let connect = TcpStream::connect((Ipv4Addr::LOCALHOST, port));
    matches!(
        tokio::time::timeout(PROBE_TIMEOUT, connect).await,
        Ok(Ok(_))
    )
}

// ---- 联机房客 ----

/// 连到联机中心的一条 Scaffolding 连接。
pub struct SignalClient {
    stream: TcpStream,
    /// 协商后双方都支持 `c:player_easytier_id`。
    easytier_id: bool,
}

impl SignalClient {
    /// 连接联机中心的信令服务（房客经 EasyTier 端口转发后的本机地址）。
    pub async fn connect(addr: SocketAddr) -> AppResult<Self> {
        let stream = tokio::time::timeout(REQUEST_TIMEOUT, TcpStream::connect(addr))
            .await
            .map_err(|_| AppError::new(ErrorCode::NetTimeout))??;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            easytier_id: false,
        })
    }

    /// 联机中心的地址（断线重连用）。
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.stream.peer_addr().ok()
    }

    /// 发送一个请求，返回状态与响应体。
    pub async fn request(&mut self, kind: &str, body: &[u8]) -> AppResult<(u8, Vec<u8>)> {
        let len = u8::try_from(kind.len())
            .map_err(|_| AppError::new(ErrorCode::CommonInternal).with_detail("request type"))?;
        let mut head = vec![len];
        head.extend_from_slice(kind.as_bytes());
        let exchange = async {
            write_frame(&mut self.stream, &head, body).await?;
            let status = self.stream.read_u8().await?;
            let body = read_body(&mut self.stream).await?;
            std::io::Result::Ok((status, body))
        };
        let (status, body) = tokio::time::timeout(REQUEST_TIMEOUT, exchange)
            .await
            .map_err(|_| AppError::new(ErrorCode::NetTimeout).with_detail(kind))??;
        Ok((status, body))
    }

    async fn expect_ok(&mut self, kind: &str, body: &[u8]) -> AppResult<Vec<u8>> {
        match self.request(kind, body).await? {
            (STATUS_OK, body) => Ok(body),
            (status, body) => Err(protocol_error(format!(
                "{kind}: status {status}: {}",
                String::from_utf8_lossy(&body)
            ))),
        }
    }

    /// `c:ping`：确认联机中心在运行。
    pub async fn ping(&mut self) -> AppResult<()> {
        let payload = b"tmcl";
        if self.expect_ok("c:ping", payload).await? != payload {
            return Err(protocol_error("c:ping: echo mismatch"));
        }
        Ok(())
    }

    /// `c:protocols`：协商协议，返回联机中心支持的协议列表。
    pub async fn protocols(&mut self) -> AppResult<Vec<String>> {
        let theirs = split_protocols(
            &self
                .expect_ok("c:protocols", &join_protocols(&PROTOCOLS))
                .await?,
        );
        self.easytier_id = theirs.iter().any(|p| p == "c:player_easytier_id");
        Ok(theirs)
    }

    /// `c:server_port`：房主上 Minecraft 的端口；房主还没开放世界时返回 `P2P_SERVER_NOT_RUNNING`。
    pub async fn server_port(&mut self) -> AppResult<u16> {
        match self.request("c:server_port", &[]).await? {
            (STATUS_OK, body) => match <[u8; 2]>::try_from(body.as_slice()) {
                Ok(bytes) if u16::from_be_bytes(bytes) != 0 => Ok(u16::from_be_bytes(bytes)),
                _ => Err(protocol_error("c:server_port: bad body")),
            },
            (STATUS_SERVER_NOT_RUNNING, _) => Err(AppError::new(ErrorCode::P2pServerNotRunning)),
            (status, body) => Err(protocol_error(format!(
                "c:server_port: status {status}: {}",
                String::from_utf8_lossy(&body)
            ))),
        }
    }

    /// `c:player_ping` 心跳；未协商 `c:player_easytier_id` 时不发送节点 ID。
    pub async fn player_ping(&mut self, me: &PlayerProfile) -> AppResult<()> {
        let mut me = me.clone();
        if !self.easytier_id {
            me.easytier_id = None;
        }
        let mut body = serde_json::json!({
            "name": me.name,
            "machine_id": me.machine_id,
            "vendor": me.vendor,
        });
        if let (Some(id), Some(map)) = (me.easytier_id, body.as_object_mut()) {
            map.insert("easytier_id".to_owned(), id.into());
        }
        self.expect_ok("c:player_ping", &serde_json::to_vec(&body)?)
            .await?;
        Ok(())
    }

    /// `c:player_profiles_list`：房间里的全部玩家。
    pub async fn players(&mut self) -> AppResult<Vec<PlayerProfile>> {
        let body = self.expect_ok("c:player_profiles_list", &[]).await?;
        serde_json::from_slice(&body)
            .map_err(|e| protocol_error(format!("c:player_profiles_list: {e}")))
    }
}

fn protocol_error(detail: impl AsRef<str>) -> AppError {
    AppError::new(ErrorCode::P2pProtocolError).with_detail(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, id: &str) -> PlayerProfile {
        PlayerProfile {
            name: name.to_owned(),
            machine_id: id.to_owned(),
            easytier_id: Some(format!("et-{id}")),
            vendor: "TMCL test".to_owned(),
            kind: PlayerKind::Guest,
        }
    }

    async fn server_with_mc() -> (SignalServer, TcpListener) {
        let mc = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = mc.local_addr().unwrap().port();
        let server = SignalServer::start(profile("Host", "h"), port)
            .await
            .unwrap();
        (server, mc)
    }

    async fn client(server: &SignalServer) -> SignalClient {
        SignalClient::connect((Ipv4Addr::LOCALHOST, server.port()).into())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn standard_flow_round_trip() {
        let (server, mc) = server_with_mc().await;
        assert!(server.port() > 1024);
        let mut c = client(&server).await;
        let me = profile("Alex", "a");
        c.player_ping(&me).await.unwrap();
        c.ping().await.unwrap();
        let theirs = c.protocols().await.unwrap();
        assert_eq!(theirs, PROTOCOLS);
        assert_eq!(
            c.server_port().await.unwrap(),
            mc.local_addr().unwrap().port()
        );
        c.player_ping(&me).await.unwrap();
        let players = c.players().await.unwrap();
        assert_eq!(players.len(), 2);
        assert_eq!(players[0].kind, PlayerKind::Host);
        assert_eq!(players[0].name, "Host");
        assert_eq!(players[1].kind, PlayerKind::Guest);
        assert_eq!(players[1].easytier_id.as_deref(), Some("et-a"));
        assert_eq!(server.players(), players);
    }

    #[tokio::test]
    async fn server_not_running_is_status_32() {
        let (server, mc) = server_with_mc().await;
        drop(mc);
        let mut c = client(&server).await;
        let err = c.server_port().await.unwrap_err();
        assert_eq!(err.code, ErrorCode::P2pServerNotRunning);
    }

    #[tokio::test]
    async fn easytier_id_only_after_negotiation() {
        let (server, _mc) = server_with_mc().await;
        let mut c = client(&server).await;
        // 不协商：心跳和列表里都没有 easytier_id。
        c.player_ping(&profile("Alex", "a")).await.unwrap();
        let (status, body) = c.request("c:player_profiles_list", &[]).await.unwrap();
        assert_eq!(status, 0);
        let text = String::from_utf8(body).unwrap();
        assert!(!text.contains("easytier_id"), "{text}");
        assert!(text.contains(r#""kind":"HOST""#), "{text}");
    }

    #[tokio::test]
    async fn rejects_bad_requests_without_dropping_connection() {
        let (server, _mc) = server_with_mc().await;
        let mut c = client(&server).await;
        for (kind, body) in [
            ("c:unknown", &b""[..]),
            ("c:ping", &[0u8; 40][..]),
            ("c:player_ping", &b"not json"[..]),
            (
                "c:player_ping",
                &br#"{"name":"","machine_id":"x","vendor":""}"#[..],
            ),
            (
                "c:player_ping",
                &br#"{"name":"Fake","machine_id":"h","vendor":""}"#[..],
            ),
        ] {
            let (status, _) = c.request(kind, body).await.unwrap();
            assert_eq!(status, 255, "{kind}");
        }
        c.ping().await.unwrap();
        assert_eq!(server.players().len(), 1);
    }

    #[tokio::test]
    async fn oversized_body_closes_connection() {
        let (server, _mc) = server_with_mc().await;
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port()))
            .await
            .unwrap();
        let mut frame = vec![6u8];
        frame.extend_from_slice(b"c:ping");
        frame.extend_from_slice(&u32::MAX.to_be_bytes());
        stream.write_all(&frame).await.unwrap();
        let mut buf = [0u8; 1];
        assert_eq!(stream.read(&mut buf).await.unwrap_or(0), 0);
    }

    #[test]
    fn guests_expire_and_are_capped() {
        let now = Instant::now();
        let mut room = Room {
            host: PlayerProfile {
                kind: PlayerKind::Host,
                ..profile("Host", "h")
            },
            mc_port: 25565,
            guests: HashMap::new(),
        };
        assert!(room.heartbeat(profile("B", "b"), now));
        assert!(room.heartbeat(profile("A", "a"), now));
        let names: Vec<String> = room.players(now).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Host", "A", "B"]);
        assert_eq!(room.players(now + PLAYER_TIMEOUT).len(), 1);
        for i in 0..MAX_GUESTS * 2 {
            room.heartbeat(profile("X", &format!("x{i}")), now);
        }
        assert_eq!(room.guests.len(), MAX_GUESTS);
    }

    #[test]
    fn machine_id_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let a = machine_id(dir.path()).unwrap();
        assert_eq!(a.len(), 32);
        assert_eq!(machine_id(dir.path()).unwrap(), a);
    }
}
