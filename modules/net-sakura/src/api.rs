//! SakuraFrp 开放 API v4（`https://api.natfrp.com/v4`，地址见 `docs/contracts/endpoints.md`）。
//!
//! 按官方帮助文档手写请求，不复制或生成 `natfrp/api` 的 OpenAPI 定义（AGPL-3.0，见
//! `docs/research/sakurafrp.md` 第 4 节）。需要登录的接口带 `Authorization: Bearer <访问密钥>`；
//! 密钥不写入日志或错误详情。

use std::collections::BTreeMap;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::USER_AGENT;
use ts_rs::TS;

/// API 根地址。
pub const API_BASE: &str = "https://api.natfrp.com/v4";

/// 单个请求的总时限。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// `POST /tunnel/delete` 一次最多删除的隧道数。
pub const DELETE_BATCH: usize = 10;

/// 节点 `flag` 位：允许创建隧道（满载时为 0）。
pub const FLAG_CAN_CREATE: u32 = 1 << 2;
/// 节点 `flag` 位：内地节点。
pub const FLAG_MAINLAND: u32 = 1 << 3;
/// 节点 `flag` 位：允许 UDP。
pub const FLAG_UDP: u32 = 1 << 5;
/// 节点 `flag` 位：离线。
pub const FLAG_OFFLINE: u32 = 1 << 9;
/// 节点 `flag` 位：允许穿透公开游戏服务器。
pub const FLAG_PUBLIC_GAME: u32 = 1 << 11;

/// 账号信息（`GET /user/info`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SakuraUser {
    pub id: u64,
    pub name: String,
    /// 用户组（套餐）名称。
    pub group: String,
    /// 限速说明，例如 `10 Mbps`，由服务端给出。
    pub speed: String,
    /// 隧道数上限。
    pub tunnel_limit: u32,
    /// 是否已完成实名认证；未实名时界面提示去官网认证，TMCL 不收集身份信息。
    pub verified: bool,
    /// 剩余流量（字节）。
    #[ts(type = "number")]
    pub traffic_left: u64,
}

/// 一个节点（`GET /nodes`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SakuraNode {
    pub id: u32,
    pub name: String,
    pub host: String,
    pub description: String,
    /// 使用该节点需要的用户组等级。
    pub vip: u32,
    /// 原始 `flag`，各位含义见 `FLAG_*`。
    pub flag: u32,
}

impl SakuraNode {
    /// 节点在线且未满载，可以创建隧道。
    pub fn available(&self) -> bool {
        self.flag & FLAG_CAN_CREATE != 0 && self.flag & FLAG_OFFLINE == 0
    }

    /// 允许用于公开游戏服务器：`flag & 2048`，或名称含“多线”（“三线”不算）。
    pub fn allows_public_game(&self) -> bool {
        self.flag & FLAG_PUBLIC_GAME != 0 || self.name.contains("多线")
    }
}

/// 一条隧道（`GET /tunnels`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SakuraTunnel {
    pub id: u64,
    pub name: String,
    /// 隧道类型，例如 `tcp`。
    #[serde(rename = "type")]
    pub kind: String,
    /// 节点 ID。
    pub node: u32,
    #[serde(default)]
    pub local_ip: String,
    #[serde(default)]
    pub local_port: u16,
    /// 远程端口或绑定域名。
    #[serde(default)]
    pub remote: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub note: String,
}

/// 创建 TCP 隧道的参数（`POST /tunnels`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewTunnel {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub node: u32,
    pub local_ip: String,
    pub local_port: u16,
    pub note: String,
}

/// `GET /system/clients` 中的一个下载分类。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClientCategory {
    pub ver: String,
    #[serde(default)]
    pub archs: BTreeMap<String, ClientFile>,
}

/// 某个架构的下载文件。Docker 一类只有链接的条目没有 `hash` 与 `size`。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClientFile {
    pub url: String,
    /// 32 位十六进制 MD5（2026-10-30 实测 `frpc_windows_amd64.exe` 与 MD5 一致）。
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// 服务条款（`GET /system/policy?type=tos`），首次使用时展示给用户确认。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SakuraPolicy {
    /// 更新时间，例如 `2023/01/04`。
    #[serde(default)]
    pub updated: String,
    /// 置顶警告。
    #[serde(default)]
    pub alert: String,
    /// 条款正文。
    pub content: String,
}

