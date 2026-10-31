//! 真机冒烟（M2-12b）：在本机用 [`Session`] 开一个房间、再以房客身份加入，走完整的
//! Scaffolding 流程，并做安全检查。
//!
//! `cargo run -p tmcl-p2p --example room`，数据目录取环境变量 `TMCL_P2P_ROOT`，默认为系统临时目录下的
//! `tmcl-p2p-room`。需要能访问 GitHub 和 EasyTier 公共节点。
//!
//! - 房主上有一个假的“游戏端口”（回显服务，像 Minecraft 一样监听 `0.0.0.0`），
//!   和一个不应暴露的“其他端口”。
//! - 房客加入后，经 `local_address` 应能连上游戏端口，玩家列表应有两人；
//! - 房客手动转发到房主的“其他端口”，应连不上（白名单只放行游戏端口与信令端口）。
//!
//! 只想加入别人（HMCL / PCL-CE）的房间时：`cargo run -p tmcl-p2p --example room -- --join <房间码>`；
//! 只想开房给别人加入时：`--host <游戏端口>`，按回车结束。

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tmcl_common::{AppResult, DataPaths, TaskCtx};
use tmcl_net::{Downloader, HttpClient};
use tmcl_p2p::{
    PlayerKind, PlayerProfile, RoomCode, Session, SessionOptions, effective_nodes,
    ensure_installed, signal,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    // 数据目录用环境变量 TMCL_P2P_ROOT 指定。
    let root: PathBuf = std::env::var_os("TMCL_P2P_ROOT")
        .map(Into::into)
        .unwrap_or_else(|| std::env::temp_dir().join("tmcl-p2p-room"));
    let paths = DataPaths::at(root);
    let downloader = Downloader::new(HttpClient::new()?, Default::default());
    let bins = ensure_installed(&downloader, &paths, &TaskCtx::noop("room")).await?;
    let ctx = TaskCtx::noop("room");
    let options = |name: &str, dir: &str| -> AppResult<SessionOptions> {
        let state_dir = paths.p2p_dir().join(dir);
        Ok(SessionOptions {
            player: PlayerProfile {
                name: name.to_owned(),
                machine_id: signal::machine_id(&state_dir)?,
                easytier_id: None,
                vendor: "TMCL smoke".to_owned(),
                kind: PlayerKind::Guest,
            },
            peers: effective_nodes(true, &[]),
            state_dir,
        })
    };

    if let Some(code) = flag("--join") {
        let started = Instant::now();
        let guest = Session::join(
            &bins,
            RoomCode::parse(&code)?,
            options("TMCLGuest", "guest")?,
            &ctx,
        )
        .await?;
        println!("已加入（{:?}）", started.elapsed());
        wait_enter(&guest).await;
        return guest.stop().await;
    }
    if let Some(port) = flag("--host") {
        let port: u16 = port.parse().expect("--host <端口>");
        let host = Session::host(&bins, port, options("TMCLHost", "host")?, &ctx).await?;
        println!("房间码 {}", host.status().await.code);
        wait_enter(&host).await;
        return host.stop().await;
    }

    let game = echo_server().await;
    let hidden = echo_server().await;
    let host = Session::host(&bins, game, options("TMCLHost", "host")?, &ctx).await?;
    let code = host.status().await.code;
    println!("房间码 {code}，游戏端口 {game}，其他端口 {hidden}");

    let started = Instant::now();
    let guest = match Session::join(
        &bins,
        RoomCode::parse(&code)?,
        options("TMCLGuest", "guest")?,
        &ctx,
    )
    .await
    {
        Ok(guest) => guest,
        Err(e) => {
            println!("加入失败：{e}");
            host.stop().await?;
            std::process::exit(1);
        }
    };
    println!("已加入（{:?}）", started.elapsed());

    let mut ok = true;
    let status = guest.status().await;
    let local = status.local_address.expect("房客应有本机地址");
    let reachable = echo_ok(local.port()).await;
    println!("经 {local} 连游戏端口：{reachable}（期望 true）");
    ok &= reachable;

    tokio::time::sleep(Duration::from_secs(6)).await;
    let names: Vec<String> = guest
        .status()
        .await
        .players
        .iter()
        .map(|(p, me)| format!("{}{}", p.name, if *me { "(本机)" } else { "" }))
        .collect();
    let host_names: Vec<String> = host
        .status()
        .await
        .players
        .iter()
        .map(|(p, _)| p.name.clone())
        .collect();
    println!("房客看到的玩家：{names:?}；房主看到的玩家：{host_names:?}");
    ok &= names.len() == 2 && host_names.len() == 2;
    let status = guest.status().await;
    println!(
        "延迟 {:?} ms，直连 {:?}，NAT {}，running {}",
        status.latency_ms, status.direct, status.nat_type, status.running
    );
    ok &= status.running;

    // 安全检查：用同一网络的另一个房客隧道转发到房主的其他端口，应连不上。
    let exposed = hidden_reachable(&bins, &code, hidden).await?;
    println!("其他端口 {hidden} 经转发可达：{exposed}（期望 false）");
    ok &= !exposed;

    guest.stop().await?;
    host.stop().await?;
    println!("{}", if ok { "冒烟通过" } else { "冒烟失败" });
    std::process::exit(if ok { 0 } else { 1 });
}

