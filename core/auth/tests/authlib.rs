//! 外置登录的离线测试：本地模拟一个 Yggdrasil 皮肤站（含 ALI 响应头与 POST 接口）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tmcl_auth::accounts::{self, AccountKind};
use tmcl_auth::authlib::{self, jvm_args};
use tmcl_auth::yggdrasil::{Profile, Session, Yggdrasil, resolve_server};
use tmcl_auth::{MemorySecretStore, SecretStore};
use tmcl_common::{AppResult, Db, ErrorCode, TaskCtx};
use tmcl_net::HttpClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const PROFILE_ID: &str = "0b6f4c2a9d3e4f8a8b1c2d3e4f5a6b7c";

/// 一条预设响应：状态码、额外响应头、响应体。
#[derive(Clone)]
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn reply(status: u16, body: impl Into<Vec<u8>>) -> Reply {
    Reply {
        status,
        headers: vec![],
        body: body.into(),
    }
}

#[derive(Default)]
struct State {
    /// `"<METHOD> <path>"` → 依次使用的响应，最后一个重复使用。
    routes: HashMap<String, Vec<Reply>>,
    /// 收到的 POST：路径与 JSON 请求体。
    posts: Vec<(String, Value)>,
}

struct Site {
    base: String,
    state: Arc<Mutex<State>>,
}

impl Site {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let state = shared.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let head_end = loop {
                        let n = socket.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            break i + 4;
                        }
                    };
                    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                    let mut lines = head.lines();
                    let mut first = lines.next().unwrap_or_default().split(' ');
                    let method = first.next().unwrap_or_default().to_owned();
                    let path = first.next().unwrap_or_default().to_owned();
                    let length: usize = lines
                        .filter_map(|l| l.split_once(':'))
                        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                        .and_then(|(_, v)| v.trim().parse().ok())
                        .unwrap_or(0);
                    while buf.len() < head_end + length {
                        let n = socket.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    let reply = {
                        let mut s = state.lock().unwrap();
                        if method == "POST" {
                            let body =
                                serde_json::from_slice(&buf[head_end..]).unwrap_or(Value::Null);
                            s.posts.push((path.clone(), body));
                        }
                        match s.routes.get_mut(&format!("{method} {path}")) {
                            Some(list) if list.len() > 1 => list.remove(0),
                            Some(list) => list[0].clone(),
                            None => reply(404, ""),
                        }
                    };
                    let mut out = format!(
                        "HTTP/1.1 {} X\r\ncontent-length: {}\r\ncontent-type: application/json\r\nconnection: close\r\n",
                        reply.status,
                        reply.body.len()
                    );
                    for (k, v) in &reply.headers {
                        out.push_str(&format!("{k}: {v}\r\n"));
                    }
                    out.push_str("\r\n");
                    let _ = socket.write_all(out.as_bytes()).await;
                    let _ = socket.write_all(&reply.body).await;
                });
            }
        });
        Self { base, state }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn on(&self, method: &str, path: &str, replies: Vec<Reply>) {
        self.state
            .lock()
            .unwrap()
            .routes
            .insert(format!("{method} {path}"), replies);
    }

    fn posts(&self) -> Vec<(String, Value)> {
        self.state.lock().unwrap().posts.clone()
    }

    /// 首页经 ALI 指向 `/api/yggdrasil`，API 根返回元数据。
    fn serve_metadata(&self) {
        self.on(
            "GET",
            "/",
            vec![Reply {
                status: 200,
                headers: vec![(
                    "X-Authlib-Injector-API-Location".into(),
                    "/api/yggdrasil/".into(),
                )],
                body: b"<html></html>".to_vec(),
            }],
        );
        self.on("GET", "/api/yggdrasil/", vec![reply(200, metadata())]);
    }
}

fn metadata() -> String {
    json!({
        "meta": { "serverName": "Test Skin", "implementationName": "test" },
        "skinDomains": ["127.0.0.1"],
        "signaturePublickey": "-----BEGIN PUBLIC KEY-----\n-----END PUBLIC KEY-----\n"
    })
    .to_string()
}

fn profile_json() -> Value {
    json!({ "id": PROFILE_ID, "name": "Steve" })
}

fn ctx() -> TaskCtx {
    TaskCtx::noop("test")
}

fn clients() -> (HttpClient, Yggdrasil) {
    (HttpClient::new().unwrap(), Yggdrasil::new().unwrap())
}

#[tokio::test]
async fn resolves_api_location_and_name() {
    let site = Site::start().await;
    site.serve_metadata();
    let (http, _) = clients();
    let server = resolve_server(&http, &site.url("/"), &ctx()).await.unwrap();
    assert_eq!(server.api_url, site.url("/api/yggdrasil"));
    assert_eq!(server.name, "Test Skin");
    assert_eq!(server.metadata, metadata());
}