#[derive(Deserialize)]
struct RawUser {
    id: u64,
    name: String,
    /// 账号被冻结时只有 `id`、`name`、`avatar`、`ban`。
    #[serde(default)]
    ban: Option<serde_json::Value>,
    #[serde(default)]
    speed: String,
    #[serde(default)]
    tunnels: u32,
    #[serde(default)]
    realname: i64,
    #[serde(default)]
    group: Option<RawGroup>,
    /// `[本日消耗流量, 总剩余流量]`。
    #[serde(default)]
    traffic: Vec<u64>,
}

#[derive(Deserialize)]
struct RawGroup {
    name: String,
}

#[derive(Deserialize)]
struct RawNode {
    name: String,
    host: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    vip: u32,
    #[serde(default)]
    flag: u32,
}

#[derive(Deserialize)]
struct Created {
    id: u64,
}

/// 接口的通用错误响应。
#[derive(Deserialize)]
struct ApiError {
    #[serde(default)]
    msg: String,
}

/// SakuraFrp API 客户端。
#[derive(Debug, Clone)]
pub struct SakuraClient {
    client: reqwest::Client,
    base: String,
}

impl SakuraClient {
    /// 使用官方地址。
    pub fn new() -> AppResult<Self> {
        Self::with_base(API_BASE)
    }

