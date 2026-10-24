//! Xbox Live 用户认证与 XSTS 授权（PLAN.md M0-10 第二、三步）。

use std::fmt;

use serde::Deserialize;
use serde_json::json;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};

use crate::http;
use crate::microsoft::MsClient;

/// XSTS 的信赖方：Minecraft 服务。
pub const MC_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";
/// 微软账号还没有 Xbox 档案。
const XERR_NO_XBOX_ACCOUNT: u64 = 2_148_916_233;
/// 未成年账号需要加入家庭组。
const XERR_CHILD_ACCOUNT: u64 = 2_148_916_238;

/// Xbox 令牌与用户哈希（`uhs`）。
#[derive(Clone, PartialEq, Eq)]
pub struct XboxToken {
    pub token: String,
    pub user_hash: String,
}

impl fmt::Debug for XboxToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 用户哈希也是可追踪的标识，与令牌一起打码（AGENTS.md §11）。
        f.write_str("XboxToken(***)")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawToken {
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<UserClaim>,
}

#[derive(Deserialize)]
struct UserClaim {
    uhs: String,
}

fn parse_token(url: &str, bytes: &[u8]) -> AppResult<XboxToken> {
    let raw: RawToken = http::json(url, bytes)?;
    let user_hash = raw
        .display_claims
        .xui
        .into_iter()
        .next()
        .map(|c| c.uhs)
        .ok_or_else(|| {
            AppError::new(ErrorCode::CommonJson)
                .with_param("url", url)
                .with_detail("missing DisplayClaims.xui")
        })?;
    Ok(XboxToken {
        token: raw.token,
        user_hash,
    })
}

/// XSTS 拒绝授权时的错误码。
fn xsts_error(url: &str, status: u16, bytes: &[u8]) -> AppError {
    #[derive(Deserialize)]
    struct Body {
        #[serde(rename = "XErr")]
        xerr: u64,
    }
    let Some(xerr) = serde_json::from_slice::<Body>(bytes).ok().map(|b| b.xerr) else {
        return http::http_status(url, status);
    };
    match xerr {
        XERR_NO_XBOX_ACCOUNT => AppError::new(ErrorCode::AuthXstsNoXboxAccount),
        XERR_CHILD_ACCOUNT => AppError::new(ErrorCode::AuthXstsChildAccount),
        other => AppError::new(ErrorCode::AuthXstsDenied).with_param("xerr", other),
    }
}

impl MsClient {
    /// 用微软访问令牌换 Xbox Live 用户令牌。
    pub async fn xbox_user(&self, ms_access_token: &str, ctx: &TaskCtx) -> AppResult<XboxToken> {
        let url = &self.endpoints.xbox_user;
        let body = json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access_token}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        });
        let request = self
            .client
            .post(url)
            .header("Accept", "application/json")
            .json(&body);
        let (status, bytes) = http::send(request, url, ctx).await?;
        if !(200..300).contains(&status) {
            return Err(http::http_status(url, status));
        }
        parse_token(url, &bytes)
    }

    /// 用 Xbox 用户令牌换 Minecraft 服务的 XSTS 令牌。
    ///
    /// `XErr` 2148916233、2148916238 分别返回 `AUTH_XSTS_NO_XBOX_ACCOUNT`、
    /// `AUTH_XSTS_CHILD_ACCOUNT`，其他拒绝返回 `AUTH_XSTS_DENIED`（参数 `xerr`）。
    pub async fn xsts(&self, user: &XboxToken, ctx: &TaskCtx) -> AppResult<XboxToken> {
        let url = &self.endpoints.xsts;
        let body = json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [user.token] },
            "RelyingParty": MC_RELYING_PARTY,
            "TokenType": "JWT",
        });
        let request = self
            .client
            .post(url)
            .header("Accept", "application/json")
            .json(&body);
        let (status, bytes) = http::send(request, url, ctx).await?;
        if !(200..300).contains(&status) {
            return Err(xsts_error(url, status, &bytes));
        }
        parse_token(url, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xerr_maps_to_codes() {
        let body = |x: u64| {
            format!(
                r#"{{"Identity":"0","XErr":{x},"Message":"","Redirect":"https://start.ui.xboxlive.com/AddChildToFamily"}}"#
            )
        };
        let map = |x| xsts_error("u", 401, body(x).as_bytes());
        assert_eq!(map(2148916233).code, ErrorCode::AuthXstsNoXboxAccount);
        assert_eq!(map(2148916238).code, ErrorCode::AuthXstsChildAccount);
        let other = map(2148916235);
        assert_eq!(other.code, ErrorCode::AuthXstsDenied);
        assert_eq!(other.params["xerr"], "2148916235");
        assert_eq!(xsts_error("u", 500, b"").code, ErrorCode::NetHttpStatus);
    }

    #[test]
    fn token_needs_user_hash() {
        let ok = br#"{"IssueInstant":"","NotAfter":"","Token":"t","DisplayClaims":{"xui":[{"uhs":"h"}]}}"#;
        let token = parse_token("u", ok).unwrap();
        assert_eq!((token.token.as_str(), token.user_hash.as_str()), ("t", "h"));
        assert!(!format!("{token:?}").contains('h'));
        let missing = br#"{"Token":"t","DisplayClaims":{"xui":[]}}"#;
        assert_eq!(
            parse_token("u", missing).unwrap_err().code,
            ErrorCode::CommonJson
        );
    }
}