/// 以一个额外的房客加入同一网络，把本机端口转发到房主虚拟 IP 的 `port`，看能否连通。
async fn hidden_reachable(
    bins: &tmcl_p2p::EasyTierBinaries,
    code: &str,
    port: u16,
) -> AppResult<bool> {
    use std::net::{SocketAddr, SocketAddrV4};
    use tmcl_p2p::{PortForward, Proto, Tunnel, TunnelConfig, find_center};
    let room = RoomCode::parse(code)?;
    let mut probe = Tunnel::start(
        bins,
        &TunnelConfig::guest(&room, effective_nodes(true, &[])),
    )
    .await?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let center = loop {
        if let Some(center) = find_center(&probe.peers().await.unwrap_or_default()) {
            break center.0;
        }
        if Instant::now() > deadline {
            probe.stop().await?;
            return Ok(false);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    };
    let local = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()?
        .port();
    probe
        .add_port_forward(&PortForward {
            proto: Proto::Tcp,
            local: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, local)),
            remote: SocketAddr::V4(SocketAddrV4::new(center, port)),
        })
        .await?;
    let mut reachable = false;
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if echo_ok(local).await {
            reachable = true;
            break;
        }
    }
    probe.stop().await?;
    Ok(reachable)
}

async fn wait_enter(session: &Session) {
    println!("按回车结束；每 5 秒打印一次状态。");
    let mut line = String::new();
    let stdin = tokio::task::spawn_blocking(move || std::io::stdin().read_line(&mut line));
    tokio::pin!(stdin);
    loop {
        tokio::select! {
            _ = &mut stdin => return,
            () = tokio::time::sleep(Duration::from_secs(5)) => {
                let s = session.status().await;
                let players: Vec<String> = s.players.iter().map(|(p, _)| format!("{} [{}]", p.name, p.vendor)).collect();
                println!(
                    "running={} 本机地址={:?} 游戏端口={} 延迟={:?} 直连={:?} NAT={} 玩家={players:?}",
                    s.running, s.local_address, s.mc_port, s.latency_ms, s.direct, s.nat_type
                );
            }
        }
    }
}

/// 在随机端口上起一个回显服务（监听 `0.0.0.0`，与 Minecraft 相同），返回端口。
async fn echo_server() -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 64];
                if let Ok(n) = stream.read(&mut buf).await {
                    let _ = stream.write_all(&buf[..n]).await;
                }
            });
        }
    });
    port
}

async fn echo_ok(port: u16) -> bool {
    let attempt = async {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await?;
        stream.write_all(b"tmcl").await?;
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).await?;
        std::io::Result::Ok(&buf == b"tmcl")
    };
    matches!(
        tokio::time::timeout(Duration::from_secs(8), attempt).await,
        Ok(Ok(true))
    )
}
