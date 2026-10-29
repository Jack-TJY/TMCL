//! CurseForge 客户端测试：离线部分用 `fixtures/curseforge/` 下录制的真实响应（2026-10-30，
//! 已截短长列表），联网部分标记为 `#[ignore]`，且只在带 key 的构建中运行。

#[allow(dead_code)] // 与 modrinth.rs 共用，部分辅助函数只在那边使用。
mod support;

use std::time::Duration;

use support::{MockServer, Response};
use tmcl_common::{ErrorCode, TaskCtx};
use tmcl_net::{HttpClient, RetryPolicy};
use tmcl_resources::curseforge::{
    self, ClassId, CurseForgeClient, FileFilter, ModLoaderType, RelationType, ReleaseType,
    SearchQuery, fingerprint,
};
use tmcl_resources::modrinth::USER_AGENT;

const SEARCH: &[u8] = include_bytes!("fixtures/curseforge/search.json");
const MOD: &[u8] = include_bytes!("fixtures/curseforge/mod.json");
const MOD_FILES: &[u8] = include_bytes!("fixtures/curseforge/mod_files.json");
const FILE: &[u8] = include_bytes!("fixtures/curseforge/file.json");
const FINGERPRINTS: &[u8] = include_bytes!("fixtures/curseforge/fingerprints.json");
const MODPACK_SEARCH: &[u8] = include_bytes!("fixtures/curseforge/modpack_search.json");

const KEY: &str = "test-api-key";
const JEI: u32 = 238_222;
const JEI_FILE: u32 = 9_352_806;

fn client(server: &MockServer) -> CurseForgeClient {
    CurseForgeClient::with_base(&HttpClient::new().unwrap(), &server.url("/v1"), KEY)
        .unwrap()
        .with_retry(RetryPolicy::with_delays([Duration::ZERO; 3]))
}

fn ctx() -> TaskCtx {
    TaskCtx::noop("test")
}

fn jei_fingerprint() -> u32 {
    let value: serde_json::Value = serde_json::from_slice(FILE).unwrap();
    value["data"]["fileFingerprint"].as_u64().unwrap() as u32
}

#[tokio::test]
async fn search_sends_key_user_agent_and_params() {
    let server = MockServer::start(|_, _| Response::json(SEARCH)).await;
    let query = SearchQuery {
        class_id: Some(ClassId::Mods),
        game_version: Some("1.20.1".into()),
        loader: Some(ModLoaderType::Forge),
        limit: 2,
        ..SearchQuery::new("jei")
    };
    let page = client(&server).search(&query, &ctx()).await.unwrap();
    assert_eq!(page.pagination.total_count, 201);
    let jei = &page.data[0];
    assert_eq!((jei.id, jei.slug.as_str()), (JEI, "jei"));
    assert_eq!(jei.class_id, Some(ClassId::Mods));
    assert_eq!(jei.allow_mod_distribution, Some(true));
    assert!(!jei.game_versions().is_empty());

    let request = &server.requests()[0];
    assert_eq!(request.path(), "/v1/mods/search");
    assert_eq!(request.header("x-api-key"), Some(KEY));
    assert_eq!(request.header("user-agent"), Some(USER_AGENT));
    let query = request.query();
    for expected in [
        ("gameId", "432"),
        ("classId", "6"),
        ("searchFilter", "jei"),
        ("gameVersion", "1.20.1"),
        ("modLoaderType", "1"),
        ("pageSize", "2"),
    ] {
        assert!(
            query.contains(&(expected.0.to_owned(), expected.1.to_owned())),
            "{expected:?} in {query:?}"
        );
    }
}

#[tokio::test]
async fn project_parses_and_missing_is_none() {
    let server = MockServer::start(|req, _| match req.path() {
        "/v1/mods/238222" => Response::json(MOD),
        _ => Response::status(404),
    })
    .await;
    let client = client(&server);
    let jei = client.project(JEI, &ctx()).await.unwrap().unwrap();
    assert_eq!(jei.name, "Just Enough Items (JEI)");
    assert_eq!(
        jei.website_url(),
        "https://www.curseforge.com/minecraft/mc-mods/jei"
    );
    assert_eq!(
        jei.file_page_url(JEI_FILE),
        "https://www.curseforge.com/minecraft/mc-mods/jei/files/9352806"
    );
    assert!(client.project(1, &ctx()).await.unwrap().is_none());
}

#[tokio::test]
async fn project_files_send_filter() {
    let server = MockServer::start(|_, _| Response::json(MOD_FILES)).await;
    let filter = FileFilter {
        limit: 2,
        ..FileFilter::for_instance("1.20.1", "forge")
    };
    let page = client(&server)
        .project_files(JEI, &filter, &ctx())
        .await
        .unwrap();
    assert_eq!(page.data[0].id, JEI_FILE);
    assert_eq!(page.data[0].release_type, ReleaseType::Beta);
    let request = &server.requests()[0];
    assert_eq!(request.path(), "/v1/mods/238222/files");
    assert_eq!(
        request.query(),
        vec![
            ("gameVersion".to_owned(), "1.20.1".to_owned()),
            ("modLoaderType".to_owned(), "1".to_owned()),
            ("pageSize".to_owned(), "2".to_owned()),
        ]
    );
}

