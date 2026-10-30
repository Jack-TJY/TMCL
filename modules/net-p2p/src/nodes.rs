//! EasyTier 公共节点。
//!
//! 默认使用与陶瓦联机相同的四个社区节点（维护者 2026-10-29 决定，见
//! `docs/research/p2p.md` 第 7 节）；用户可在设置里追加自己的节点，或关闭默认节点。
//! 后两个节点由社区成员 zkitefly 提供，两节点均禁止中转，node1 只对中国大陆开放。

use reqwest::Url;
use tmcl_common::{AppError, AppResult, ErrorCode};

/// 默认公共节点。
pub const DEFAULT_PUBLIC_NODES: [&str; 4] = [
    "tcp://public.easytier.top:11010",
    "tcp://public2.easytier.cn:54321",
    "https://etnode.zkitefly.eu.org/node1",
    "https://etnode.zkitefly.eu.org/node2",
];

/// EasyTier `-p` 参数接受的协议：直连的 tcp/udp/ws/wss/quic/wg，
/// 以及从 http(s)/txt/srv 记录读取节点列表的地址。
const SCHEMES: [&str; 10] = [
    "tcp", "udp", "ws", "wss", "quic", "wg", "http", "https", "txt", "srv",
];

/// 校验一个节点地址：`协议://主机[:端口][/路径]`，协议须在 EasyTier 支持的范围内。
pub fn validate_node(node: &str) -> AppResult<()> {
    let invalid = || AppError::new(ErrorCode::P2pNodeInvalid).with_param("node", node);
    if node.trim() != node || node.chars().any(char::is_whitespace) {
        return Err(invalid());
    }
    let url = Url::parse(node).map_err(|_| invalid())?;
    let has_host = url.host_str().is_some_and(|h| !h.is_empty());
    if !SCHEMES.contains(&url.scheme()) || !has_host || url.password().is_some() {
        return Err(invalid());
    }
    Ok(())
}

/// 启动 EasyTier 时实际使用的节点：先用户节点，再默认节点（`use_default` 为真时），
/// 去掉空白、无效项和重复项。
pub fn effective_nodes(use_default: bool, custom: &[String]) -> Vec<String> {
    let defaults = DEFAULT_PUBLIC_NODES
        .iter()
        .filter(|_| use_default)
        .map(|s| (*s).to_owned());
    let mut nodes: Vec<String> = Vec::new();
    for node in custom.iter().map(|s| s.trim().to_owned()).chain(defaults) {
        if validate_node(&node).is_ok() && !nodes.contains(&node) {
            nodes.push(node);
        }
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        for node in DEFAULT_PUBLIC_NODES {
            validate_node(node).unwrap();
        }
    }

    #[test]
    fn rejects_invalid_nodes() {
        for node in [
            "",
            "public.easytier.top:11010",
            "ftp://example.com",
            "tcp://",
            "tcp://exa mple.com:1",
            " tcp://example.com:1",
            "tcp://user:pass@example.com:1",
        ] {
            let err = validate_node(node).unwrap_err();
            assert_eq!(err.code, ErrorCode::P2pNodeInvalid, "{node}");
        }
        validate_node("udp://[2001:db8::1]:11010").unwrap();
        validate_node("wss://node.example.com/path").unwrap();
    }

    #[test]
    fn custom_nodes_first_without_duplicates() {
        let custom = vec![
            " tcp://my.node:11010 ".to_owned(),
            "not a url".to_owned(),
            "tcp://public.easytier.top:11010".to_owned(),
            "tcp://my.node:11010".to_owned(),
        ];
        let nodes = effective_nodes(true, &custom);
        assert_eq!(nodes[0], "tcp://my.node:11010");
        assert_eq!(nodes[1], "tcp://public.easytier.top:11010");
        assert_eq!(nodes.len(), 1 + DEFAULT_PUBLIC_NODES.len());
        assert_eq!(
            effective_nodes(false, &custom),
            ["tcp://my.node:11010", "tcp://public.easytier.top:11010"]
        );
        assert!(effective_nodes(false, &[]).is_empty());
    }
}
