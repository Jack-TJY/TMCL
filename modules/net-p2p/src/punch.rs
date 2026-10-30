//! 读取 EasyTier 的连接与 NAT 状态（`easytier-cli -o json peer`）。
//!
//! 打洞由 EasyTier 完成，这里只解析结果：哪些节点已直连、延迟多少、NAT 类型，
//! 以及网络里的联机中心（房主）在哪里。

use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode};

use crate::room::parse_center_hostname;

/// 网络中的一个节点。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Peer {
    /// EasyTier 节点 ID。
    pub id: String,
    /// 主机名；公共节点以 `PublicServer_` 开头，联机中心为 `scaffolding-mc-server-<端口>`。
    pub hostname: String,
    /// 虚拟 IP；公共节点和尚未分配时为 `None`。
    pub ipv4: Option<Ipv4Addr>,
    /// 是否为本机。
    pub is_local: bool,
    /// 是否已直连（`cost` 为 `p2p`）；经其他节点转发时为 `false`。
    pub direct: bool,
    /// 延迟（毫秒），本机或未知时为 `None`。
    pub latency_ms: Option<f64>,
    /// NAT 类型，EasyTier 的原始名称（如 `FullCone`、`Symmetric`）。
    pub nat_type: String,
    /// EasyTier 版本。
    pub version: String,
}

/// `easytier-cli peer` 每行的 JSON 结构（字段均为字符串）。
#[derive(Deserialize)]
struct RawPeer {
    #[serde(default)]
    id: String,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    ipv4: String,
    #[serde(default)]
    cost: String,
    #[serde(default)]
    lat_ms: String,
    #[serde(default)]
    nat_type: String,
    #[serde(default)]
    version: String,
}

/// 解析 `easytier-cli -o json peer` 的输出。
pub fn parse_peers(output: &[u8]) -> AppResult<Vec<Peer>> {
    let raw: Vec<RawPeer> = serde_json::from_slice(output).map_err(|e| {
        AppError::new(ErrorCode::CommonJson).with_detail(format!("easytier-cli peer: {e}"))
    })?;
    Ok(raw
        .into_iter()
        .map(|p| Peer {
            ipv4: p
                .ipv4
                .parse()
                .ok()
                .filter(|ip: &Ipv4Addr| !ip.is_unspecified()),
            is_local: p.cost == "Local",
            direct: p.cost == "p2p",
            latency_ms: p.lat_ms.parse().ok().filter(|ms: &f64| ms.is_finite()),
            id: p.id,
            hostname: p.hostname,
            nat_type: p.nat_type,
            version: p.version,
        })
        .collect())
}

/// 网络中的联机中心：返回其虚拟 IP 与 Scaffolding 信令端口。
///
/// 规范要求一个网络只有一个联机中心；找到多个时视为没有（未定义行为，不猜）。
pub fn find_center(peers: &[Peer]) -> Option<(Ipv4Addr, u16)> {
    let mut centers = peers
        .iter()
        .filter(|p| !p.is_local)
        .filter_map(|p| Some((p.ipv4?, parse_center_hostname(&p.hostname)?)));
    let first = centers.next()?;
    centers.next().is_none().then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
      {"cidr":"10.144.144.2/24","ipv4":"10.144.144.2","hostname":"guest-pc","cost":"Local","lat_ms":"-","loss_rate":"-","rx_bytes":"-","tx_bytes":"-","tunnel_proto":"-","nat_type":"PortRestricted","id":"111","version":"2.6.4-abc"},
      {"cidr":"","ipv4":"","hostname":"PublicServer_public.easytier.top","cost":"p2p","lat_ms":"31.20","loss_rate":"0.0%","rx_bytes":"1 kB","tx_bytes":"1 kB","tunnel_proto":"tcp","nat_type":"Unknown","id":"222","version":"2.6.4"},
      {"cidr":"10.144.144.1/24","ipv4":"10.144.144.1","hostname":"scaffolding-mc-server-33768","cost":"relay(2)","lat_ms":"80.00","loss_rate":"0.0%","rx_bytes":"0 B","tx_bytes":"0 B","tunnel_proto":"","nat_type":"Symmetric","id":"333","version":"2.5.0"}
    ]"#;

    #[test]
    fn parses_cli_output() {
        let peers = parse_peers(SAMPLE.as_bytes()).unwrap();
        assert_eq!(peers.len(), 3);
        assert!(peers[0].is_local && !peers[0].direct);
        assert_eq!(peers[0].latency_ms, None);
        assert_eq!(peers[1].ipv4, None);
        assert!(peers[1].direct);
        assert_eq!(peers[1].latency_ms, Some(31.2));
        assert_eq!(peers[2].nat_type, "Symmetric");
        assert!(!peers[2].direct);
    }

    #[test]
    fn finds_single_center() {
        let peers = parse_peers(SAMPLE.as_bytes()).unwrap();
        assert_eq!(
            find_center(&peers),
            Some((Ipv4Addr::new(10, 144, 144, 1), 33768))
        );
        let mut two = peers.clone();
        two.push(Peer {
            ipv4: Some(Ipv4Addr::new(10, 144, 144, 9)),
            ..peers[2].clone()
        });
        assert_eq!(find_center(&two), None);
        assert_eq!(find_center(&peers[..2]), None);
    }

    #[test]
    fn invalid_json_is_an_error() {
        let err = parse_peers(b"not json").unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonJson);
    }
}
