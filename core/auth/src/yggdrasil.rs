//! Yggdrasil 协议（外置登录皮肤站使用），按 authlib-injector 的规范实现：
//! <https://github.com/yushijinhun/authlib-injector/wiki/Yggdrasil-服务端技术规范>。
//!
//! 只发送用户主动输入的账号密码；令牌与密码不写入日志或错误详情。

use std::fmt;
use std::time::Duration;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::{HttpClient, USER_AGENT};

/// API 地址指示（ALI）响应头。
const ALI_HEADER: &str = "x-authlib-injector-api-location";
/// 单个 POST 请求的总时限。
const POST_TIMEOUT: Duration = Duration::from_secs(30);

/// 皮肤站信息：规整后的 API 地址、名称与原始元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    /// API 根地址，不带末尾 `/`。
    pub api_url: String,
    /// `meta.serverName`；缺失时用主机名。
    pub name: String,
    /// API 根地址返回的原始 JSON，启动时作为 `prefetched` 传给 authlib-injector。
    pub metadata: String,
}

/// 皮肤站上的一个角色。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// 不带连字符的 UUID。
    pub id: String,
    pub name: String,
}

/// 登录得到的会话；作为外置登录账号的令牌存入密钥库。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub client_token: String,
    pub profile: Profile,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("access_token", &"***")
            .field("client_token", &"***")
            .field("profile", &self.profile)
            .finish()
    }
}

fn invalid_server(url: &str) -> AppError {
    AppError::new(ErrorCode::AuthAuthlibServerInvalid).with_param("url", url)
}

/// 把用户输入规整为 URL：缺少协议时补 `https://`。
fn parse_input(input: &str) -> AppResult<Url> {
    let trimmed = input.trim();
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    match Url::parse(&with_scheme) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.host().is_some() => Ok(url),
        _ => Err(invalid_server(trimmed)),
    }
}

fn trim_slash(url: &Url) -> String {
    url.as_str().trim_end_matches('/').to_owned()
}

async fn get_text(
    http: &HttpClient,
    url: &str,
    ctx: &TaskCtx,
) -> AppResult<(Option<String>, Vec<u8>)> {
    let response = http.get(url, ctx).await?;
    let ali = response
        .headers()
        .get(ALI_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = http.body(response, url).bytes(ctx).await?;
    Ok((ali, body))
}

/// 读取皮肤站元数据。`input` 可以是皮肤站首页（经 ALI 指向 API）或 API 地址本身。
pub async fn resolve_server(
    http: &HttpClient,
    input: &str,
    ctx: &TaskCtx,
) -> AppResult<ServerInfo> {
    let mut url = parse_input(input)?;
    let (ali, mut body) = get_text(http, url.as_str(), ctx).await?;
    if let Some(location) = ali {
        let target = url
            .join(&location)
            .map_err(|_| invalid_server(url.as_str()))?;
        if target != url {
            url = target;
            body = get_text(http, url.as_str(), ctx).await?.1;
        }
    }
    let api_url = trim_slash(&url);
    let meta: Value = serde_json::from_slice(&body).map_err(|_| invalid_server(&api_url))?;
    // 合法的 Yggdrasil API 根返回带 `meta` 与 `skinDomains` 的对象。
    if !meta.get("meta").is_some_and(Value::is_object) || meta.get("skinDomains").is_none() {
        return Err(invalid_server(&api_url));
    }
    let name = meta["meta"]["serverName"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| url.host_str().unwrap_or_default().to_owned());
    let metadata = String::from_utf8(body).map_err(|_| invalid_server(&api_url))?;
    Ok(ServerInfo {
        api_url,
        name,
        metadata,
    })
}

/// 发送 JSON POST，返回状态码与响应体。可被 `ctx` 取消。
async fn post(
    client: &reqwest::Client,
    url: &str,
    body: &Value,
    ctx: &TaskCtx,
) -> AppResult<(u16, Vec<u8>)> {
    let request = async {
        let response = client.post(url).json(body).send().await?;
        let status = response.status().as_u16();
        Ok::<_, reqwest::Error>((status, response.bytes().await?.to_vec()))
    };
    let result = tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
        r = tokio::time::timeout(POST_TIMEOUT, request) => r,
    };
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => {
            let code = if err.is_timeout() {
                ErrorCode::NetTimeout
            } else if err.is_connect() {
                ErrorCode::NetConnect
            } else {
                ErrorCode::NetRequest
            };
            // reqwest 的错误信息不含请求体，可以作为详情。
            Err(AppError::new(code)
                .with_param("url", url)
                .with_detail(err.without_url().to_string()))
        }
        Err(_) => Err(AppError::new(ErrorCode::NetTimeout).with_param("url", url)),
    }
}

