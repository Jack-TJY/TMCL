//! 访问密钥与一次联机会话：为局域网端口建临时 TCP 隧道 → 启动 frpc → 得到联机地址 →
//! 关闭时结束 frpc 并删除隧道。
//!
//! 访问密钥只存系统密钥库（键名 [`KEY_SECRET`]），不写配置文件、日志或诊断信息。
//! 临时隧道的备注为 [`TEMP_NOTE`]；启动器异常退出留下的临时隧道在下次开始联机时清理。

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tmcl_auth::SecretStore;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use ts_rs::TS;

use crate::api::{NewTunnel, SakuraClient, SakuraUser};
use crate::frpc::{Addresses, Frpc};

/// 访问密钥在系统密钥库中的键名。
pub const KEY_SECRET: &str = "tmcl:sakura:access-key";
/// 临时隧道的备注，用来识别 TMCL 自己建的隧道。
pub const TEMP_NOTE: &str = "TMCL temporary tunnel";
/// 隧道名前缀，后接本地端口与随机后缀。
const NAME_PREFIX: &str = "tmcl";

/// 读取保存的访问密钥；没有保存时返回 `SAKURA_KEY_MISSING`。
pub fn load_key(secrets: &dyn SecretStore) -> AppResult<String> {
    secrets
        .get(KEY_SECRET)?
        .filter(|k| !k.is_empty())
        .ok_or_else(|| AppError::new(ErrorCode::SakuraKeyMissing))
}

/// 校验访问密钥（`GET /user/info`）并保存，返回账号信息。密钥无效时不保存。
pub async fn save_key(
    secrets: &dyn SecretStore,
    api: &SakuraClient,
    key: &str,
    ctx: &TaskCtx,
) -> AppResult<SakuraUser> {
    let key = key.trim();
    if key.is_empty() || key.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AppError::new(ErrorCode::SakuraKeyInvalid));
    }
    let user = api.user_info(key, ctx).await?;
    secrets.set(KEY_SECRET, key)?;
    Ok(user)
}

/// 删除保存的访问密钥。
pub fn clear_key(secrets: &dyn SecretStore) -> AppResult<()> {
    secrets.delete(KEY_SECRET)
}

/// 开始联机的参数。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
pub struct StartRequest {
    /// 游戏“对局域网开放”后显示的端口。
    pub local_port: u16,
    /// 节点 ID。
    pub node: u32,
    /// 房间是否会公开发布（论坛、服务器列表、大群等）；公开时只能用允许公开游戏的节点。
    pub public: bool,
}

/// 联机中的房间信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SakuraRoom {
    #[ts(type = "number")]
    pub tunnel_id: u64,
    pub node: u32,
    pub local_port: u16,
    /// 分享给朋友的地址，例如 `frp-xxx.com:10086`。
    pub address: String,
    /// IP 形式的地址；部分节点没有。
    #[serde(default)]
    #[ts(optional)]
    pub ip_address: Option<String>,
}

/// 一次联机会话。丢弃时 frpc 随之结束，但隧道要调用 [`Session::stop`] 才会删除。
#[derive(Debug)]
pub struct Session {
    pub room: SakuraRoom,
    key: String,
    frpc: Frpc,
}

