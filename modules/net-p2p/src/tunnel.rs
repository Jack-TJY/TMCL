//! EasyTier 子进程：启动、停止、状态、端口转发。
//!
//! 两侧都用 `--no-tun`，不创建虚拟网卡、不需要管理员权限。房主用
//! `--tcp-whitelist` / `--udp-whitelist` 只放行 Minecraft 端口和 Scaffolding 信令端口，
//! 房客的白名单为 `0`，不接受任何入站连接（PLAN.md M2-12 安全验收项）。
//! 网络密钥经环境变量 `ET_NETWORK_SECRET` 传入，不出现在命令行上。

use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tmcl_common::{AppError, AppResult, ErrorCode};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};

use crate::easytier::EasyTierBinaries;
use crate::punch::{Peer, parse_peers};
use crate::room::{RoomCode, center_hostname};

/// 房主在 EasyTier 网络中的固定虚拟 IP（与陶瓦联机一致），房客由 DHCP 分配。
pub const HOST_IPV4: Ipv4Addr = Ipv4Addr::new(10, 144, 144, 1);
/// 保留的最近日志行数。
const LOG_LINES: usize = 200;
/// 启动后等待多久检查进程是否立即退出。
const STARTUP_GRACE: Duration = Duration::from_millis(800);
/// 单次 `easytier-cli` 调用的超时。
const CLI_TIMEOUT: Duration = Duration::from_secs(10);

/// 端口转发的协议。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proto {
    Tcp,
    Udp,
}

impl Proto {
    fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

/// 把本机 `local` 上的连接经 EasyTier 转发到虚拟网络中的 `remote`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortForward {
    pub proto: Proto,
    pub local: SocketAddr,
    pub remote: SocketAddr,
}

/// 启动一个 EasyTier 实例所需的参数。
#[derive(Clone, PartialEq, Eq)]
pub struct TunnelConfig {
    /// EasyTier 网络名。
    pub network_name: String,
    /// EasyTier 网络密钥。
    pub network_secret: String,
    /// 公共节点（`-p`）。
    pub peers: Vec<String>,
    /// 主机名；房主为 `scaffolding-mc-server-<端口>`。
    pub hostname: Option<String>,
    /// 固定虚拟 IP；`None` 时用 DHCP。
    pub ipv4: Option<Ipv4Addr>,
    /// 允许入站的 TCP 端口；`[0]` 表示全部拒绝。
    pub tcp_whitelist: Vec<u16>,
    /// 允许入站的 UDP 端口；`[0]` 表示全部拒绝。
    pub udp_whitelist: Vec<u16>,
}

impl TunnelConfig {
    /// 房主：固定虚拟 IP，主机名带信令端口，只放行信令端口与 Minecraft 端口。
    pub fn host(room: &RoomCode, peers: Vec<String>, signal_port: u16, mc_port: u16) -> Self {
        Self {
            network_name: room.network_name().to_owned(),
            network_secret: room.network_secret().to_owned(),
            peers,
            hostname: Some(center_hostname(signal_port)),
            ipv4: Some(HOST_IPV4),
            tcp_whitelist: vec![signal_port, mc_port],
            udp_whitelist: vec![mc_port],
        }
    }

    /// 房客：DHCP 分配虚拟 IP，不接受任何入站连接。
    pub fn guest(room: &RoomCode, peers: Vec<String>) -> Self {
        Self {
            network_name: room.network_name().to_owned(),
            network_secret: room.network_secret().to_owned(),
            peers,
            hostname: None,
            ipv4: None,
            tcp_whitelist: vec![0],
            udp_whitelist: vec![0],
        }
    }

    /// `easytier-core` 的命令行参数（不含网络密钥）。
    pub fn args(&self, rpc_port: u16) -> Vec<String> {
        let mut args: Vec<String> = [
            "--no-tun",
            "--p2p-only",
            "--compression=zstd",
            "--multi-thread",
            "--latency-first",
            "--enable-kcp-proxy",
            "-l",
            "tcp://0.0.0.0:0",
            "-l",
            "udp://0.0.0.0:0",
        ]
        .map(String::from)
        .into();
        args.extend(["--network-name".to_owned(), self.network_name.clone()]);
        args.extend(["-r".to_owned(), format!("127.0.0.1:{rpc_port}")]);
        for peer in &self.peers {
            args.extend(["-p".to_owned(), peer.clone()]);
        }
        if let Some(hostname) = &self.hostname {
            args.extend(["--hostname".to_owned(), hostname.clone()]);
        }
        match self.ipv4 {
            Some(ip) => args.extend(["--ipv4".to_owned(), ip.to_string()]),
            None => args.push("--dhcp".to_owned()),
        }
        args.extend(
            self.tcp_whitelist
                .iter()
                .map(|p| format!("--tcp-whitelist={p}")),
        );
        args.extend(
            self.udp_whitelist
                .iter()
                .map(|p| format!("--udp-whitelist={p}")),
        );
        args
    }
}

