//! EasyTier 公共节点。
//!
//! 原定与陶瓦联机相同的四个节点（`docs/research/p2p.md` 第 7 节），但 EasyTier 官方的
//! `public.easytier.top`、`public2.easytier.cn` 在 2026-10-30 已无法解析。维护者按第 8 节的实测
//! 改为：两个社区 TCP 节点在前（直接建连，约 4 秒找到房间），zkitefly 的两个 https 节点兜底
//! （需要先请求一次拿到真实地址，较慢；node1 只对中国大陆开放，node2 只对大陆以外开放）。
//! 这些节点都由个人维护，可能随时下线；用户可在设置里追加自己的节点，或关闭默认节点。
//!
//! 2026-10-31 实测 HMCL（陶瓦联机）加入 TMCL 房间时，两边没有共同可用的节点就会找不到房间。
//! HMCL 的节点列表从 Glavo 维护的 [`TERRACOTTA_NODE_LIST_URL`] 动态获取，因此 TMCL 开房 / 加入
//! 时也读取这份列表，放在 zkitefly 节点之前；读取失败时用 [`TERRACOTTA_FALLBACK_NODE`]。

use std::time::Duration;

use reqwest::Url;
use serde::Deserialize;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::HttpClient;

/// HMCL 读取陶瓦联机节点列表的地址（JSON：`[{ "url": ..., "region": ... }]`）。
pub const TERRACOTTA_NODE_LIST_URL: &str = "https://terracotta.glavo.site/nodes";
/// 2026-10-31 从 HMCL 日志看到的陶瓦联机节点；读取节点列表失败时使用。
pub const TERRACOTTA_FALLBACK_NODE: &str =
    "https://terracotta.glavo.site/acebc7d8-1208-47fd-b212-d03ac49e36e0";
/// 读取陶瓦联机节点列表的超时。
const TERRACOTTA_TIMEOUT: Duration = Duration::from_secs(5);

/// 默认公共节点。第三项在开房 / 加入时替换为 [`TERRACOTTA_NODE_LIST_URL`] 的最新列表。
pub const DEFAULT_PUBLIC_NODES: [&str; 5] = [
    "tcp://sh.vomiku.com:7910",
    "tcp://us01.225284.xyz:11010",
    TERRACOTTA_FALLBACK_NODE,
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

#[derive(Deserialize)]
struct TerracottaNode {
    url: String,
}

/// 解析陶瓦联机节点列表，只保留通过 [`validate_node`] 的地址（不按地区过滤：
/// 多一个节点只是多一次连接尝试，能提高与对方共用节点的机会）。
pub fn parse_terracotta_nodes(json: &[u8]) -> Vec<String> {
    let nodes: Vec<Option<TerracottaNode>> = serde_json::from_slice(json).unwrap_or_default();
    let mut urls: Vec<String> = Vec::new();
    for url in nodes.into_iter().flatten().map(|n| n.url) {
        if validate_node(&url).is_ok() && !urls.contains(&url) {
            urls.push(url);
        }
    }
    urls
}

/// 读取陶瓦联机节点列表；失败或超时时返回空列表（调用方继续用内置的备用节点）。
pub async fn fetch_terracotta_nodes(http: &HttpClient, ctx: &TaskCtx) -> Vec<String> {
    let fetch = http.get_bytes(TERRACOTTA_NODE_LIST_URL, ctx);
    match tokio::time::timeout(TERRACOTTA_TIMEOUT, fetch).await {
        Ok(Ok(body)) => parse_terracotta_nodes(&body),
        Ok(Err(e)) => {
            tracing::warn!("读取陶瓦联机节点列表失败：{e}");
            Vec::new()
        }
        Err(_) => {
            tracing::warn!("读取陶瓦联机节点列表超时");
            Vec::new()
        }
    }
}

/// 用读到的陶瓦联机节点替换列表中的 [`TERRACOTTA_FALLBACK_NODE`]（没有时不变），并去重。
pub fn with_terracotta_nodes(nodes: Vec<String>, fetched: &[String]) -> Vec<String> {
    if fetched.is_empty() {
        return nodes;
    }
    let mut out: Vec<String> = Vec::new();
    for node in nodes {
        let replacement = if node == TERRACOTTA_FALLBACK_NODE {
            fetched.to_vec()
        } else {
            vec![node]
        };
        for n in replacement {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
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
    fn parses_terracotta_node_list() {
        let json = br#"[
            {"url":"https://terracotta.glavo.site/abc","region":"CN"},
            {"url":"https://etnode.zkitefly.eu.org/node1"},
            null,
            {"url":"not a url"},
            {"url":"https://terracotta.glavo.site/abc"}
        ]"#;
        assert_eq!(
            parse_terracotta_nodes(json),
            [
                "https://terracotta.glavo.site/abc",
                "https://etnode.zkitefly.eu.org/node1"
            ]
        );
        assert!(parse_terracotta_nodes(b"<html>").is_empty());
    }

    #[test]
    fn terracotta_nodes_replace_fallback_in_place() {
        let nodes = effective_nodes(true, &[]);
        let fetched = vec![
            "https://terracotta.glavo.site/new".to_owned(),
            "https://etnode.zkitefly.eu.org/node1".to_owned(),
        ];
        assert_eq!(
            with_terracotta_nodes(nodes.clone(), &fetched),
            [
                "tcp://sh.vomiku.com:7910",
                "tcp://us01.225284.xyz:11010",
                "https://terracotta.glavo.site/new",
                "https://etnode.zkitefly.eu.org/node1",
                "https://etnode.zkitefly.eu.org/node2",
            ]
        );
        assert_eq!(with_terracotta_nodes(nodes.clone(), &[]), nodes);
        // 关闭默认节点时不会凭空加入陶瓦节点。
        assert!(with_terracotta_nodes(Vec::new(), &fetched).is_empty());
    }

    #[test]
    fn custom_nodes_first_without_duplicates() {
        let custom = vec![
            " tcp://my.node:11010 ".to_owned(),
            "not a url".to_owned(),
            "https://etnode.zkitefly.eu.org/node1".to_owned(),
            "tcp://my.node:11010".to_owned(),
        ];
        let nodes = effective_nodes(true, &custom);
        assert_eq!(nodes[0], "tcp://my.node:11010");
        assert_eq!(nodes[1], "https://etnode.zkitefly.eu.org/node1");
        assert_eq!(nodes.len(), 1 + DEFAULT_PUBLIC_NODES.len());
        assert_eq!(
            effective_nodes(false, &custom),
            [
                "tcp://my.node:11010",
                "https://etnode.zkitefly.eu.org/node1"
            ]
        );
        assert!(effective_nodes(false, &[]).is_empty());
    }
}
