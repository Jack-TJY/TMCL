//! 微软登录全链路的离线测试：本地服务器按 `tests/fixtures/microsoft/` 中的录制响应
//! （令牌、用户哈希、UUID 已替换为假值）模拟微软、Xbox Live 与 Minecraft 服务。

mod support;

use std::time::Duration;

use serde_json::Value;
use tmcl_auth::accounts::{self, AccountKind};
use tmcl_auth::microsoft::{Endpoints, MsClient};
use tmcl_auth::store::{self, MicrosoftSession};
use tmcl_auth::{MemorySecretStore, SecretStore};
use tmcl_common::{AppError, AppResult, Db, ErrorCode, TaskCtx};

use support::{Reply, Site, reply};

const CLIENT_ID: &str = "00000000-0000-0000-0000-000000000000";
const PROFILE_ID: &str = "0b6f4c2a9d3e4f8a8b1c2d3e4f5a6b7c";

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/microsoft/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn ok(name: &str) -> Reply {
    reply(200, fixture(name))
}

fn ctx() -> TaskCtx {
    TaskCtx::noop("test")
}

struct Mock {
    site: Site,
    ms: MsClient,
}

impl Mock {
    /// 默认每一步都成功：轮询一次 `authorization_pending` 后拿到令牌。
    async fn start() -> Self {
        let site = Site::start().await;
        let b = &site.base;
        let endpoints = Endpoints::with_bases(
            &format!("{b}/oauth"),
            &format!("{b}/xbl"),
            &format!("{b}/xsts"),
            &format!("{b}/mc"),
        );
        let ms = MsClient::new(CLIENT_ID, endpoints).unwrap();
        site.on("POST", "/oauth/devicecode", vec![ok("devicecode.json")]);
        site.on(
            "POST",
            "/oauth/token",
            vec![reply(400, fixture("token_pending.json")), ok("token.json")],
        );
        site.on("POST", "/xbl/user/authenticate", vec![ok("xbox_user.json")]);
        site.on("POST", "/xsts/xsts/authorize", vec![ok("xsts.json")]);
        site.on(
            "POST",
            "/mc/authentication/login_with_xbox",
            vec![ok("mc_login.json")],
        );
        site.on(
            "GET",
            "/mc/entitlements/mcstore",
            vec![ok("entitlements.json")],
        );
        site.on("GET", "/mc/minecraft/profile", vec![ok("profile.json")]);
        Self { site, ms }
    }

    async fn login(&self) -> AppResult<MicrosoftSession> {
        store::login(&self.ms, |_| {}, &ctx()).await
    }

    fn request(&self, path: &str) -> support::Request {
        self.site
            .requests()
            .into_iter()
            .rev()
            .find(|r| r.path == path)
            .unwrap_or_else(|| panic!("no request to {path}"))
    }
}

fn json_body(request: &support::Request) -> Value {
    serde_json::from_slice(&request.body).unwrap()
}

/// 错误的任何部分都不能带出令牌。
fn assert_no_secret(err: &AppError) {
    let text = format!("{err:?} {}", serde_json::to_string(err).unwrap());
    for secret in ["test-", "0000000000000000000", "AADSTS"] {
        assert!(!text.contains(secret), "{text}");
    }
}