/// 子进程状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelState {
    /// 正在运行。
    Running,
    /// 已退出；`code` 为退出码（被信号终止时为 `None`）。
    Exited { code: Option<i32> },
}

/// 一个运行中的 `easytier-core`。被丢弃时进程随之结束。
pub struct Tunnel {
    child: Child,
    cli: PathBuf,
    rpc_port: u16,
    log: Arc<Mutex<VecDeque<String>>>,
}

impl Tunnel {
    /// 启动 `easytier-core`；进程在启动后很快退出时返回 `P2P_TUNNEL_EXITED`。
    pub async fn start(binaries: &EasyTierBinaries, config: &TunnelConfig) -> AppResult<Self> {
        let rpc_port = free_local_port()?;
        let mut command = Command::new(&binaries.core);
        command
            .args(config.args(rpc_port))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(dir) = binaries.core.parent() {
            command.current_dir(dir);
        }
        // EasyTier 会读取 ET_* 环境变量，先清掉用户环境里的同名变量，再传入密钥。
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("ET_") {
                command.env_remove(key);
            }
        }
        command.env("ET_NETWORK_SECRET", &config.network_secret);
        hide_console(&mut command);

        let mut child = command.spawn().map_err(|e| {
            AppError::new(ErrorCode::P2pTunnelExited).with_detail(format!("spawn: {e}"))
        })?;
        let log = Arc::new(Mutex::new(VecDeque::with_capacity(LOG_LINES)));
        let secret = config.network_secret.clone();
        if let Some(out) = child.stdout.take() {
            collect_lines(out, log.clone(), secret.clone());
        }
        if let Some(err) = child.stderr.take() {
            collect_lines(err, log.clone(), secret);
        }
        let mut tunnel = Self {
            child,
            cli: binaries.cli.clone(),
            rpc_port,
            log,
        };
        tokio::time::sleep(STARTUP_GRACE).await;
        if let TunnelState::Exited { code } = tunnel.state() {
            return Err(tunnel.exited_error(code));
        }
        tracing::info!(rpc_port, "EasyTier 已启动");
        Ok(tunnel)
    }

    /// `easytier-core` 的进程号；进程已退出时为 `None`。
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// 当前状态。
    pub fn state(&mut self) -> TunnelState {
        match self.child.try_wait() {
            Ok(None) => TunnelState::Running,
            Ok(Some(status)) => TunnelState::Exited {
                code: status.code(),
            },
            Err(e) => {
                tracing::warn!(error = %e, "无法读取 EasyTier 进程状态");
                TunnelState::Exited { code: None }
            }
        }
    }

    /// 最近的 EasyTier 输出（网络密钥已替换为 `***`），用于诊断。
    pub fn recent_log(&self) -> Vec<String> {
        self.log
            .lock()
            .map(|log| log.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 网络中的节点，包括本机（`is_local`）和公共节点。
    pub async fn peers(&mut self) -> AppResult<Vec<Peer>> {
        let output = self.cli(&["-o", "json", "peer"]).await?;
        parse_peers(&output)
    }

    /// 添加一条端口转发（房客把本机端口转到房主的虚拟 IP）。
    pub async fn add_port_forward(&mut self, forward: &PortForward) -> AppResult<()> {
        let local = forward.local.to_string();
        let remote = forward.remote.to_string();
        self.cli(&[
            "port-forward",
            "add",
            forward.proto.as_str(),
            &local,
            &remote,
        ])
        .await?;
        Ok(())
    }

    /// 结束进程并等待退出。
    pub async fn stop(mut self) -> AppResult<()> {
        if self.state() == TunnelState::Running {
            self.child.kill().await?;
        }
        tracing::info!("EasyTier 已停止");
        Ok(())
    }

    async fn cli(&mut self, args: &[&str]) -> AppResult<Vec<u8>> {
        if let TunnelState::Exited { code } = self.state() {
            return Err(self.exited_error(code));
        }
        let portal = format!("127.0.0.1:{}", self.rpc_port);
        let mut command = Command::new(&self.cli);
        command
            .args(["-p", &portal])
            .args(args)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        hide_console(&mut command);
        let output = tokio::time::timeout(CLI_TIMEOUT, command.output())
            .await
            .map_err(|_| {
                AppError::new(ErrorCode::CommonInternal).with_detail("easytier-cli timed out")
            })??;
        if !output.status.success() {
            return Err(
                AppError::new(ErrorCode::CommonInternal).with_detail(format!(
                    "easytier-cli {:?}: {}",
                    args,
                    String::from_utf8_lossy(&output.stderr).trim()
                )),
            );
        }
        Ok(output.stdout)
    }

    fn exited_error(&self, code: Option<i32>) -> AppError {
        let tail: Vec<String> = self.recent_log().into_iter().rev().take(20).collect();
        let tail: Vec<&str> = tail.iter().rev().map(String::as_str).collect();
        let mut err = AppError::new(ErrorCode::P2pTunnelExited).with_detail(tail.join("\n"));
        if let Some(code) = code {
            err = err.with_param("code", code);
        }
        err
    }
}

/// 向系统要一个本机空闲 TCP 端口，用作 EasyTier 的 RPC 端口。
fn free_local_port() -> AppResult<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    Ok(listener.local_addr()?.port())
}

