//! 离线测试：本地服务器模拟 `api.natfrp.com/v4` 与 frpc 下载地址，Unix 上用 shell 脚本模拟 frpc。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use md5::{Digest, Md5};
use tmcl_auth::{MemorySecretStore, SecretStore};
use tmcl_common::{ErrorCode, TaskCtx};
use tmcl_net::HttpClient;
use tmcl_sakura::tunnel::{self, KEY_SECRET};
use tmcl_sakura::{SakuraClient, Session, StartRequest, ensure_frpc, frpc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 服务器收到的请求。
#[derive(Debug, Clone)]
struct Request {
    method: String,
    path: String,
    auth: String,
    body: String,
}

type Routes = Arc<Mutex<HashMap<String, (u16, Vec<u8>)>>>;
type Requests = Arc<Mutex<Vec<Request>>>;

struct Server {
    base: String,
    routes: Routes,
    requests: Requests,
}

impl Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes = Routes::default();
        let requests = Requests::default();
        let (r, q) = (routes.clone(), requests.clone());
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(serve(socket, r.clone(), q.clone()));
            }
        });
        Self {
            base,
            routes,
            requests,
        }
    }

    /// 路由键为 `<方法> <路径>`。
    fn route(&self, key: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.routes
            .lock()
            .unwrap()
            .insert(key.to_owned(), (status, body.into()));
    }

    fn requests(&self, key: &str) -> Vec<Request> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| format!("{} {}", r.method, r.path) == key)
            .cloned()
            .collect()
    }

    fn api(&self) -> SakuraClient {
        SakuraClient::with_base(&self.base).unwrap()
    }
}

async fn serve(mut socket: tokio::net::TcpStream, routes: Routes, requests: Requests) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        let n = socket.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let header = |name: &str| {
        head.lines()
            .filter_map(|l| l.split_once(':'))
            .find(|(n, _)| n.trim().eq_ignore_ascii_case(name))
            .map(|(_, v)| v.trim().to_owned())
            .unwrap_or_default()
    };
    let length: usize = header("content-length").parse().unwrap_or(0);
    let mut body = buf[head_end..].to_vec();
    while body.len() < length {
        let n = socket.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let mut start = head.split_whitespace();
    let request = Request {
        method: start.next().unwrap_or_default().to_owned(),
        path: start.next().unwrap_or_default().to_owned(),
        auth: header("authorization"),
        body: String::from_utf8_lossy(&body).into_owned(),
    };
    let key = format!("{} {}", request.method, request.path);
    requests.lock().unwrap().push(request);
    let (status, payload) = routes
        .lock()
        .unwrap()
        .get(&key)
        .cloned()
        .unwrap_or((404, br#"{"code":404,"msg":"not found"}"#.to_vec()));
    let head = format!(
        "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    let _ = socket.write_all(head.as_bytes()).await;
    let _ = socket.write_all(&payload).await;
    let _ = socket.shutdown().await;
}

const USER: &str = r#"{"id":10,"name":"DemoUser","avatar":"","token":"x","speed":"10 Mbps","tunnels":2,
"realname":1,"group":{"name":"普通用户","level":0,"expires":0},"traffic":[0,1048576],
"sign":{"config":false,"signed":false,"last":"","days":0,"traffic":0}}"#;
const NODES: &str = r#"{"5":{"name":"某地 BGP","host":"a.natfrp.test","description":"","vip":0,"flag":4},
"7":{"name":"某地多线","host":"b.natfrp.test","description":"","vip":0,"flag":4},
"9":{"name":"离线节点","host":"c.natfrp.test","description":"","vip":0,"flag":516}}"#;
/// 隧道 1 是上次残留的临时隧道，隧道 2 是用户自己的。
#[cfg(unix)]
const TUNNELS: &str = r#"[{"id":1,"name":"old","type":"tcp","node":5,"online":false,"status":0,"status_reason":"",
"note":"TMCL temporary tunnel","extra":"","remote":"","local_ip":"127.0.0.1","local_port":1,"export":""},
{"id":2,"name":"mine","type":"tcp","node":5,"online":false,"status":0,"status_reason":"",
"note":"","extra":"","remote":"","local_ip":"127.0.0.1","local_port":22,"export":""}]"#;

fn ctx() -> TaskCtx {
    TaskCtx::noop("sakura")
}

fn store_with_key() -> MemorySecretStore {
    let store = MemorySecretStore::default();
    store.set(KEY_SECRET, "test-key").unwrap();
    store
}

#[tokio::test]
async fn save_key_validates_before_storing() {
    let server = Server::start().await;
    let store = MemorySecretStore::default();
    server.route(
        "GET /user/info",
        401,
        r#"{"code":401,"msg":"访问密钥无效"}"#,
    );
    let err = tunnel::save_key(&store, &server.api(), "bad-key", &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::SakuraKeyInvalid);
    assert!(store.keys().is_empty());

    server.route("GET /user/info", 200, USER);
    let user = tunnel::save_key(&store, &server.api(), " test-key \n", &ctx())
        .await
        .unwrap();
    assert_eq!(user.name, "DemoUser");
    assert!(user.verified);
    assert_eq!(user.traffic_left, 1_048_576);
    assert_eq!(tunnel::load_key(&store).unwrap(), "test-key");
    assert_eq!(server.requests("GET /user/info")[1].auth, "Bearer test-key");
}

