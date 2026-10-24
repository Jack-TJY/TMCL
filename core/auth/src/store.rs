//! 微软账号：整条登录链路、令牌保存到系统密钥库、启动前续期（PLAN.md M0-10）。

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, Db, ErrorCode, TaskCtx};
use uuid::Uuid;

use crate::accounts::{self, Account, AccountKind, NewAccount};
use crate::microsoft::{DeviceCode, MsClient, MsToken};
use crate::minecraft::McProfile;
use crate::secrets::SecretStore;

/// 剩余有效期少于这个秒数时，启动前先续期。
const REFRESH_MARGIN_SECS: u64 = 10 * 60;

/// 微软账号的令牌，序列化为 JSON 存入密钥库。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicrosoftSession {
    /// 微软刷新令牌，用于续期整条链路。
    pub refresh_token: String,
    /// Minecraft 访问令牌，启动游戏时作为 `--accessToken`。
    pub access_token: String,
    /// Minecraft 访问令牌的过期时间（unix 秒）。
    pub expires_at: u64,
    pub profile: McProfile,
}

impl fmt::Debug for MicrosoftSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MicrosoftSession")
            .field("refresh_token", &"***")
            .field("access_token", &"***")
            .field("expires_at", &self.expires_at)
            .field("profile", &self.profile)
            .finish()
    }
}

impl MicrosoftSession {
    /// 访问令牌在 `now` 之后至少还有 [`REFRESH_MARGIN_SECS`] 秒有效。
    pub fn is_fresh(&self, now: u64) -> bool {
        self.expires_at > now.saturating_add(REFRESH_MARGIN_SECS)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// 用微软令牌走完 Xbox → XSTS → Minecraft 登录 → 权益与档案。
async fn authorize(ms: &MsClient, token: MsToken, ctx: &TaskCtx) -> AppResult<MicrosoftSession> {
    tracing::debug!("microsoft: xbox user authenticate");
    let user = ms.xbox_user(&token.access_token, ctx).await?;
    tracing::debug!("microsoft: xsts authorize");
    let xsts = ms.xsts(&user, ctx).await?;
    tracing::debug!("microsoft: login_with_xbox");
    let mc = ms.login_with_xbox(&xsts, ctx).await?;
    let profile = ms.require_profile(&mc, ctx).await?;
    Ok(MicrosoftSession {
        refresh_token: token.refresh_token,
        access_token: mc.access_token,
        expires_at: now_secs().saturating_add(mc.expires_in),
        profile,
    })
}

/// 设备码登录：申请设备码并交给 `on_code`（界面显示代码与验证页），
/// 等用户在浏览器中完成登录后走完整条链路。
pub async fn login(
    ms: &MsClient,
    on_code: impl FnOnce(&DeviceCode),
    ctx: &TaskCtx,
) -> AppResult<MicrosoftSession> {
    let code = ms.device_code(ctx).await?;
    on_code(&code);
    let token = ms.poll_token(&code, ctx).await?;
    authorize(ms, token, ctx).await
}

/// 用刷新令牌续期：换新的微软令牌后重走 Xbox 与 Minecraft 登录，档案（玩家名）一并更新。
pub async fn refresh(
    ms: &MsClient,
    session: &MicrosoftSession,
    ctx: &TaskCtx,
) -> AppResult<MicrosoftSession> {
    let token = ms
        .refresh_token(&session.refresh_token, &session.profile.name, ctx)
        .await?;
    authorize(ms, token, ctx).await
}

fn to_secret(session: &MicrosoftSession) -> AppResult<String> {
    serde_json::to_string(session)
        .map_err(|e| AppError::new(ErrorCode::CommonJson).with_detail(e.to_string()))
}

/// 保存登录结果：令牌写入密钥库，账号新增或更新（同一档案只保留一个）并设为默认。
pub fn save_session(
    db: &Db,
    secrets: &dyn SecretStore,
    session: &MicrosoftSession,
) -> AppResult<Account> {
    let uuid = Uuid::parse_str(&session.profile.id).map_err(|_| {
        AppError::new(ErrorCode::CommonJson).with_detail("invalid minecraft profile id")
    })?;
    accounts::upsert(
        db,
        secrets,
        NewAccount {
            kind: AccountKind::Microsoft,
            name: session.profile.name.clone(),
            uuid,
            server: None,
        },
        Some(&to_secret(session)?),
    )
}

/// 启动前准备：读取令牌，快过期时续期并写回密钥库，返回可用于启动的会话。
/// 没有令牌、令牌损坏或刷新令牌失效时返回 `AUTH_MS_SESSION_EXPIRED`，需重新登录。
pub async fn prepare_launch(
    secrets: &dyn SecretStore,
    ms: &MsClient,
    account: &Account,
    ctx: &TaskCtx,
) -> AppResult<MicrosoftSession> {
    let expired =
        || AppError::new(ErrorCode::AuthMsSessionExpired).with_param("name", &account.name);
    let stored = accounts::secret(secrets, &account.id)?.ok_or_else(expired)?;
    let session: MicrosoftSession = serde_json::from_str(&stored).map_err(|_| expired())?;
    if session.is_fresh(now_secs()) {
        return Ok(session);
    }
    tracing::info!(account = %account.id, "refreshing microsoft session");
    let next = refresh(ms, &session, ctx).await?;
    accounts::set_secret(secrets, &account.id, &to_secret(&next)?)?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(expires_at: u64) -> MicrosoftSession {
        MicrosoftSession {
            refresh_token: "secret-refresh".into(),
            access_token: "secret-access".into(),
            expires_at,
            profile: McProfile {
                id: "0".repeat(32),
                name: "Steve".into(),
            },
        }
    }

    #[test]
    fn freshness_keeps_a_margin() {
        let now = 1_000_000;
        assert!(session(now + REFRESH_MARGIN_SECS + 1).is_fresh(now));
        assert!(!session(now + REFRESH_MARGIN_SECS).is_fresh(now));
        assert!(!session(0).is_fresh(now));
    }

    #[test]
    fn debug_hides_tokens() {
        let text = format!("{:?}", session(1));
        assert!(!text.contains("secret"));
        assert!(text.contains("Steve"));
    }
}