fn collect_lines<R>(reader: R, log: Arc<Mutex<VecDeque<String>>>, secret: String)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = if secret.is_empty() {
                line
            } else {
                line.replace(&secret, "***")
            };
            tracing::debug!(target: "easytier", "{line}");
            if let Ok(mut log) = log.lock() {
                if log.len() == LOG_LINES {
                    log.pop_front();
                }
                log.push_back(line);
            }
        }
    });
}

#[cfg(windows)]
pub(crate) fn hide_console(command: &mut Command) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn hide_console(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn room() -> RoomCode {
        RoomCode::parse("U/YNZE-U61D-2206-HXRG").unwrap()
    }

    fn value_after<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
        args.windows(2)
            .filter(|w| w[0] == flag)
            .map(|w| w[1].as_str())
            .collect()
    }

    #[test]
    fn host_args_expose_only_game_and_signal_ports() {
        let peers = vec!["tcp://a:1".to_owned(), "tcp://b:2".to_owned()];
        let args = TunnelConfig::host(&room(), peers, 33768, 25565).args(15888);
        assert!(args.contains(&"--no-tun".to_owned()));
        assert!(args.contains(&"--p2p-only".to_owned()));
        assert_eq!(
            value_after(&args, "--network-name"),
            ["scaffolding-mc-YNZE-U61D"]
        );
        assert_eq!(
            value_after(&args, "--hostname"),
            ["scaffolding-mc-server-33768"]
        );
        assert_eq!(value_after(&args, "--ipv4"), ["10.144.144.1"]);
        assert_eq!(value_after(&args, "-r"), ["127.0.0.1:15888"]);
        assert_eq!(value_after(&args, "-p"), ["tcp://a:1", "tcp://b:2"]);
        let whitelist: Vec<&String> = args.iter().filter(|a| a.contains("-whitelist=")).collect();
        assert_eq!(
            whitelist,
            [
                "--tcp-whitelist=33768",
                "--tcp-whitelist=25565",
                "--udp-whitelist=25565"
            ]
        );
        assert!(!args.contains(&"--dhcp".to_owned()));
    }

    #[test]
    fn guest_args_deny_all_inbound_and_use_dhcp() {
        let args = TunnelConfig::guest(&room(), vec![]).args(1);
        assert!(args.contains(&"--dhcp".to_owned()));
        assert!(args.contains(&"--tcp-whitelist=0".to_owned()));
        assert!(args.contains(&"--udp-whitelist=0".to_owned()));
        assert!(value_after(&args, "--hostname").is_empty());
    }

    #[test]
    fn secret_never_on_command_line() {
        for config in [
            TunnelConfig::host(&room(), vec![], 33768, 25565),
            TunnelConfig::guest(&room(), vec![]),
        ] {
            let args = config.args(1).join(" ");
            assert!(!args.contains("2206-HXRG"));
            assert!(!args.contains("--network-secret"));
        }
    }
}
