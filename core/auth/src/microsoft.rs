//! 微软 OAuth 设备码流程（PLAN.md M0-10 第一步）：申请设备码、轮询令牌、用刷新令牌续期。
//!
//! 地址见 `docs/contracts/endpoints.md`。令牌只出现在请求体与返回值里，
//! 不写入日志或错误详情；`Debug` 输出一律打码。

use std::fmt;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};

use crate::config::ms_client_id;
use crate::http;

/// 申请的权限：Xbox Live 登录与刷新令牌。
pub const SCOPE: &str = "XboxLive.signin offline_access";
/// 设备码授权类型。
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// 服务端没有给出轮询间隔时使用的默认值（RFC 8628 §3.5）。
const DEFAULT_INTERVAL: u64 = 5;
/// 收到 `slow_down` 后增加的间隔（RFC 8628 §3.5）。
const SLOW_DOWN_STEP: u64 = 5;

/// 整条登录链路访问的地址；测试时指向本地模拟服务器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub device_code: String,
    pub token: String,
    pub xbox_user: String,
    pub xsts: String,
    pub mc_login: String,
    pub mc_entitlements: String,
    pub mc_profile: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self::with_bases(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0",
            "https://user.auth.xboxlive.com",
            "https://xsts.auth.xboxlive.com",
            "https://api.minecraftservices.com",
        )
    }
}

impl Endpoints {
    /// 由四个服务的根地址拼出全部地址。
    pub fn with_bases(oauth: &str, xbox_user: &str, xsts: &str, minecraft: &str) -> Self {
        Self {
            device_code: format!("{oauth}/devicecode"),
            token: format!("{oauth}/token"),
            xbox_user: format!("{xbox_user}/user/authenticate"),
            xsts: format!("{xsts}/xsts/authorize"),
            mc_login: format!("{minecraft}/authentication/login_with_xbox"),
            mc_entitlements: format!("{minecraft}/entitlements/mcstore"),
            mc_profile: format!("{minecraft}/minecraft/profile"),
        }
    }
}

/// 微软登录客户端：持有 Client ID 与各服务地址，整条链路的请求都经它发出。
#[derive(Clone)]
pub struct MsClient {
    pub(crate) client: reqwest::Client,
    client_id: String,
    pub(crate) endpoints: Endpoints,
}

impl fmt::Debug for MsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Client ID 不进日志（AGENTS.md §11）。
        f.debug_struct("MsClient")
            .field("endpoints", &self.endpoints)
            .finish_non_exhaustive()
    }
}

impl MsClient {
    /// 使用给定的 Client ID 与地址。
    pub fn new(client_id: &str, endpoints: Endpoints) -> AppResult<Self> {
        Ok(Self {
            client: http::client()?,
            client_id: client_id.to_owned(),
            endpoints,
        })
    }

    /// 使用构建时注入的 Client ID 与官方地址；未注入时返回 `AUTH_MS_DISABLED`。
    pub fn from_build() -> AppResult<Self> {
        let id = ms_client_id().ok_or_else(|| AppError::new(ErrorCode::AuthMsDisabled))?;
        Self::new(id, Endpoints::default())
    }

    /// `application/x-www-form-urlencoded` 的 POST 请求。
    fn form_post(&self, url: &str, pairs: &[(&str, &str)]) -> reqwest::RequestBuilder {
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs)
            .finish();
        self.client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
    }

    /// 申请设备码。
    pub async fn device_code(&self, ctx: &TaskCtx) -> AppResult<DeviceCode> {
        let url = &self.endpoints.device_code;
        let form = [("client_id", self.client_id.as_str()), ("scope", SCOPE)];
        let (status, bytes) = http::send(self.form_post(url, &form), url, ctx).await?;
        if !(200..300).contains(&status) {
            return Err(oauth_error(url, status, &bytes, None));
        }
        let raw: RawDeviceCode = http::json(url, &bytes)?;
        Ok(DeviceCode {
            user_code: raw.user_code,
            verification_uri: raw.verification_uri,
            expires_in: raw.expires_in,
            interval: raw.interval.unwrap_or(DEFAULT_INTERVAL),
            device_code: raw.device_code,
        })
    }

    /// 按设备码给出的间隔轮询，直到用户在浏览器中完成登录。
    ///
    /// 用户拒绝返回 `AUTH_MS_DECLINED`，设备码过期返回 `AUTH_MS_CODE_EXPIRED`，可被 `ctx` 取消。
    pub async fn poll_token(&self, code: &DeviceCode, ctx: &TaskCtx) -> AppResult<MsToken> {
        let url = &self.endpoints.token;
        let deadline = Instant::now() + Duration::from_secs(code.expires_in);
        let mut interval = code.interval;
        loop {
            let form = [
                ("grant_type", DEVICE_CODE_GRANT),
                ("client_id", self.client_id.as_str()),
                ("device_code", code.device_code.as_str()),
            ];
            let (status, bytes) = http::send(self.form_post(url, &form), url, ctx).await?;
            if (200..300).contains(&status) {
                return token_from(url, &bytes);
            }
            match oauth_error_code(&bytes).as_deref() {
                Some("authorization_pending") => {}
                Some("slow_down") => interval += SLOW_DOWN_STEP,
                _ => return Err(oauth_error(url, status, &bytes, None)),
            }
            let wait = Duration::from_secs(interval);
            if Instant::now() + wait >= deadline {
                return Err(AppError::new(ErrorCode::AuthMsCodeExpired));
            }
            tokio::select! {
                biased;
                _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }

    /// 用刷新令牌换新的访问令牌。刷新令牌失效时返回 `AUTH_MS_SESSION_EXPIRED`（参数 `name`）。
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
        name: &str,
        ctx: &TaskCtx,
    ) -> AppResult<MsToken> {
        let url = &self.endpoints.token;
        let form = [
            ("grant_type", "refresh_token"),
            ("client_id", self.client_id.as_str()),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ];
        let (status, bytes) = http::send(self.form_post(url, &form), url, ctx).await?;
        if (200..300).contains(&status) {
            return token_from(url, &bytes);
        }
        Err(oauth_error(url, status, &bytes, Some(name)))
    }
}

