//! Minecraft 局域网广播：房主侧找出已“对局域网开放”的世界，房客侧把转发端口广播出去，
//! 让房间出现在多人游戏列表的“局域网游戏”中。
//!
//! Minecraft 每 1.5 秒向组播地址 `224.0.2.60:4445` 发送一条
//! `[MOTD]<世界名>[/MOTD][AD]<端口>[/AD]`，客户端在同一地址上监听。

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use serde::Serialize;
use socket2::{Domain, Protocol, Socket, Type};
use tmcl_common::AppResult;
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

use crate::signal::probe;

/// Minecraft 局域网广播的组播地址与端口。
pub const LAN_GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 2, 60);
pub const LAN_PORT: u16 = 4445;
/// 广播间隔，与 Minecraft 相同。
const ANNOUNCE_INTERVAL: Duration = Duration::from_millis(1500);
/// 一条广播的最大长度。
const MAX_DATAGRAM: usize = 1024;

/// 一个对局域网开放的世界。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanWorld {
    /// 世界的 MOTD（通常是“玩家名 - 世界名”）。
    pub motd: String,
    /// 本机上的游戏端口。
    pub port: u16,
}

/// 解析一条局域网广播；格式不对时返回 `None`。
pub fn parse_announcement(data: &[u8]) -> Option<LanWorld> {
    let text = std::str::from_utf8(data).ok()?;
    let between = |open: &str, close: &str| {
        let start = text.find(open)? + open.len();
        let len = text[start..].find(close)?;
        Some(&text[start..start + len])
    };
    let port: u16 = between("[AD]", "[/AD]")?.trim().parse().ok()?;
    (port != 0).then(|| LanWorld {
        motd: between("[MOTD]", "[/MOTD]").unwrap_or_default().to_owned(),
        port,
    })
}

/// 生成一条局域网广播。
pub fn announcement(motd: &str, port: u16) -> String {
    format!("[MOTD]{motd}[/MOTD][AD]{port}[/AD]")
}

/// 在组播地址上监听 `duration`，返回本机上对局域网开放的世界：按端口去重、排序，
/// 跳过 `exclude` 中的端口，并且只保留 `127.0.0.1` 上能连上的端口（即游戏在本机）。
pub async fn scan(duration: Duration, exclude: &[u16]) -> AppResult<Vec<LanWorld>> {
    let socket = listen_socket()?;
    let mut found: Vec<LanWorld> = Vec::new();
    let mut buf = [0u8; MAX_DATAGRAM];
    let deadline = tokio::time::Instant::now() + duration;
    while let Ok(received) = tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await {
        let Ok((len, _)) = received else { continue };
        if let Some(world) = parse_announcement(&buf[..len])
            && !exclude.contains(&world.port)
            && !found.iter().any(|w| w.port == world.port)
        {
            found.push(world);
        }
    }
    let mut local = Vec::with_capacity(found.len());
    for world in found {
        if probe(world.port).await {
            local.push(world);
        }
    }
    local.sort_by_key(|w| w.port);
    Ok(local)
}

/// 与 Minecraft 共用 4445 端口的监听套接字（`SO_REUSEADDR`），加入组播组。
fn listen_socket() -> AppResult<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.bind(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, LAN_PORT)).into())?;
    socket.join_multicast_v4(&LAN_GROUP, &Ipv4Addr::UNSPECIFIED)?;
    socket.set_nonblocking(true)?;
    Ok(UdpSocket::from_std(socket.into())?)
}

/// 周期性发送局域网广播的后台任务，被丢弃时停止。
pub struct Announcer {
    task: JoinHandle<()>,
}

impl Announcer {
    /// 每 1.5 秒在本机广播一次 `motd` 与 `port`。
    pub async fn start(motd: String, port: u16) -> AppResult<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
        socket.set_multicast_loop_v4(true)?;
        socket.set_multicast_ttl_v4(1)?;
        let message = announcement(&motd, port);
        let task = tokio::spawn(async move {
            let target = (LAN_GROUP, LAN_PORT);
            loop {
                if let Err(e) = socket.send_to(message.as_bytes(), target).await {
                    tracing::debug!(error = %e, "局域网广播发送失败");
                }
                tokio::time::sleep(ANNOUNCE_INTERVAL).await;
            }
        });
        Ok(Self { task })
    }
}

impl Drop for Announcer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minecraft_announcements() {
        assert_eq!(
            parse_announcement(b"[MOTD]Steve - New World[/MOTD][AD]51234[/AD]"),
            Some(LanWorld {
                motd: "Steve - New World".to_owned(),
                port: 51234
            })
        );
        assert_eq!(
            parse_announcement(b"[AD]25565[/AD]").map(|w| w.port),
            Some(25565)
        );
        for bad in [
            &b"[MOTD]x[/MOTD]"[..],
            b"[MOTD]x[/MOTD][AD]0[/AD]",
            b"[MOTD]x[/MOTD][AD]70000[/AD]",
            b"[MOTD]x[/MOTD][AD]abc[/AD]",
            b"\xff\xfe",
        ] {
            assert_eq!(parse_announcement(bad), None);
        }
    }

    #[test]
    fn announcement_round_trips() {
        let text = announcement("TMCL 联机房间", 40000);
        let world = parse_announcement(text.as_bytes()).unwrap();
        assert_eq!(world.motd, "TMCL 联机房间");
        assert_eq!(world.port, 40000);
    }
}