#[tokio::test]
async fn banned_and_api_errors_are_mapped() {
    let server = Server::start().await;
    server.route(
        "GET /user/info",
        200,
        r#"{"id":10,"name":"x","avatar":"","ban":{"title":"冻结","reason":"r","expires":0}}"#,
    );
    let err = server.api().user_info("k", &ctx()).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::SakuraAccountBanned);

    server.route(
        "POST /tunnel/delete",
        403,
        r#"{"code":403,"msg":"无权访问"}"#,
    );
    let err = server
        .api()
        .delete_tunnels("k", &[1], &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::SakuraApiError);
    assert_eq!(err.params["msg"], "无权访问");
}

#[tokio::test]
async fn delete_is_batched_by_ten() {
    let server = Server::start().await;
    server.route("POST /tunnel/delete", 200, r#"{"deleted":[],"failed":[]}"#);
    let ids: Vec<u64> = (1..=12).collect();
    server
        .api()
        .delete_tunnels("k", &ids, &ctx())
        .await
        .unwrap();
    let bodies: Vec<String> = server
        .requests("POST /tunnel/delete")
        .into_iter()
        .map(|r| r.body)
        .collect();
    assert_eq!(
        bodies,
        vec![
            r#"{"ids":"1,2,3,4,5,6,7,8,9,10"}"#.to_owned(),
            r#"{"ids":"11,12"}"#.to_owned()
        ]
    );
}

#[tokio::test]
async fn start_checks_account_and_node() {
    let server = Server::start().await;
    let store = store_with_key();
    server.route("GET /nodes", 200, NODES);
    let request = |node, public| StartRequest {
        local_port: 25565,
        node,
        public,
    };
    let start = |req: StartRequest| {
        let api = server.api();
        let store = &store;
        async move {
            Session::start(
                &api,
                store,
                Path::new("frpc"),
                &req,
                Duration::from_secs(1),
                &ctx(),
            )
            .await
            .unwrap_err()
            .code
        }
    };

    server.route(
        "GET /user/info",
        200,
        USER.replace("\"realname\":1", "\"realname\":0"),
    );
    assert_eq!(
        start(request(5, false)).await,
        ErrorCode::SakuraRealnameRequired
    );

    server.route("GET /user/info", 200, USER);
    assert_eq!(
        start(request(9, false)).await,
        ErrorCode::SakuraNodeUnavailable
    );
    assert_eq!(
        start(request(404, false)).await,
        ErrorCode::SakuraNodeUnavailable
    );
    assert_eq!(
        start(request(5, true)).await,
        ErrorCode::SakuraNodeNotPublic
    );
    assert!(server.requests("POST /tunnels").is_empty());

    let missing = MemorySecretStore::default();
    let err = Session::start(
        &server.api(),
        &missing,
        Path::new("frpc"),
        &request(5, false),
        Duration::from_secs(1),
        &ctx(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::SakuraKeyMissing);
}

fn md5_hex(bytes: &[u8]) -> String {
    Md5::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn clients_json(server: &Server, hash: &str, size: usize) -> String {
    let key = frpc::platform_key().unwrap();
    format!(
        r#"{{"windows":{{"ver":"3.2.0.2","time":1,"archs":{{"default":{{"title":"t","url":"https://x.test/a.exe","hash":"00","size":1}}}}}},
"frpc":{{"ver":"0.51.0-sakura-15.1","time":1,"note":"","archs":{{
"{key}":{{"title":"frpc","url":"{}/download/frpc","hash":"{hash}","size":{size}}},
"docker_hub":{{"title":"Docker","url":"https://hub.docker.test/"}}}}}}}}"#,
        server.base
    )
}

#[tokio::test]
async fn ensure_frpc_downloads_and_verifies() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let http = HttpClient::new().unwrap();
    let binary = b"fake frpc binary".to_vec();
    server.route("GET /download/frpc", 200, binary.clone());

    // 哈希不符：报错且不留下文件。
    server.route(
        "GET /system/clients",
        200,
        clients_json(&server, &"0".repeat(32), binary.len()),
    );
    let err = ensure_frpc(&server.api(), &http, dir.path(), &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHashMismatch);
    assert!(
        !dir.path()
            .join("0.51.0-sakura-15.1")
            .join(frpc::exe_name())
            .exists()
    );

    // 旧版本目录在成功后被清理。
    std::fs::create_dir_all(dir.path().join("0.51.0-sakura-14.0")).unwrap();
    server.route(
        "GET /system/clients",
        200,
        clients_json(&server, &md5_hex(&binary), binary.len()),
    );
    let path = ensure_frpc(&server.api(), &http, dir.path(), &ctx())
        .await
        .unwrap();
    assert_eq!(
        path,
        dir.path().join("0.51.0-sakura-15.1").join(frpc::exe_name())
    );
    assert_eq!(std::fs::read(&path).unwrap(), binary);
    assert!(!dir.path().join("0.51.0-sakura-14.0").exists());

    // 已存在且校验通过时不再下载。
    ensure_frpc(&server.api(), &http, dir.path(), &ctx())
        .await
        .unwrap();
    assert_eq!(server.requests("GET /download/frpc").len(), 2);

    // 下载信息取不到时使用本地版本。
    server.route("GET /system/clients", 500, "{}");
    assert_eq!(
        ensure_frpc(&server.api(), &http, dir.path(), &ctx())
            .await
            .unwrap(),
        path
    );
}