impl Session {
    /// 检查账号与节点，清理残留的临时隧道，建隧道并启动 frpc，等到联机地址后返回。
    ///
    /// 未实名返回 `SAKURA_REALNAME_REQUIRED`；节点不存在、离线或满载返回
    /// `SAKURA_NODE_UNAVAILABLE`；公开房间选了不允许公开游戏的节点返回
    /// `SAKURA_NODE_NOT_PUBLIC`。frpc 启动失败时删除刚建的隧道。
    pub async fn start(
        api: &SakuraClient,
        secrets: &dyn SecretStore,
        frpc_exe: &Path,
        request: &StartRequest,
        timeout: Duration,
        ctx: &TaskCtx,
    ) -> AppResult<Self> {
        let key = load_key(secrets)?;
        let user = api.user_info(&key, ctx).await?;
        if !user.verified {
            return Err(AppError::new(ErrorCode::SakuraRealnameRequired));
        }
        let nodes = api.nodes(&key, ctx).await?;
        let node = nodes
            .iter()
            .find(|n| n.id == request.node)
            .filter(|n| n.available())
            .ok_or_else(|| {
                AppError::new(ErrorCode::SakuraNodeUnavailable).with_param("node", request.node)
            })?;
        if request.public && !node.allows_public_game() {
            return Err(
                AppError::new(ErrorCode::SakuraNodeNotPublic).with_param("node", &node.name)
            );
        }
        if let Err(e) = cleanup_stale(api, &key, ctx).await {
            if e.is_cancelled() {
                return Err(e);
            }
            tracing::warn!("清理残留的樱花临时隧道失败：{e}");
        }

        let tunnel = NewTunnel {
            name: tunnel_name(request.local_port),
            kind: "tcp",
            node: node.id,
            local_ip: "127.0.0.1".to_owned(),
            local_port: request.local_port,
            note: TEMP_NOTE.to_owned(),
        };
        let tunnel_id = api.create_tunnel(&key, &tunnel, ctx).await?;
        tracing::info!("已创建樱花临时隧道 {tunnel_id}（节点 {}）", node.id);
        let started = Frpc::start(frpc_exe, &key, tunnel_id, timeout, ctx).await;
        let (
            frpc,
            Addresses {
                address,
                ip_address,
            },
        ) = match started {
            Ok(v) => v,
            Err(e) => {
                // 取消时也要删掉隧道，所以不复用已取消的 ctx。
                let cleanup = TaskCtx::noop(ctx.kind.clone());
                if let Err(del) = api.delete_tunnels(&key, &[tunnel_id], &cleanup).await {
                    tracing::warn!("删除樱花临时隧道 {tunnel_id} 失败：{del}");
                }
                return Err(e);
            }
        };
        Ok(Self {
            room: SakuraRoom {
                tunnel_id,
                node: node.id,
                local_port: request.local_port,
                address,
                ip_address,
            },
            key,
            frpc,
        })
    }

    /// frpc 是否仍在运行。
    pub fn is_running(&mut self) -> bool {
        self.frpc.is_running()
    }

    /// 结束 frpc 并删除临时隧道。
    pub async fn stop(self, api: &SakuraClient, ctx: &TaskCtx) -> AppResult<()> {
        self.frpc.stop().await;
        api.delete_tunnels(&self.key, &[self.room.tunnel_id], ctx)
            .await?;
        tracing::info!("已删除樱花临时隧道 {}", self.room.tunnel_id);
        Ok(())
    }
}

/// 删除不在线的 TMCL 临时隧道（上次异常退出留下的），返回删除的条数。
pub async fn cleanup_stale(api: &SakuraClient, key: &str, ctx: &TaskCtx) -> AppResult<usize> {
    let stale: Vec<u64> = api
        .tunnels(key, ctx)
        .await?
        .into_iter()
        .filter(|t| t.note == TEMP_NOTE && !t.online)
        .map(|t| t.id)
        .collect();
    if !stale.is_empty() {
        api.delete_tunnels(key, &stale, ctx).await?;
    }
    Ok(stale.len())
}

/// `tmcl<端口><4 位十六进制>`，只含字母和数字。
fn tunnel_name(port: u16) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{NAME_PREFIX}{port}{:04x}", nanos & 0xffff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tmcl_auth::MemorySecretStore;

    #[test]
    fn key_round_trip() {
        let store = MemorySecretStore::default();
        assert_eq!(
            load_key(&store).unwrap_err().code,
            ErrorCode::SakuraKeyMissing
        );
        store.set(KEY_SECRET, "test-key").unwrap();
        assert_eq!(load_key(&store).unwrap(), "test-key");
        clear_key(&store).unwrap();
        assert!(store.keys().is_empty());
    }

    #[test]
    fn tunnel_names_are_alphanumeric() {
        let name = tunnel_name(25565);
        assert!(name.starts_with("tmcl25565"), "{name}");
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