/// 设备码。`device_code` 只用于轮询，不对外暴露。
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceCode {
    /// 用户在验证页输入的代码。
    pub user_code: String,
    /// 验证页地址，通常是 `https://www.microsoft.com/link`。
    pub verification_uri: String,
    /// 有效期，单位秒。
    pub expires_in: u64,
    /// 轮询间隔，单位秒。
    pub interval: u64,
    device_code: String,
}

impl fmt::Debug for DeviceCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceCode")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("expires_in", &self.expires_in)
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct RawDeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
}

/// 微软访问令牌与刷新令牌。
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct MsToken {
    pub access_token: String,
    pub refresh_token: String,
}

impl fmt::Debug for MsToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MsToken(***)")
    }
}

fn token_from(url: &str, bytes: &[u8]) -> AppResult<MsToken> {
    http::json(url, bytes)
}

/// OAuth 错误响应中的 `error` 字段。
fn oauth_error_code(bytes: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct Body {
        error: String,
    }
    serde_json::from_slice::<Body>(bytes).ok().map(|b| b.error)
}

/// 把 OAuth 错误映射为错误码。`refresh_name` 为 `Some` 表示刷新请求，`invalid_grant`
/// 意味着需要重新登录。详情只放 `error` 字段，`error_description` 含追踪 ID 等杂项，不保留。
fn oauth_error(url: &str, status: u16, bytes: &[u8], refresh_name: Option<&str>) -> AppError {
    let error = oauth_error_code(bytes);
    let code = match (error.as_deref(), refresh_name) {
        (Some("authorization_declined"), _) => ErrorCode::AuthMsDeclined,
        (Some("expired_token" | "bad_verification_code"), None) => ErrorCode::AuthMsCodeExpired,
        (Some("invalid_grant"), Some(_)) => ErrorCode::AuthMsSessionExpired,
        _ => return http::http_status(url, status).with_detail(error.unwrap_or_default()),
    };
    let err = AppError::new(code);
    match refresh_name {
        Some(name) if code == ErrorCode::AuthMsSessionExpired => err.with_param("name", name),
        _ => err,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_endpoints_match_contract() {
        let e = Endpoints::default();
        assert_eq!(
            e.device_code,
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode"
        );
        assert_eq!(e.xsts, "https://xsts.auth.xboxlive.com/xsts/authorize");
        assert_eq!(
            e.mc_login,
            "https://api.minecraftservices.com/authentication/login_with_xbox"
        );
    }

    #[test]
    fn oauth_errors_map_to_codes() {
        let body = |e: &str| format!(r#"{{"error":"{e}","error_description":"AADSTS70016"}}"#);
        let map =
            |e: &str, refresh: Option<&str>| oauth_error("u", 400, body(e).as_bytes(), refresh);
        assert_eq!(
            map("authorization_declined", None).code,
            ErrorCode::AuthMsDeclined
        );
        assert_eq!(
            map("expired_token", None).code,
            ErrorCode::AuthMsCodeExpired
        );
        let expired = map("invalid_grant", Some("Steve"));
        assert_eq!(expired.code, ErrorCode::AuthMsSessionExpired);
        assert_eq!(expired.params["name"], "Steve");
        let other = map("invalid_client", None);
        assert_eq!(other.code, ErrorCode::NetHttpStatus);
        assert_eq!(other.detail.as_deref(), Some("invalid_client"));
        assert!(!format!("{other:?}").contains("AADSTS"));
    }

    #[test]
    fn debug_hides_secrets() {
        let code = DeviceCode {
            user_code: "ABCD-EFGH".into(),
            verification_uri: "https://www.microsoft.com/link".into(),
            expires_in: 900,
            interval: 5,
            device_code: "secret-device".into(),
        };
        assert!(!format!("{code:?}").contains("secret"));
        let token = MsToken {
            access_token: "secret-a".into(),
            refresh_token: "secret-r".into(),
        };
        assert!(!format!("{token:?}").contains("secret"));
        let client =
            MsClient::new("00000000-0000-0000-0000-000000000000", Endpoints::default()).unwrap();
        assert!(!format!("{client:?}").contains("00000000"));
    }
}
