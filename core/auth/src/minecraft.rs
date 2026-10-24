//! Minecraft 服务（PLAN.md M0-10 第四至六步）：用 XSTS 令牌登录、查询权益与玩家档案。

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};

use crate::http;
use crate::microsoft::MsClient;
use crate::xbox::XboxToken;

/// Minecraft 访问令牌。
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct McToken {
    pub access_token: String,
    /// 有效期，单位秒（通常 86400）。
    pub expires_in: u64,
}

impl fmt::Debug for McToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McToken")
            .field("access_token", &"***")
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// 玩家档案。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McProfile {
    /// 不带连字符的 UUID。
    pub id: String,
    pub name: String,
}

impl MsClient {
    /// 用 XSTS 令牌登录 Minecraft 服务。应用未获批准（403）返回 `AUTH_MC_APP_NOT_APPROVED`。
    pub async fn login_with_xbox(&self, xsts: &XboxToken, ctx: &TaskCtx) -> AppResult<McToken> {
        let url = &self.endpoints.mc_login;
        let body = json!({
            "identityToken": format!("XBL3.0 x={};{}", xsts.user_hash, xsts.token),
        });
        let (status, bytes) = http::send(self.client.post(url).json(&body), url, ctx).await?;
        match status {
            200..=299 => http::json(url, &bytes),
            403 => Err(AppError::new(ErrorCode::AuthMcAppNotApproved)),
            _ => Err(http::http_status(url, status)),
        }
    }

    /// 账号是否有 Minecraft 权益。Xbox Game Pass 账号的权益列表可能为空，
    /// 因此只在没有档案时用来区分“未购买”与“未创建档案”。
    pub async fn has_entitlement(&self, mc: &McToken, ctx: &TaskCtx) -> AppResult<bool> {
        #[derive(Deserialize)]
        struct Body {
            #[serde(default)]
            items: Vec<serde_json::Value>,
        }
        let url = &self.endpoints.mc_entitlements;
        let request = self.client.get(url).bearer_auth(&mc.access_token);
        let (status, bytes) = http::send(request, url, ctx).await?;
        if !(200..300).contains(&status) {
            return Err(http::http_status(url, status));
        }
        Ok(!http::json::<Body>(url, &bytes)?.items.is_empty())
    }

    /// 读取玩家档案；还没有档案（404）时返回 `None`。
    pub async fn profile(&self, mc: &McToken, ctx: &TaskCtx) -> AppResult<Option<McProfile>> {
        let url = &self.endpoints.mc_profile;
        let request = self.client.get(url).bearer_auth(&mc.access_token);
        let (status, bytes) = http::send(request, url, ctx).await?;
        match status {
            200..=299 => http::json(url, &bytes).map(Some),
            404 => Ok(None),
            _ => Err(http::http_status(url, status)),
        }
    }

    /// 查询档案；没有档案时按权益返回 `AUTH_MC_NO_PROFILE` 或 `AUTH_MC_NOT_OWNED`。
    pub async fn require_profile(&self, mc: &McToken, ctx: &TaskCtx) -> AppResult<McProfile> {
        let owned = self.has_entitlement(mc, ctx).await?;
        match self.profile(mc, ctx).await? {
            Some(profile) => Ok(profile),
            None if owned => Err(AppError::new(ErrorCode::AuthMcNoProfile)),
            None => Err(AppError::new(ErrorCode::AuthMcNotOwned)),
        }
    }
}
