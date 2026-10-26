//! `MojangClient` 的离线测试：本地服务器模拟 `api.minecraftservices.com`，检查请求方法、
//! 路径、令牌与 multipart 请求体，以及错误码映射。

use std::sync::{Arc, Mutex};

use tmcl_common::{ErrorCode, TaskCtx};
use tmcl_skin::{MojangClient, SkinVariant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 服务器收到的请求：方法、路径、`Authorization` 头、`Content-Type` 头与请求体。
#[derive(Debug, Clone)]
struct Request {
    method: String,
    path: String,
    auth: String,
    content_type: String,
    body: Vec<u8>,
}

type Requests = Arc<Mutex<Vec<Request>>>;

/// 启动服务器：每个请求都返回 `(status, body)`。
async fn server(status: u16, body: &'static str) -> (String, Requests) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Requests::default();
    let recorded = requests.clone();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let recorded = recorded.clone();
            tokio::spawn(async move {
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
                let mut req_body = buf[head_end..].to_vec();
                while req_body.len() < length {
                    let n = socket.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    req_body.extend_from_slice(&chunk[..n]);
                }
                let mut start = head.split_whitespace();
                recorded.lock().unwrap().push(Request {
                    method: start.next().unwrap_or_default().to_owned(),
                    path: start.next().unwrap_or_default().to_owned(),
                    auth: header("authorization"),
                    content_type: header("content-type"),
                    body: req_body,
                });
                let out = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(out.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (base, requests)
}

/// 去掉令牌与真实 UUID 的档案响应。
const PROFILE: &str = r#"{
  "id": "0b6f4c2a9d3e4f8a8b1c2d3e4f5a6b7c",
  "name": "Steve",
  "skins": [
    { "id": "s1", "state": "ACTIVE", "url": "http://textures.minecraft.net/texture/aa",
      "textureKey": "aa", "variant": "SLIM" },
    { "id": "s2", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/bb",
      "textureKey": "bb", "variant": "CLASSIC" }
  ],
  "capes": [
    { "id": "c1", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/cc",
      "alias": "Migrator" }
  ],
  "profileActions": {}
}"#;

fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&64u32.to_be_bytes());
    bytes.extend_from_slice(&64u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

fn ctx() -> TaskCtx {
    TaskCtx::noop("skin")
}

#[tokio::test]
async fn reads_profile_skins_and_capes() {
    let (base, requests) = server(200, PROFILE).await;
    let client = MojangClient::with_base(&base).unwrap();
    let profile = client.profile("test-mc", &ctx()).await.unwrap();
    assert_eq!(profile.name, "Steve");
    assert_eq!(profile.skins.len(), 2);
    assert!(profile.skins[0].active);
    assert_eq!(profile.skins[0].variant, SkinVariant::Slim);
    assert_eq!(profile.skins[1].variant, SkinVariant::Classic);
    assert_eq!(profile.capes[0].alias, "Migrator");
    assert!(!profile.capes[0].active);

    let req = &requests.lock().unwrap()[0];
    assert_eq!(
        (req.method.as_str(), req.path.as_str()),
        ("GET", "/minecraft/profile")
    );
    assert_eq!(req.auth, "Bearer test-mc");
}

#[tokio::test]
async fn uploads_skin_as_multipart() {
    let (base, requests) = server(200, PROFILE).await;
    let client = MojangClient::with_base(&base).unwrap();
    let profile = client
        .upload_skin("test-mc", SkinVariant::Slim, &png(), &ctx())
        .await
        .unwrap();
    assert_eq!(profile.skins[0].id, "s1");

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1, "响应即为档案时不再读取");
    let req = &requests[0];
    assert_eq!(
        (req.method.as_str(), req.path.as_str()),
        ("POST", "/minecraft/profile/skins")
    );
    assert_eq!(req.auth, "Bearer test-mc");
    let boundary = req
        .content_type
        .strip_prefix("multipart/form-data; boundary=")
        .unwrap();
    let body = String::from_utf8_lossy(&req.body);
    assert!(body.starts_with(&format!("--{boundary}\r\n")));
    assert!(body.contains("name=\"variant\"\r\n\r\nslim\r\n"));
    assert!(body.contains("name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png"));
    assert!(body.ends_with(&format!("--{boundary}--\r\n")));
    let start = req
        .body
        .windows(8)
        .position(|w| w == b"\x89PNG\r\n\x1a\n")
        .unwrap();
    assert_eq!(&req.body[start..start + png().len()], png().as_slice());
}

#[tokio::test]
async fn invalid_png_is_rejected_before_upload() {
    let (base, requests) = server(200, PROFILE).await;
    let client = MojangClient::with_base(&base).unwrap();
    let err = client
        .upload_skin("test-mc", SkinVariant::Classic, b"GIF89a", &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::SkinInvalidImage);
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn sets_and_hides_cape() {
    let (base, requests) = server(200, PROFILE).await;
    let client = MojangClient::with_base(&base).unwrap();
    client
        .set_cape("test-mc", Some("c1"), &ctx())
        .await
        .unwrap();
    client.set_cape("test-mc", None, &ctx()).await.unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests[0].method, "PUT");
    assert_eq!(requests[0].path, "/minecraft/profile/capes/active");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].body).unwrap(),
        serde_json::json!({ "capeId": "c1" })
    );
    assert_eq!(requests[1].method, "DELETE");
    assert_eq!(requests[1].path, "/minecraft/profile/capes/active");
    assert!(requests.iter().all(|r| r.auth == "Bearer test-mc"));
}

#[tokio::test]
async fn non_profile_response_triggers_refetch() {
    let (base, requests) = server(200, "{}").await;
    let client = MojangClient::with_base(&base).unwrap();
    let err = client.set_cape("test-mc", None, &ctx()).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::CommonJson);
    let paths: Vec<_> = requests
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.method.clone())
        .collect();
    assert_eq!(paths, ["DELETE", "GET"]);
}

#[tokio::test]
async fn maps_error_statuses() {
    let cases = [
        (401, ErrorCode::AuthMsSessionExpired),
        (404, ErrorCode::AuthMcNoProfile),
        (500, ErrorCode::NetHttpStatus),
    ];
    for (status, code) in cases {
        let (base, _) = server(status, "").await;
        let client = MojangClient::with_base(&base).unwrap();
        let err = client.profile("test-mc", &ctx()).await.unwrap_err();
        assert_eq!(err.code, code, "{status}");
        assert!(
            !format!("{err:?}").contains("test-mc"),
            "错误中不应出现令牌"
        );
    }
    let (base, _) = server(400, r#"{"errorMessage":"bad skin"}"#).await;
    let client = MojangClient::with_base(&base).unwrap();
    let err = client
        .upload_skin("test-mc", SkinVariant::Classic, &png(), &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::SkinInvalidImage);
    let (base, _) = server(404, "").await;
    let client = MojangClient::with_base(&base).unwrap();
    let err = client
        .set_cape("test-mc", Some("x"), &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
}

#[tokio::test]
async fn cancelled_request_returns_cancelled() {
    let ctx = ctx();
    ctx.cancel.cancel();
    let client = MojangClient::with_base("http://127.0.0.1:9").unwrap();
    let err = client.profile("test-mc", &ctx).await.unwrap_err();
    assert!(err.is_cancelled());
}