#[tokio::test]
async fn full_login_saves_account_and_tokens() -> AppResult<()> {
    let mock = Mock::start().await;
    let mut shown = None;
    let session = store::login(&mock.ms, |code| shown = Some(code.clone()), &ctx()).await?;
    let code = shown.unwrap();
    assert_eq!(code.user_code, "TMCLTEST");
    assert_eq!(code.verification_uri, "https://www.microsoft.com/link");
    assert_eq!(code.expires_in, 900);
    assert!(!format!("{code:?}").contains("test-device-code"));

    assert_eq!(session.access_token, "test-mc-access-token");
    assert_eq!(session.refresh_token, "test-ms-refresh-token");
    assert_eq!(session.profile.id, PROFILE_ID);
    assert_eq!(session.profile.name, "Steve");
    assert!(session.is_fresh(0));

    // 请求内容符合各服务的协议。
    let device = mock.request("/oauth/devicecode");
    assert_eq!(
        device.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(
        device.text(),
        format!("client_id={CLIENT_ID}&scope=XboxLive.signin+offline_access")
    );
    let poll = mock.request("/oauth/token").text();
    assert!(poll.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"));
    assert!(poll.contains("device_code=test-device-code"));
    let polls = mock
        .site
        .requests()
        .iter()
        .filter(|r| r.path == "/oauth/token")
        .count();
    assert_eq!(polls, 2);
    let xbl = json_body(&mock.request("/xbl/user/authenticate"));
    assert_eq!(xbl["Properties"]["RpsTicket"], "d=test-ms-access-token");
    assert_eq!(xbl["RelyingParty"], "http://auth.xboxlive.com");
    let xsts = json_body(&mock.request("/xsts/xsts/authorize"));
    assert_eq!(xsts["Properties"]["UserTokens"][0], "test-xbl-token");
    assert_eq!(xsts["RelyingParty"], "rp://api.minecraftservices.com/");
    let login = json_body(&mock.request("/mc/authentication/login_with_xbox"));
    assert_eq!(
        login["identityToken"],
        "XBL3.0 x=0000000000000000000;test-xsts-token"
    );
    for path in ["/mc/entitlements/mcstore", "/mc/minecraft/profile"] {
        assert_eq!(
            mock.request(path).header("authorization"),
            Some("Bearer test-mc-access-token")
        );
    }

    let db = Db::open_in_memory()?;
    let secrets = MemorySecretStore::default();
    let account = store::save_session(&db, &secrets, &session)?;
    assert_eq!(account.kind, AccountKind::Microsoft);
    assert_eq!(account.name, "Steve");
    assert_eq!(account.uuid, "0b6f4c2a-9d3e-4f8a-8b1c-2d3e4f5a6b7c");
    assert_eq!(accounts::list(&db)?.default_id, Some(account.id.clone()));
    let stored: MicrosoftSession =
        serde_json::from_str(&accounts::secret(&secrets, &account.id)?.unwrap()).unwrap();
    assert_eq!(stored, session);
    // 再次登录同一个档案只更新令牌。
    let again = store::save_session(&db, &secrets, &session)?;
    assert_eq!(again.id, account.id);
    assert_eq!(accounts::list(&db)?.accounts.len(), 1);
    Ok(())
}

#[tokio::test]
async fn user_declined_or_code_expired() {
    let mock = Mock::start().await;
    mock.site.on(
        "POST",
        "/oauth/token",
        vec![reply(400, fixture("token_declined.json"))],
    );
    let err = mock.login().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthMsDeclined);
    assert_no_secret(&err);

    mock.site.on(
        "POST",
        "/oauth/token",
        vec![reply(400, fixture("token_expired.json"))],
    );
    let err = mock.login().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthMsCodeExpired);
    assert_no_secret(&err);
}

#[tokio::test]
async fn polling_can_be_cancelled() {
    let mock = Mock::start().await;
    mock.site.on(
        "POST",
        "/oauth/token",
        vec![reply(400, fixture("token_pending.json"))],
    );
    let ctx = ctx();
    let cancel = ctx.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel.cancel();
    });
    let started = std::time::Instant::now();
    let err = store::login(&mock.ms, |_| {}, &ctx).await.unwrap_err();
    assert!(err.is_cancelled());
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn xsts_errors_are_mapped() {
    let mock = Mock::start().await;
    for (file, code) in [
        ("xsts_no_xbox.json", ErrorCode::AuthXstsNoXboxAccount),
        ("xsts_child.json", ErrorCode::AuthXstsChildAccount),
    ] {
        mock.site.on(
            "POST",
            "/xsts/xsts/authorize",
            vec![reply(401, fixture(file))],
        );
        mock.site.on("POST", "/oauth/token", vec![ok("token.json")]);
        let err = mock.login().await.unwrap_err();
        assert_eq!(err.code, code, "{file}");
        assert_no_secret(&err);
    }
}

#[tokio::test]
async fn unapproved_app_is_reported() {
    let mock = Mock::start().await;
    mock.site.on(
        "POST",
        "/mc/authentication/login_with_xbox",
        vec![reply(403, fixture("mc_login_forbidden.json"))],
    );
    let err = mock.login().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthMcAppNotApproved);
    assert_no_secret(&err);
}