#[tokio::test]
async fn rejects_non_yggdrasil_site() {
    let site = Site::start().await;
    site.on("GET", "/", vec![reply(200, "<html>not a skin site</html>")]);
    let (http, _) = clients();
    let err = resolve_server(&http, &site.url("/"), &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthAuthlibServerInvalid);
}

#[tokio::test]
async fn login_saves_account_and_token() -> AppResult<()> {
    let site = Site::start().await;
    site.serve_metadata();
    site.on(
        "POST",
        "/api/yggdrasil/authserver/authenticate",
        vec![reply(
            200,
            json!({
                "accessToken": "test-access",
                "clientToken": "test-client",
                "availableProfiles": [profile_json()],
                "selectedProfile": profile_json(),
            })
            .to_string(),
        )],
    );
    let (http, ygg) = clients();
    let db = Db::open_in_memory()?;
    let secrets = MemorySecretStore::default();
    let account = authlib::login(
        &db,
        &secrets,
        &http,
        &ygg,
        authlib::LoginRequest {
            server: &site.url("/"),
            username: "steve@example.com",
            password: "test-password",
        },
        &ctx(),
    )
    .await?;
    assert_eq!(account.kind, AccountKind::Authlib);
    assert_eq!(account.name, "Steve");
    assert_eq!(account.uuid, "0b6f4c2a-9d3e-4f8a-8b1c-2d3e4f5a6b7c");
    assert_eq!(account.server.as_ref().unwrap().name, "Test Skin");
    assert_eq!(accounts::list(&db)?.default_id, Some(account.id.clone()));

    let posts = site.posts();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].1["username"], "steve@example.com");
    assert_eq!(posts[0].1["agent"]["name"], "Minecraft");

    let stored: Session =
        serde_json::from_str(&accounts::secret(&secrets, &account.id)?.unwrap()).unwrap();
    assert_eq!(stored.access_token, "test-access");
    assert_eq!(stored.profile.name, "Steve");
    Ok(())
}

#[tokio::test]
async fn binds_first_profile_when_none_selected() {
    let site = Site::start().await;
    site.on(
        "POST",
        "/authserver/authenticate",
        vec![reply(
            200,
            json!({
                "accessToken": "unbound",
                "clientToken": "c",
                "availableProfiles": [profile_json(), { "id": "1".repeat(32), "name": "Alex" }],
            })
            .to_string(),
        )],
    );
    site.on(
        "POST",
        "/authserver/refresh",
        vec![reply(
            200,
            json!({ "accessToken": "bound", "clientToken": "c", "selectedProfile": profile_json() })
                .to_string(),
        )],
    );
    let (_, ygg) = clients();
    let session = ygg
        .authenticate(&site.url(""), "u", "p", &ctx())
        .await
        .unwrap();
    assert_eq!(session.access_token, "bound");
    assert_eq!(session.profile.name, "Steve");
    let posts = site.posts();
    assert_eq!(posts[1].0, "/authserver/refresh");
    assert_eq!(posts[1].1["selectedProfile"]["id"], PROFILE_ID);
}

#[tokio::test]
async fn maps_login_errors() {
    let site = Site::start().await;
    let forbidden =
        json!({ "error": "ForbiddenOperationException", "errorMessage": "Invalid credentials." });
    site.on(
        "POST",
        "/authserver/authenticate",
        vec![
            reply(403, forbidden.to_string()),
            reply(
                200,
                json!({ "accessToken": "a", "availableProfiles": [] }).to_string(),
            ),
            reply(503, ""),
        ],
    );
    let (_, ygg) = clients();
    let api = site.url("");
    let codes: Vec<ErrorCode> = [
        ygg.authenticate(&api, "u", "wrong", &ctx()).await,
        ygg.authenticate(&api, "u", "p", &ctx()).await,
        ygg.authenticate(&api, "u", "p", &ctx()).await,
    ]
    .into_iter()
    .map(|r| r.unwrap_err().code)
    .collect();
    assert_eq!(
        codes,
        vec![
            ErrorCode::AuthYggdrasilInvalidCredentials,
            ErrorCode::AuthYggdrasilNoProfile,
            ErrorCode::NetHttpStatus,
        ]
    );
}

fn stored_session(secrets: &MemorySecretStore, db: &Db, api: &str) -> String {
    let session = Session {
        access_token: "old".into(),
        client_token: "c".into(),
        profile: Profile {
            id: PROFILE_ID.into(),
            name: "Steve".into(),
        },
    };
    let account = accounts::upsert(
        db,
        secrets,
        accounts::NewAccount {
            kind: AccountKind::Authlib,
            name: "Steve".into(),
            uuid: uuid::Uuid::parse_str(PROFILE_ID).unwrap(),
            server: Some(tmcl_auth::AuthlibServer {
                api_url: api.into(),
                name: "Test Skin".into(),
            }),
        },
        Some(&serde_json::to_string(&session).unwrap()),
    )
    .unwrap();
    account.id
}