#[tokio::test]
async fn file_exposes_hash_loaders_and_dependencies() {
    let server = MockServer::start(|_, _| Response::json(FILE)).await;
    let file = client(&server)
        .file(JEI, JEI_FILE, &ctx())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(server.requests()[0].path(), "/v1/mods/238222/files/9352806");
    assert_eq!(
        file.sha1().as_deref(),
        Some("7e095c4c5b95a478002df0822975920a4201263e")
    );
    assert_eq!(file.minecraft_versions(), vec!["1.20.1".to_owned()]);
    assert_eq!(file.loaders(), vec!["forge".to_owned()]);
    assert!(
        file.dependencies
            .iter()
            .any(|d| d.relation_type == RelationType::RequiredDependency)
    );
    let item = file.download_item(std::path::Path::new("mods")).unwrap();
    assert!(item.url.starts_with("https://edge.forgecdn.net/"));
    assert_eq!(
        item.path,
        std::path::Path::new("mods").join("jei-1.20.1-forge-15.74.0.232.jar")
    );
    assert_eq!(item.size, Some(file.file_length));
}

#[tokio::test]
async fn batch_requests_post_ids() {
    let server = MockServer::start(|req, _| match req.path() {
        "/v1/mods" => Response::json(br#"{"data":[]}"#),
        "/v1/mods/files" => Response::json(br#"{"data":[]}"#),
        _ => Response::status(404),
    })
    .await;
    let client = client(&server);
    assert!(client.projects(&[1, 2], &ctx()).await.unwrap().is_empty());
    assert!(client.files(&[3], &ctx()).await.unwrap().is_empty());
    assert!(client.projects(&[], &ctx()).await.unwrap().is_empty());
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "POST");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["modIds"], serde_json::json!([1, 2]));
    let body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(body["fileIds"], serde_json::json!([3]));
}

#[tokio::test]
async fn fingerprints_tolerate_null_lists() {
    let server = MockServer::start(|_, _| Response::json(FINGERPRINTS)).await;
    let fp = jei_fingerprint();
    let matches = client(&server)
        .match_fingerprints(&[fp, 1], &ctx())
        .await
        .unwrap();
    let request = &server.requests()[0];
    assert_eq!(request.path(), "/v1/fingerprints/432");
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["fingerprints"], serde_json::json!([fp, 1]));
    assert_eq!(matches.exact_matches.len(), 1);
    assert_eq!(matches.exact_matches[0].id, JEI);
    assert_eq!(matches.exact_matches[0].file.file_fingerprint, fp);
    assert!(matches.unmatched_fingerprints.is_empty());
}

#[tokio::test]
async fn modpack_search_parses() {
    let server = MockServer::start(|_, _| Response::json(MODPACK_SEARCH)).await;
    let query = SearchQuery {
        class_id: Some(ClassId::Modpacks),
        ..SearchQuery::default()
    };
    let page = client(&server).search(&query, &ctx()).await.unwrap();
    assert!(
        page.data
            .iter()
            .all(|m| m.class_id == Some(ClassId::Modpacks))
    );
    assert!(page.data[0].website_url().contains("/minecraft/modpacks/"));
}

#[tokio::test]
async fn server_errors_are_retried_and_forbidden_is_not() {
    let server = MockServer::start(|req, n| match (req.path(), n) {
        ("/v1/mods/238222", 0) => Response::status(503),
        ("/v1/mods/238222", _) => Response::json(MOD),
        _ => Response::status(403),
    })
    .await;
    let client = client(&server);
    assert!(client.project(JEI, &ctx()).await.unwrap().is_some());
    let err = client
        .search(&SearchQuery::default(), &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    assert_eq!(err.params.get("status").map(String::as_str), Some("403"));
    assert!(!format!("{err:?}").contains(KEY));
    // 403 不重试：两次成功前的 503 + 一次 403。
    assert_eq!(server.requests().len(), 3);
}

#[tokio::test]
async fn cancelled_request_fails_fast() {
    let server = MockServer::start(|_, _| Response::json(SEARCH)).await;
    let ctx = ctx();
    ctx.cancel.cancel();
    let err = client(&server)
        .search(&SearchQuery::default(), &ctx)
        .await
        .unwrap_err();
    assert!(err.is_cancelled());
}

#[tokio::test]
#[ignore = "联网：需要带 CurseForge API Key 构建"]
async fn online_search_and_cdn_download() {
    if !curseforge::is_available() {
        eprintln!("本次构建没有 CurseForge API Key，跳过");
        return;
    }
    let http = HttpClient::new().unwrap();
    let client = CurseForgeClient::from_build(&http).unwrap().unwrap();
    let ctx = ctx();
    let query = SearchQuery {
        class_id: Some(ClassId::Mods),
        game_version: Some("1.20.1".into()),
        loader: Some(ModLoaderType::Forge),
        limit: 1,
        ..SearchQuery::new("jei")
    };
    let page = client.search(&query, &ctx).await.unwrap();
    assert!(!page.data.is_empty());
    let files = client
        .project_files(
            JEI,
            &FileFilter {
                limit: 1,
                ..FileFilter::for_instance("1.20.1", "forge")
            },
            &ctx,
        )
        .await
        .unwrap();
    let file = &files.data[0];
    let cdn = curseforge::with_cdn_key(&http).unwrap();
    let bytes = cdn
        .get_bytes(file.https_download_url().unwrap(), &ctx)
        .await
        .unwrap();
    assert_eq!(fingerprint::of_bytes(&bytes), file.file_fingerprint);
}