#[tokio::test]
async fn missing_profile_distinguishes_ownership() {
    let mock = Mock::start().await;
    mock.site.on(
        "GET",
        "/mc/minecraft/profile",
        vec![reply(404, fixture("profile_not_found.json"))],
    );
    mock.site.on("POST", "/oauth/token", vec![ok("token.json")]);
    assert_eq!(
        mock.login().await.unwrap_err().code,
        ErrorCode::AuthMcNoProfile
    );
    mock.site.on(
        "GET",
        "/mc/entitlements/mcstore",
        vec![ok("entitlements_empty.json")],
    );
    assert_eq!(
        mock.login().await.unwrap_err().code,
        ErrorCode::AuthMcNotOwned
    );
    // Game Pass：权益为空但有档案，照常登录。
    mock.site
        .on("GET", "/mc/minecraft/profile", vec![ok("profile.json")]);
    assert_eq!(mock.login().await.unwrap().profile.name, "Steve");
}

fn saved(db: &Db, secrets: &MemorySecretStore, expires_at: u64) -> AppResult<tmcl_auth::Account> {
    let session = MicrosoftSession {
        refresh_token: "test-ms-refresh-token".into(),
        access_token: "test-mc-old".into(),
        expires_at,
        profile: tmcl_auth::minecraft::McProfile {
            id: PROFILE_ID.into(),
            name: "Steve".into(),
        },
    };
    store::save_session(db, secrets, &session)
}

#[tokio::test]
async fn prepare_launch_uses_fresh_token_without_network() -> AppResult<()> {
    let mock = Mock::start().await;
    let db = Db::open_in_memory()?;
    let secrets = MemorySecretStore::default();
    let account = saved(&db, &secrets, u64::MAX)?;
    let session = store::prepare_launch(&secrets, &mock.ms, &account, &ctx()).await?;
    assert_eq!(session.access_token, "test-mc-old");
    assert!(mock.site.requests().is_empty());
    Ok(())
}

#[tokio::test]
async fn prepare_launch_refreshes_expired_token() -> AppResult<()> {
    let mock = Mock::start().await;
    mock.site
        .on("POST", "/oauth/token", vec![ok("token_refreshed.json")]);
    let db = Db::open_in_memory()?;
    let secrets = MemorySecretStore::default();
    let account = saved(&db, &secrets, 0)?;
    let session = store::prepare_launch(&secrets, &mock.ms, &account, &ctx()).await?;
    assert_eq!(session.access_token, "test-mc-access-token");
    assert_eq!(session.refresh_token, "test-ms-refresh-token-2");
    let refresh = mock.request("/oauth/token").text();
    assert!(refresh.contains("grant_type=refresh_token"));
    assert!(refresh.contains("refresh_token=test-ms-refresh-token&"));
    let stored: MicrosoftSession =
        serde_json::from_str(&accounts::secret(&secrets, &account.id)?.unwrap()).unwrap();
    assert_eq!(stored, session);
    Ok(())
}

#[tokio::test]
async fn prepare_launch_needs_relogin() -> AppResult<()> {
    let mock = Mock::start().await;
    mock.site.on(
        "POST",
        "/oauth/token",
        vec![reply(400, fixture("token_invalid_grant.json"))],
    );
    let db = Db::open_in_memory()?;
    let secrets = MemorySecretStore::default();
    let account = saved(&db, &secrets, 0)?;
    let err = store::prepare_launch(&secrets, &mock.ms, &account, &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthMsSessionExpired);
    assert_eq!(err.params["name"], "Steve");
    assert_no_secret(&err);

    // 密钥库中的令牌丢失或损坏。
    secrets.set(&tmcl_auth::secrets::secret_key(&account.id), "not json")?;
    let err = store::prepare_launch(&secrets, &mock.ms, &account, &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthMsSessionExpired);
    secrets.delete(&tmcl_auth::secrets::secret_key(&account.id))?;
    let err = store::prepare_launch(&secrets, &mock.ms, &account, &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthMsSessionExpired);
    Ok(())
}

#[tokio::test]
async fn network_errors_do_not_leak_tokens() {
    let mock = Mock::start().await;
    mock.site
        .on("POST", "/xbl/user/authenticate", vec![reply(500, "")]);
    let err = mock.login().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    assert_no_secret(&err);
}
