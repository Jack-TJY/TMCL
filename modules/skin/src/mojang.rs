//! 正版账号的皮肤与披风：`api.minecraftservices.com` 的档案接口（地址见
//! `docs/contracts/endpoints.md`）。
//!
//! 所有请求都带 `Authorization: Bearer <Minecraft 访问令牌>`；令牌由调用方经
//! `tmcl_auth::store::prepare_launch` 取得（快过期时自动续期），不写入日志或错误详情。

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::USER_AGENT;
use ts_rs::TS;

use crate::SkinVariant;
use crate::library::validate_png;

/// Minecraft 服务的根地址。
pub const API_BASE: &str = "https://api.minecraftservices.com";

/// 单个请求的总时限。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// multipart 分隔符；请求体只含 `variant` 文本与 PNG，PNG 中不会出现这个 ASCII 串。
const BOUNDARY: &str = "----tmcl-skin-upload-7f3a9c";

/// 档案中的一个皮肤。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ProfileSkin {
    pub id: String,
    /// 皮肤图片地址（`textures.minecraft.net`）。
    pub url: String,
    pub variant: SkinVariant,
    /// 是否为当前使用的皮肤。
    pub active: bool,
}

/// 档案中的一个披风。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ProfileCape {
    pub id: String,
    /// 披风名称，例如 `Migrator`。
    pub alias: String,
    /// 披风图片地址。
    pub url: String,
    /// 是否为当前显示的披风；全部为 `false` 表示隐藏披风。
    pub active: bool,
}

/// 玩家档案中的皮肤与披风。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SkinProfile {
    /// 不带连字符的 UUID。
    pub id: String,
    pub name: String,
    pub skins: Vec<ProfileSkin>,
    pub capes: Vec<ProfileCape>,
}

/// 接口返回的原始结构；`state` 为 `ACTIVE` / `INACTIVE`。
#[derive(Deserialize)]
struct RawProfile {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<RawSkin>,
    #[serde(default)]
    capes: Vec<RawCape>,
}

#[derive(Deserialize)]
struct RawSkin {
    id: String,
    state: String,
    url: String,
    /// `CLASSIC` / `SLIM`。
    variant: String,
}

#[derive(Deserialize)]
struct RawCape {
    id: String,
    state: String,
    url: String,
    #[serde(default)]
    alias: String,
}

impl From<RawProfile> for SkinProfile {
    fn from(raw: RawProfile) -> Self {
        let active = |state: &str| state.eq_ignore_ascii_case("ACTIVE");
        Self {
            id: raw.id,
            name: raw.name,
            skins: raw
                .skins
                .into_iter()
                .map(|s| ProfileSkin {
                    active: active(&s.state),
                    id: s.id,
                    url: s.url,
                    variant: if s.variant.eq_ignore_ascii_case("slim") {
                        SkinVariant::Slim
                    } else {
                        SkinVariant::Classic
                    },
                })
                .collect(),
            capes: raw
                .capes
                .into_iter()
                .map(|c| ProfileCape {
                    active: active(&c.state),
                    id: c.id,
                    alias: c.alias,
                    url: c.url,
                })
                .collect(),
        }
    }
}

/// 皮肤与披风接口的客户端。
#[derive(Debug, Clone)]
pub struct MojangClient {
    client: reqwest::Client,
    base: String,
}

impl MojangClient {
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
        format!("{}/minecraft/profile{path}", self.base)
    }

    /// 读取档案中的皮肤与披风（`GET /minecraft/profile`）。
    ///
    /// 令牌失效（401）返回 `AUTH_MS_SESSION_EXPIRED`，没有档案（404）返回 `AUTH_MC_NO_PROFILE`。
    pub async fn profile(&self, token: &str, ctx: &TaskCtx) -> AppResult<SkinProfile> {
        let url = self.url("");
        let bytes = self
            .send(self.client.get(&url).bearer_auth(token), &url, ctx)
            .await?;
        parse_profile(&url, &bytes)
    }

    /// 上传皮肤并设为当前皮肤（`POST /minecraft/profile/skins`，multipart：`variant`、`file`）。
    ///
    /// 先在本地校验 PNG；服务器拒收图片（400）同样返回 `SKIN_INVALID_IMAGE`。
    pub async fn upload_skin(
        &self,
        token: &str,
        variant: SkinVariant,
        png: &[u8],
        ctx: &TaskCtx,
    ) -> AppResult<SkinProfile> {
        validate_png(png)?;
        let url = self.url("/skins");
        let request = self
            .client
            .post(&url)
            .bearer_auth(token)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .body(multipart_body(variant, png));
        let bytes = match self.send(request, &url, ctx).await {
            Err(err) if err.params.get("status").map(String::as_str) == Some("400") => {
                return Err(AppError::new(ErrorCode::SkinInvalidImage)
                    .with_detail("rejected by minecraft services"));
            }
            other => other?,
        };
        self.profile_or_refetch(&url, &bytes, token, ctx).await
    }

    /// 显示指定披风（`PUT /minecraft/profile/capes/active`）；`None` 时隐藏披风（`DELETE`）。
    pub async fn set_cape(
        &self,
        token: &str,
        cape_id: Option<&str>,
        ctx: &TaskCtx,
    ) -> AppResult<SkinProfile> {
        let url = self.url("/capes/active");
        let request = match cape_id {
            Some(id) => self
                .client
                .put(&url)
                .json(&serde_json::json!({ "capeId": id })),
            None => self.client.delete(&url),
        };
        let bytes = self.send(request.bearer_auth(token), &url, ctx).await?;
        self.profile_or_refetch(&url, &bytes, token, ctx).await
    }

    /// 修改类接口通常直接返回新档案；响应体不是档案时再读一次。
    async fn profile_or_refetch(
        &self,
        url: &str,
        bytes: &[u8],
        token: &str,
        ctx: &TaskCtx,
    ) -> AppResult<SkinProfile> {
        match parse_profile(url, bytes) {
            Ok(profile) => Ok(profile),
            Err(_) => self.profile(token, ctx).await,
        }
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
        match status {
            200..=299 => Ok(bytes),
            401 => Err(AppError::new(ErrorCode::AuthMsSessionExpired)),
            404 if url == self.url("") => Err(AppError::new(ErrorCode::AuthMcNoProfile)),
            _ => Err(AppError::new(ErrorCode::NetHttpStatus)
                .with_param("url", url)
                .with_param("status", status)),
        }
    }
}

fn parse_profile(url: &str, bytes: &[u8]) -> AppResult<SkinProfile> {
    serde_json::from_slice::<RawProfile>(bytes)
        .map(SkinProfile::from)
        .map_err(|e| {
            AppError::new(ErrorCode::CommonJson)
                .with_param("url", url)
                .with_detail(e.to_string())
        })
}

fn multipart_body(variant: SkinVariant, png: &[u8]) -> Vec<u8> {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{}\r\n\
         --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\n\
         Content-Type: image/png\r\n\r\n",
        variant.as_str()
    )
    .into_bytes();
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}
