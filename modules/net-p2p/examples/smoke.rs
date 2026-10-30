//! 真机冒烟：下载 EasyTier，在本机起一个房主和一个房客，经公共节点互相发现，
//! 再检查房主只暴露游戏端口和信令端口。
//!
//! `cargo run -p tmcl-p2p --example smoke [数据目录]`，数据目录默认为系统临时目录下的
//! `tmcl-p2p-smoke`。需要能访问 GitHub 和 EasyTier 公共节点。

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use tmcl_common::{AppResult, DataPaths, TaskCtx};
use tmcl_net::{Downloader, HttpClient};
use tmcl_p2p::{
    PortForward, Proto, RoomCode, Tunnel, TunnelConfig, effective_nodes, ensure_installed,
    find_center,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> AppResult<()> {
    let root = std::env::args()
        .nth(1)
        .map(Into::into)
        .unwrap_or_else(|| std::env::temp_dir().join("tmcl-p2p-smoke"));
    let paths = DataPaths::at(root);
    let downloader = Downloader::new(HttpClient::new()?, Default::default());
    let started = Instant::now();
    let bins = ensure_installed(&downloader, &paths, &TaskCtx::noop("smoke")).await?;
    println!(
        "EasyTier: {} ({:?})",
        bins.core.display(),
        started.elapsed()
    );

    // 房主上的三个“服务”：游戏端口、信令端口应能访问，额外端口应被白名单挡住。
    let game = echo_server().await;
    let signal = echo_server().await;
    let hidden = echo_server().await;

    let room = RoomCode::generate()?;
    println!("房间码 {}", room.code());
    // TMCL_P2P_NODES：逗号分隔的额外节点，排在默认节点前面。
    let custom: Vec<String> = std::env::var("TMCL_P2P_NODES")
        .unwrap_or_default()
        .split(',')
        .map(str::to_owned)
        .collect();
    let peers = effective_nodes(true, &custom);
    println!("节点：{peers:?}");
    let host = Tunnel::start(
        &bins,
        &TunnelConfig::host(&room, peers.clone(), signal, game),
    )
    .await?;
    let mut guest = Tunnel::start(&bins, &TunnelConfig::guest(&room, peers)).await?;

    let deadline = Instant::now() + Duration::from_secs(90);
    let center = loop {
        let list = guest.peers().await.unwrap_or_default();
        if let Some(center) = find_center(&list) {
            println!("房客看到的节点：");
            for p in &list {
                println!(
                    "  {:<40} {:<15} direct={} lat={:?} nat={}",
                    p.hostname,
                    p.ipv4.map(|ip| ip.to_string()).unwrap_or_default(),
                    p.direct,
                    p.latency_ms,
                    p.nat_type
                );
            }
            break center;
        }
        if Instant::now() > deadline {
            println!(
                "90 秒内没有发现联机中心。房主日志：\n{}",
                host.recent_log().join("\n")
            );
            println!("房客日志：\n{}", guest.recent_log().join("\n"));
            // process::exit 不会运行析构函数，先结束两个 EasyTier 进程。
            guest.stop().await?;
            host.stop().await?;
            std::process::exit(1);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    };
    println!(
        "联机中心 {}:{} ({:?})",
        center.0,
        center.1,
        started.elapsed()
    );
    assert_eq!(center.1, signal);

    let mut ok = true;
    for (name, port, expect) in [
        ("game", game, true),
        ("signal", signal, true),
        ("hidden", hidden, false),
    ] {
        let local = free_port();
        guest
            .add_port_forward(&PortForward {
                proto: Proto::Tcp,
                local: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, local)),
                remote: SocketAddr::V4(SocketAddrV4::new(center.0, port)),
            })
            .await?;
        // 新转发规则和经节点建立的连接需要一点时间生效，最多试三次。
        let mut reachable = false;
        for _ in 0..3 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if echo_ok(local).await {
                reachable = true;
                break;
            }
        }
        println!("{name} 端口 {port} 经转发可达：{reachable}（期望 {expect}）");
        ok &= reachable == expect;
    }

    guest.stop().await?;
    host.stop().await?;
    println!("{}", if ok { "冒烟通过" } else { "冒烟失败" });
    std::process::exit(if ok { 0 } else { 1 });
}

/// 在随机端口上起一个回显服务，返回端口。
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

fn free_port() -> u16 {
    std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
