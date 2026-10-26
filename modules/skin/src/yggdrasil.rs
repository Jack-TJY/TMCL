//! 外置登录账号的皮肤由皮肤站自己管理，启动器只负责打开皮肤站页面。
//!
//! 页面地址取自 API 元数据的 `meta.links.homepage`（authlib-injector 规范），
//! 元数据读取失败或没有这一项时退回 API 地址的站点根，例如
//! `https://littleskin.cn/api/yggdrasil` → `https://littleskin.cn/`。

use reqwest::Url;
use serde_json::Value;
use tmcl_auth::yggdrasil::resolve_server;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::HttpClient;

/// 读取皮肤站元数据并返回要在浏览器中打开的页面地址。
///
/// `api_url` 为账号保存的 API 地址；本身不是合法地址时返回 `AUTH_AUTHLIB_SERVER_INVALID`。
pub async fn site_url(http: &HttpClient, api_url: &str, ctx: &TaskCtx) -> AppResult<String> {
    let fallback = origin(api_url)?;
    match resolve_server(http, api_url, ctx).await {
        Ok(server) => Ok(homepage(&server.metadata).unwrap_or(fallback)),
        Err(err) if err.is_cancelled() => Err(err),
        Err(err) => {
            tracing::warn!(error = %err, "读取皮肤站元数据失败，改用站点根地址");
            Ok(fallback)
        }
    }
}

/// 元数据中的 `meta.links.homepage`，须为可安全交给系统浏览器的地址。
pub fn homepage(metadata: &str) -> Option<String> {
    let meta: Value = serde_json::from_str(metadata).ok()?;
    let link = meta["meta"]["links"]["homepage"].as_str()?;
    is_safe_web_url(link).then(|| link.to_owned())
}

/// API 地址的站点根 `<scheme>://<host>[:port]/`。
fn origin(api_url: &str) -> AppResult<String> {
    let invalid = || AppError::new(ErrorCode::AuthAuthlibServerInvalid).with_param("url", api_url);
    let url = Url::parse(api_url).map_err(|_| invalid())?;
    let origin = url.origin().ascii_serialization();
    let root = format!("{origin}/");
    if is_safe_web_url(&root) {
        Ok(root)
    } else {
        Err(invalid())
    }
}

/// `http(s)://` 开头、带主机名，且只含不会被系统打开命令误解析的字符。
pub fn is_safe_web_url(link: &str) -> bool {
    let parsed = Url::parse(link);
    let web = parsed
        .as_ref()
        .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host().is_some());
    web && link
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-._~/:?=%#+@".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tmcl_net::testing::TestServer;

    #[test]
    fn homepage_is_read_from_metadata() {
        let meta = r#"{"meta":{"links":{"homepage":"https://skin.example.com/user"}}}"#;
        assert_eq!(
            homepage(meta).as_deref(),
            Some("https://skin.example.com/user")
        );
        for bad in [
            r#"{"meta":{"links":{"homepage":"javascript:alert(1)"}}}"#,
            r#"{"meta":{"links":{"homepage":"https://a.test/x&calc"}}}"#,
            r#"{"meta":{}}"#,
            "not json",
        ] {
            assert_eq!(homepage(bad), None, "{bad}");
        }
    }

    #[test]
    fn origin_of_api_url() {
        assert_eq!(
            origin("https://littleskin.cn/api/yggdrasil").unwrap(),
            "https://littleskin.cn/"
        );
        assert_eq!(
            origin("http://127.0.0.1:8080/api").unwrap(),
            "http://127.0.0.1:8080/"
        );
        assert_eq!(
            origin("file:///etc/passwd").unwrap_err().code,
            ErrorCode::AuthAuthlibServerInvalid
        );
    }

    #[tokio::test]
    async fn site_url_prefers_homepage_and_falls_back_to_origin() {
        let meta = serde_json::json!({
            "meta": { "serverName": "Test", "links": { "homepage": "https://skin.example.com/" } },
            "skinDomains": [],
        });
        let server = TestServer::start([("/api", meta.to_string().into_bytes())]).await;
        let http = HttpClient::new().unwrap();
        let ctx = TaskCtx::noop("skin");
        let url = site_url(&http, &server.url("/api"), &ctx).await.unwrap();
        assert_eq!(url, "https://skin.example.com/");
        let url = site_url(&http, &server.url("/missing"), &ctx)
            .await
            .unwrap();
        assert_eq!(url, server.url("/"));
    }
}