#[tokio::test]
async fn prepare_launch_refreshes_expired_token() -> AppResult<()> {
    let site = Site::start().await;
    site.serve_metadata();
    let api = site.url("/api/yggdrasil");
    site.on(
        "POST",
        "/api/yggdrasil/authserver/validate",
        vec![reply(403, "")],
    );
    site.on(
        "POST",
        "/api/yggdrasil/authserver/refresh",
        vec![reply(
            200,
            json!({ "accessToken": "new", "clientToken": "c", "selectedProfile": profile_json() })
                .to_string(),
        )],
    );
    // API 根本身也能直接访问（启动时用已保存的 API 地址）。
    site.on("GET", "/api/yggdrasil", vec![reply(200, metadata())]);
    let (http, ygg) = clients();
    let db = Db::open_in_memory()?;
    let secrets = MemorySecretStore::default();
    let id = stored_session(&secrets, &db, &api);
    let account = accounts::get(&db, &id)?.unwrap();
    let launch = authlib::prepare_launch(&secrets, &http, &ygg, &account, &ctx()).await?;
    assert_eq!(launch.session.access_token, "new");
    assert_eq!(launch.metadata, metadata());
    let stored: Session =
        serde_json::from_str(&secrets.get(&format!("tmcl:account:{id}"))?.unwrap()).unwrap();
    assert_eq!(stored.access_token, "new");
    Ok(())
}

#[tokio::test]
async fn prepare_launch_reports_expired_session() -> AppResult<()> {
    let site = Site::start().await;
    let api = site.url("/api/yggdrasil");
    site.on(
        "POST",
        "/api/yggdrasil/authserver/validate",
        vec![reply(403, "")],
    );
    site.on(
        "POST",
        "/api/yggdrasil/authserver/refresh",
        vec![reply(
            403,
            json!({ "error": "ForbiddenOperationException" }).to_string(),
        )],
    );
    let (http, ygg) = clients();
    let db = Db::open_in_memory()?;
    let secrets = MemorySecretStore::default();
    let id = stored_session(&secrets, &db, &api);
    let account = accounts::get(&db, &id)?.unwrap();
    let err = authlib::prepare_launch(&secrets, &http, &ygg, &account, &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthYggdrasilSessionExpired);
    assert_eq!(err.params["name"], "Steve");
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[tokio::test]
async fn downloads_and_verifies_injector() {
    let site = Site::start().await;
    let jar = b"fake jar for tests".to_vec();
    let index = |sum: &str| {
        json!({
            "build_number": 1,
            "version": "1.2.5",
            "download_url": site.url("/authlib-injector-1.2.5.jar"),
            "checksums": { "sha256": sum },
        })
        .to_string()
    };
    site.on("GET", "/bad.json", vec![reply(200, index(&"0".repeat(64)))]);
    site.on(
        "GET",
        "/latest.json",
        vec![reply(200, index(&sha256(&jar)))],
    );
    site.on(
        "GET",
        "/authlib-injector-1.2.5.jar",
        vec![reply(200, jar.clone())],
    );
    let dir = tempfile::tempdir().unwrap();
    let (http, _) = clients();

    // 第一个索引的校验和不对，回退到第二个。
    let bad = site.url("/bad.json");
    let good = site.url("/latest.json");
    let path = authlib::ensure_injector(&http, dir.path(), &[&bad, &good], &ctx())
        .await
        .unwrap();
    assert_eq!(path, dir.path().join("authlib-injector-1.2.5.jar"));
    assert_eq!(std::fs::read(&path).unwrap(), jar);

    // 索引都不可用时使用本地已有版本。
    let missing = site.url("/missing.json");
    let again = authlib::ensure_injector(&http, dir.path(), &[&missing], &ctx())
        .await
        .unwrap();
    assert_eq!(again, path);

    let empty = tempfile::tempdir().unwrap();
    let err = authlib::ensure_injector(&http, empty.path(), &[&bad], &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHashMismatch);

    let args = jvm_args(&path, "https://a.test/api", "{}");
    assert!(args[0].starts_with("-javaagent:") && args[0].ends_with("=https://a.test/api"));
}

/// 联网：读取 LittleSkin 的元数据（只读，不登录）。`cargo test -p tmcl-auth -- --ignored online_`
#[tokio::test]
#[ignore]
async fn online_littleskin_metadata() {
    let (http, _) = clients();
    let server = resolve_server(&http, "littleskin.cn", &ctx())
        .await
        .unwrap();
    assert_eq!(server.api_url, "https://littleskin.cn/api/yggdrasil");
    assert!(!server.name.is_empty());
}

/// 联网：从官方索引下载 authlib-injector 并校验。
#[tokio::test]
#[ignore]
async fn online_injector_download() {
    let (http, _) = clients();
    let dir = tempfile::tempdir().unwrap();
    let path = authlib::ensure_injector(
        &http,
        dir.path(),
        &[authlib::ARTIFACT_URL, authlib::ARTIFACT_MIRROR_URL],
        &ctx(),
    )
    .await
    .unwrap();
    assert!(std::fs::metadata(path).unwrap().len() > 100_000);
}