/// 发送 POST 的客户端，与 [`HttpClient`] 使用相同的 `User-Agent`。
#[derive(Debug, Clone)]
pub struct Yggdrasil {
    client: reqwest::Client,
}

impl Yggdrasil {
    /// 创建客户端。
    pub fn new() -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| AppError::new(ErrorCode::NetRequest).with_detail(e.to_string()))?;
        Ok(Self { client })
    }

    /// 用账号密码登录。角色有多个且皮肤站没有选定时使用第一个。
    ///
    /// 账号或密码错误返回 `AUTH_YGGDRASIL_INVALID_CREDENTIALS`，没有角色返回
    /// `AUTH_YGGDRASIL_NO_PROFILE`。
    pub async fn authenticate(
        &self,
        api_url: &str,
        username: &str,
        password: &str,
        ctx: &TaskCtx,
    ) -> AppResult<Session> {
        let url = format!("{api_url}/authserver/authenticate");
        let client_token = uuid::Uuid::new_v4().simple().to_string();
        let body = json!({
            "agent": { "name": "Minecraft", "version": 1 },
            "username": username,
            "password": password,
            "clientToken": client_token,
            "requestUser": false,
        });
        let (status, bytes) = post(&self.client, &url, &body, ctx).await?;
        let response = parse_response(
            &url,
            status,
            &bytes,
            ErrorCode::AuthYggdrasilInvalidCredentials,
        )?;
        let session = session_from(&url, &response, &client_token)?;
        match session {
            Bound::Selected(session) => Ok(session),
            Bound::Unselected(access_token, profile) => {
                // 未选定角色时用 refresh 绑定，之后的令牌才能进入服务器。
                let unbound = Session {
                    access_token,
                    client_token,
                    profile,
                };
                self.refresh(api_url, &unbound, ctx).await
            }
        }
    }

    /// 令牌是否仍然有效。
    pub async fn validate(
        &self,
        api_url: &str,
        session: &Session,
        ctx: &TaskCtx,
    ) -> AppResult<bool> {
        let url = format!("{api_url}/authserver/validate");
        let body = json!({
            "accessToken": session.access_token,
            "clientToken": session.client_token,
        });
        let (status, _) = post(&self.client, &url, &body, ctx).await?;
        match status {
            200..=299 => Ok(true),
            401 | 403 => Ok(false),
            _ => Err(http_status(&url, status)),
        }
    }

    /// 刷新令牌（同时绑定 `session.profile`）。会话已失效需要重新输入密码时返回
    /// `AUTH_YGGDRASIL_SESSION_EXPIRED`。
    pub async fn refresh(
        &self,
        api_url: &str,
        session: &Session,
        ctx: &TaskCtx,
    ) -> AppResult<Session> {
        let url = format!("{api_url}/authserver/refresh");
        let body = json!({
            "accessToken": session.access_token,
            "clientToken": session.client_token,
            "requestUser": false,
            "selectedProfile": { "id": session.profile.id, "name": session.profile.name },
        });
        let (status, bytes) = post(&self.client, &url, &body, ctx).await?;
        let response = parse_response(&url, status, &bytes, ErrorCode::AuthYggdrasilSessionExpired)
            .map_err(|e| {
                if e.code == ErrorCode::AuthYggdrasilSessionExpired {
                    e.with_param("name", &session.profile.name)
                } else {
                    e
                }
            })?;
        match session_from(&url, &response, &session.client_token)? {
            Bound::Selected(next) => Ok(next),
            Bound::Unselected(access_token, _) => Ok(Session {
                access_token,
                client_token: session.client_token.clone(),
                profile: session.profile.clone(),
            }),
        }
    }

    /// 启动前确保令牌有效：先 validate，失效则 refresh。返回是否换了新令牌与当前会话。
    pub async fn ensure_valid(
        &self,
        api_url: &str,
        session: Session,
        ctx: &TaskCtx,
    ) -> AppResult<(bool, Session)> {
        if self.validate(api_url, &session, ctx).await? {
            return Ok((false, session));
        }
        Ok((true, self.refresh(api_url, &session, ctx).await?))
    }
}