    /// 使用指定的根地址（不带结尾 `/`），用于测试。
    pub fn with_base(base: &str) -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| AppError::new(ErrorCode::NetRequest).with_detail(e.to_string()))?;
        Ok(Self {
            client,
            base: base.trim_end_matches('/').to_owned(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// 读取账号信息，也用于校验访问密钥。账号被冻结时返回 `SAKURA_ACCOUNT_BANNED`。
    pub async fn user_info(&self, key: &str, ctx: &TaskCtx) -> AppResult<SakuraUser> {
        let url = self.url("/user/info");
        let raw: RawUser = self.get(&url, Some(key), ctx).await?;
        if raw.ban.is_some() {
            return Err(AppError::new(ErrorCode::SakuraAccountBanned));
        }
        Ok(SakuraUser {
            id: raw.id,
            name: raw.name,
            group: raw.group.map(|g| g.name).unwrap_or_default(),
            speed: raw.speed,
            tunnel_limit: raw.tunnels,
            verified: raw.realname > 0,
            traffic_left: raw.traffic.get(1).copied().unwrap_or(0),
        })
    }

    /// 节点列表，按 ID 排序。
    pub async fn nodes(&self, key: &str, ctx: &TaskCtx) -> AppResult<Vec<SakuraNode>> {
        let url = self.url("/nodes");
        let raw: BTreeMap<String, RawNode> = self.get(&url, Some(key), ctx).await?;
        let mut nodes: Vec<SakuraNode> = raw
            .into_iter()
            .filter_map(|(id, n)| {
                Some(SakuraNode {
                    id: id.parse().ok()?,
                    name: n.name,
                    host: n.host,
                    description: n.description,
                    vip: n.vip,
                    flag: n.flag,
                })
            })
            .collect();
        nodes.sort_by_key(|n| n.id);
        Ok(nodes)
    }

    /// 当前账号的全部隧道。
    pub async fn tunnels(&self, key: &str, ctx: &TaskCtx) -> AppResult<Vec<SakuraTunnel>> {
        let url = self.url("/tunnels");
        self.get(&url, Some(key), ctx).await
    }

    /// 创建隧道，返回新隧道的 ID。
    pub async fn create_tunnel(
        &self,
        key: &str,
        tunnel: &NewTunnel,
        ctx: &TaskCtx,
    ) -> AppResult<u64> {
        let url = self.url("/tunnels");
        let request = self.client.post(&url).bearer_auth(key).json(tunnel);
        let bytes = self.send(request, &url, ctx).await?;
        parse::<Created>(&url, &bytes).map(|c| c.id)
    }

    /// 删除隧道，每批最多 [`DELETE_BATCH`] 条。
    pub async fn delete_tunnels(&self, key: &str, ids: &[u64], ctx: &TaskCtx) -> AppResult<()> {
        let url = self.url("/tunnel/delete");
        for batch in ids.chunks(DELETE_BATCH) {
            let ids: Vec<String> = batch.iter().map(u64::to_string).collect();
            let body = serde_json::json!({ "ids": ids.join(",") });
            let request = self.client.post(&url).bearer_auth(key).json(&body);
            self.send(request, &url, ctx).await?;
        }
        Ok(())
    }

    /// 客户端下载信息（不需要登录），键为分类名，例如 `frpc`。
    pub async fn clients(&self, ctx: &TaskCtx) -> AppResult<BTreeMap<String, ClientCategory>> {
        let url = self.url("/system/clients");
        self.get(&url, None, ctx).await
    }

    /// 服务条款（不需要登录）。
    pub async fn terms(&self, ctx: &TaskCtx) -> AppResult<SakuraPolicy> {
        let url = self.url("/system/policy?type=tos");
        self.get(&url, None, ctx).await
    }

    async fn get<T: DeserializeOwned>(
        &self,
        url: &str,
        key: Option<&str>,
        ctx: &TaskCtx,
    ) -> AppResult<T> {
        let mut request = self.client.get(url);
        if let Some(key) = key {
            request = request.bearer_auth(key);
        }
        let bytes = self.send(request, url, ctx).await?;
        parse(url, &bytes)
    }

    /// 发送请求，返回 2xx 响应体；可被 `ctx` 取消，有总时限。
    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        url: &str,
        ctx: &TaskCtx,
    ) -> AppResult<Vec<u8>> {
        let request = async {
            let response = request.send().await?;
            let status = response.status().as_u16();
            Ok::<_, reqwest::Error>((status, response.bytes().await?.to_vec()))
        };
        let result = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
            r = tokio::time::timeout(REQUEST_TIMEOUT, request) => r,
        };
        let (status, bytes) = match result {
            Ok(Ok(value)) => value,
            Ok(Err(err)) => {
                let code = if err.is_timeout() {
                    ErrorCode::NetTimeout
                } else if err.is_connect() {
                    ErrorCode::NetConnect
                } else {
                    ErrorCode::NetRequest
                };
                return Err(AppError::new(code)
                    .with_param("url", url)
                    .with_detail(err.without_url().to_string()));
            }
            Err(_) => return Err(AppError::new(ErrorCode::NetTimeout).with_param("url", url)),
        };
        if (200..300).contains(&status) {
            return Ok(bytes);
        }
        let msg = serde_json::from_slice::<ApiError>(&bytes)
            .map(|e| e.msg)
            .unwrap_or_default();
        Err(match status {
            401 => AppError::new(ErrorCode::SakuraKeyInvalid),
            // 其余业务错误（隧道数已满、节点不可用、未实名等）由服务端给出说明，原样交给界面。
            400..=499 if !msg.is_empty() => AppError::new(ErrorCode::SakuraApiError)
                .with_param("msg", &msg)
                .with_param("status", status),
            _ => AppError::new(ErrorCode::NetHttpStatus)
                .with_param("url", url)
                .with_param("status", status),
        })
    }
}

fn parse<T: DeserializeOwned>(url: &str, bytes: &[u8]) -> AppResult<T> {
    serde_json::from_slice(bytes).map_err(|e| {
        AppError::new(ErrorCode::CommonJson)
            .with_param("url", url)
            .with_detail(e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, flag: u32) -> SakuraNode {
        SakuraNode {
            id: 1,
            name: name.to_owned(),
            host: String::new(),
            description: String::new(),
            vip: 0,
            flag,
        }
    }

    #[test]
    fn public_game_nodes() {
        assert!(node("某地 BGP", FLAG_PUBLIC_GAME).allows_public_game());
        assert!(node("某地多线 1", 0).allows_public_game());
        assert!(!node("某地三线 1", 0).allows_public_game());
        assert!(!node("某地 BGP", FLAG_CAN_CREATE).allows_public_game());
    }

    #[test]
    fn availability_needs_capacity_and_online() {
        assert!(node("a", FLAG_CAN_CREATE).available());
        assert!(!node("a", 0).available());
        assert!(!node("a", FLAG_CAN_CREATE | FLAG_OFFLINE).available());
    }
}