/// 写一个模拟 frpc 的脚本：把参数写入 `args.txt`，再执行 `body`。
#[cfg(unix)]
fn fake_frpc(dir: &Path, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("frpc");
    let args = dir.join("args.txt");
    std::fs::write(
        &path,
        format!("#!/bin/sh\necho \"$@\" > '{}'\n{body}\n", args.display()),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(unix)]
fn session_server_routes(server: &Server) {
    server.route("GET /user/info", 200, USER);
    server.route("GET /nodes", 200, NODES);
    server.route("GET /tunnels", 200, TUNNELS);
    server.route("POST /tunnels", 201, r#"{"id":114514,"name":"tmcl"}"#);
    server.route("POST /tunnel/delete", 200, r#"{"deleted":[],"failed":[]}"#);
}

#[cfg(unix)]
#[tokio::test]
async fn session_creates_tunnel_runs_frpc_and_cleans_up() {
    let server = Server::start().await;
    session_server_routes(&server);
    let dir = tempfile::tempdir().unwrap();
    let exe = fake_frpc(
        dir.path(),
        "echo '2026/10/30 [I] [sakura] 启动成功'\n\
         echo '2026/10/30 [I] 使用 >>frp-abc.natfrp.test:10086<< 连接你的隧道'\n\
         echo '2026/10/30 [I] 或使用 IP 地址连接: >>1.2.3.4:10086<<'\n\
         sleep 30",
    );
    let store = store_with_key();
    let request = StartRequest {
        local_port: 25565,
        node: 7,
        public: true,
    };
    let mut session = Session::start(
        &server.api(),
        &store,
        &exe,
        &request,
        Duration::from_secs(10),
        &ctx(),
    )
    .await
    .unwrap();
    assert_eq!(session.room.tunnel_id, 114514);
    assert_eq!(session.room.address, "frp-abc.natfrp.test:10086");
    assert_eq!(session.room.ip_address.as_deref(), Some("1.2.3.4:10086"));
    assert!(session.is_running());

    let args = std::fs::read_to_string(dir.path().join("args.txt")).unwrap();
    assert_eq!(args.trim(), "-f test-key:114514 -n --disable_log_color");

    // 只清理不在线的临时隧道（1），用户自己的隧道（2）不动。
    let deletes = server.requests("POST /tunnel/delete");
    assert_eq!(deletes.len(), 1);
    assert_eq!(deletes[0].body, r#"{"ids":"1"}"#);

    let created: serde_json::Value =
        serde_json::from_str(&server.requests("POST /tunnels")[0].body).unwrap();
    assert_eq!(created["type"], "tcp");
    assert_eq!(created["node"], 7);
    assert_eq!(created["local_ip"], "127.0.0.1");
    assert_eq!(created["local_port"], 25565);
    assert_eq!(created["note"], tunnel::TEMP_NOTE);

    session.stop(&server.api(), &ctx()).await.unwrap();
    let deletes = server.requests("POST /tunnel/delete");
    assert_eq!(deletes.last().unwrap().body, r#"{"ids":"114514"}"#);
}

#[cfg(unix)]
#[tokio::test]
async fn failed_frpc_deletes_the_new_tunnel() {
    let server = Server::start().await;
    session_server_routes(&server);
    let dir = tempfile::tempdir().unwrap();
    let exe = fake_frpc(
        dir.path(),
        "echo '2026/10/30 [E] login to server failed: test-key is wrong'\nexit 1",
    );
    let store = store_with_key();
    let request = StartRequest {
        local_port: 25565,
        node: 5,
        public: false,
    };
    let err = Session::start(
        &server.api(),
        &store,
        &exe,
        &request,
        Duration::from_secs(10),
        &ctx(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::SakuraFrpcFailed);
    let detail = err.detail.unwrap();
    assert!(detail.contains("login to server failed"), "{detail}");
    assert!(!detail.contains("test-key"), "密钥应被替换：{detail}");
    let deletes = server.requests("POST /tunnel/delete");
    assert_eq!(deletes.last().unwrap().body, r#"{"ids":"114514"}"#);
}