fn http_status(url: &str, status: u16) -> AppError {
    AppError::new(ErrorCode::NetHttpStatus)
        .with_param("url", url)
        .with_param("status", status)
}

/// 解析响应：2xx 返回 JSON；401/403 的 `ForbiddenOperationException` 映射为 `forbidden`。
fn parse_response(url: &str, status: u16, bytes: &[u8], forbidden: ErrorCode) -> AppResult<Value> {
    if (200..300).contains(&status) {
        return serde_json::from_slice(bytes).map_err(|e| {
            AppError::new(ErrorCode::CommonJson)
                .with_param("url", url)
                .with_detail(e.to_string())
        });
    }
    let error = serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|v| v["error"].as_str().map(str::to_owned));
    if matches!(status, 401 | 403) || error.as_deref() == Some("ForbiddenOperationException") {
        // 皮肤站的 errorMessage 可能含用户名，不放进详情。
        let detail = error.unwrap_or_else(|| format!("HTTP {status}"));
        return Err(AppError::new(forbidden).with_detail(detail));
    }
    Err(http_status(url, status))
}

enum Bound {
    Selected(Session),
    /// 有访问令牌但皮肤站没有选定角色。
    Unselected(String, Profile),
}

fn session_from(url: &str, response: &Value, client_token: &str) -> AppResult<Bound> {
    let invalid = || {
        AppError::new(ErrorCode::CommonJson)
            .with_param("url", url)
            .with_detail("missing accessToken or profile")
    };
    let access_token = response["accessToken"]
        .as_str()
        .ok_or_else(invalid)?
        .to_owned();
    let client_token = response["clientToken"]
        .as_str()
        .unwrap_or(client_token)
        .to_owned();
    let profile = |v: &Value| -> Option<Profile> { serde_json::from_value(v.clone()).ok() };
    if let Some(selected) = response.get("selectedProfile").and_then(profile) {
        return Ok(Bound::Selected(Session {
            access_token,
            client_token,
            profile: selected,
        }));
    }
    let first = response["availableProfiles"]
        .as_array()
        .and_then(|list| list.iter().find_map(profile))
        .ok_or_else(|| AppError::new(ErrorCode::AuthYggdrasilNoProfile))?;
    Ok(Bound::Unselected(access_token, first))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_gets_https_by_default() {
        assert_eq!(
            parse_input(" littleskin.cn/api/yggdrasil ")
                .unwrap()
                .as_str(),
            "https://littleskin.cn/api/yggdrasil"
        );
        assert_eq!(
            parse_input("http://127.0.0.1:8080/").unwrap().as_str(),
            "http://127.0.0.1:8080/"
        );
        for bad in ["", "ftp://a.test", "https://"] {
            assert_eq!(
                parse_input(bad).unwrap_err().code,
                ErrorCode::AuthAuthlibServerInvalid,
                "{bad}"
            );
        }
    }

    #[test]
    fn session_debug_hides_tokens() {
        let session = Session {
            access_token: "secret-access".into(),
            client_token: "secret-client".into(),
            profile: Profile {
                id: "0".repeat(32),
                name: "Steve".into(),
            },
        };
        let text = format!("{session:?}");
        assert!(!text.contains("secret"));
        assert!(text.contains("Steve"));
    }

    #[test]
    fn forbidden_maps_to_given_code() {
        let body =
            br#"{"error":"ForbiddenOperationException","errorMessage":"Invalid credentials."}"#;
        let err =
            parse_response("u", 403, body, ErrorCode::AuthYggdrasilInvalidCredentials).unwrap_err();
        assert_eq!(err.code, ErrorCode::AuthYggdrasilInvalidCredentials);
        assert_eq!(err.detail.as_deref(), Some("ForbiddenOperationException"));
        let err = parse_response(
            "u",
            500,
            b"oops",
            ErrorCode::AuthYggdrasilInvalidCredentials,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::NetHttpStatus);
        assert_eq!(err.params["status"], "500");
    }
}
