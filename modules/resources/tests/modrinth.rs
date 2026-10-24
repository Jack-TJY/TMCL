//! Modrinth 客户端测试：离线部分用 `fixtures/modrinth/` 下录制的真实响应（2026-10-24，
//! 已截短长文本与列表），联网部分标记为 `#[ignore]`。

mod support;

use std::time::{Duration, Instant};

use support::{MockServer, Response};
use tmcl_common::{ErrorCode, TaskCtx};
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};
use tmcl_resources::modrinth::{
    DependencyType, HashAlgorithm, ModrinthClient, ProjectType, SearchIndex, SearchQuery,
    USER_AGENT, VersionFilter,
};

const SEARCH: &[u8] = include_bytes!("fixtures/modrinth/search.json");
const PROJECT: &[u8] = include_bytes!("fixtures/modrinth/project.json");
const PROJECT_VERSIONS: &[u8] = include_bytes!("fixtures/modrinth/project_versions.json");
const DEPENDENCIES: &[u8] = include_bytes!("fixtures/modrinth/dependencies.json");
const VERSION_FILE: &[u8] = include_bytes!("fixtures/modrinth/version_file.json");
const VERSION_FILES: &[u8] = include_bytes!("fixtures/modrinth/version_files.json");
const VERSION_FILES_UPDATE: &[u8] = include_bytes!("fixtures/modrinth/version_files_update.json");

/// Sodium mc1.20.1-0.5.1（旧版本）的 SHA1。
const OLD_SODIUM: &str = "64908970887e85b6aa9fe4eaa690ddcb84c3369c";
/// Iris 1.7.6+1.20.1 的 SHA1。
const IRIS: &str = "880c0b9ce38bb4997645793407c1801df83f4233";
const UNKNOWN: &str = "0000000000000000000000000000000000000000";

fn client(server: &MockServer) -> ModrinthClient {
    ModrinthClient::with_base(&HttpClient::new().unwrap(), &server.url("/v2"))
        .unwrap()
        .with_retry(RetryPolicy::with_delays([Duration::ZERO; 3]))
}

fn ctx() -> TaskCtx {
    TaskCtx::noop("test")
}

#[tokio::test]
async fn search_sends_facets_and_user_agent() {
    let server = MockServer::start(|_, _| Response::json(SEARCH)).await;
    let query = SearchQuery {
        project_type: Some(ProjectType::Mod),
        game_versions: vec!["1.20.1".into()],
        loaders: vec!["fabric".into()],
        index: SearchIndex::Downloads,
        limit: 2,
        ..SearchQuery::new("sodium")
    };
    let result = client(&server).search(&query, &ctx()).await.unwrap();
    assert_eq!(result.hits[0].slug, "sodium");

    let request = &server.requests()[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.path(), "/v2/search");
    assert_eq!(request.header("user-agent"), Some(USER_AGENT));
    assert!(USER_AGENT.starts_with("Jack-TJY/TMCL/"));
    assert_eq!(
        request.query(),
        vec![
            ("query".to_owned(), "sodium".to_owned()),
            (
                "facets".to_owned(),
                r#"[["project_type:mod"],["versions:1.20.1"],["categories:fabric"]]"#.to_owned()
            ),
            ("index".to_owned(), "downloads".to_owned()),
            ("limit".to_owned(), "2".to_owned()),
        ]
    );
}

#[tokio::test]
async fn missing_project_is_none() {
    let server = MockServer::start(|req, _| match req.path() {
        "/v2/project/sodium" => Response::json(PROJECT),
        _ => Response::status(404),
    })
    .await;
    let client = client(&server);

    let project = client.project("sodium", &ctx()).await.unwrap().unwrap();
    assert_eq!(project.id, "AANobbMI");
    assert!(client.project("missing", &ctx()).await.unwrap().is_none());
}

#[tokio::test]
async fn path_segments_are_percent_encoded() {
    let server = MockServer::start(|_, _| Response::status(404)).await;
    let found = client(&server).project("a/b?c", &ctx()).await.unwrap();
    assert!(found.is_none());
    assert_eq!(server.requests()[0].target, "/v2/project/a%2Fb%3Fc");
}

#[tokio::test]
async fn project_versions_are_filtered() {
    let server = MockServer::start(|_, _| Response::json(PROJECT_VERSIONS)).await;
    let filter = VersionFilter::for_instance("1.20.1", "fabric");
    let versions = client(&server)
        .project_versions("sodium", &filter, &ctx())
        .await
        .unwrap();

    let request = &server.requests()[0];
    assert_eq!(request.path(), "/v2/project/sodium/version");
    assert_eq!(
        request.query(),
        vec![
            ("loaders".to_owned(), r#"["fabric"]"#.to_owned()),
            ("game_versions".to_owned(), r#"["1.20.1"]"#.to_owned()),
        ]
    );

    assert_eq!(versions[0].id, "OihdIimA");
}

#[tokio::test]
async fn dependencies_are_parsed() {
    let server = MockServer::start(|_, _| Response::json(DEPENDENCIES)).await;
    let deps = client(&server)
        .project_dependencies("sodium", &ctx())
        .await
        .unwrap();
    assert_eq!(
        server.requests()[0].path(),
        "/v2/project/sodium/dependencies"
    );
    assert_eq!(deps.projects.len(), 1);
    assert!(deps.versions.is_empty());
}

#[tokio::test]
async fn batch_lookups_send_ids_and_skip_empty_input() {
    let server = MockServer::start(|req, _| match req.path() {
        "/v2/versions" => Response::json(PROJECT_VERSIONS),
        _ => Response::json(b"[]"),
    })
    .await;
    let client = client(&server);
    assert!(client.versions(&[], &ctx()).await.unwrap().is_empty());
    assert!(client.projects(&[], &ctx()).await.unwrap().is_empty());
    assert!(server.requests().is_empty());

    let ids = vec!["OihdIimA".to_owned(), "ryOMVRuG".to_owned()];
    let versions = client.versions(&ids, &ctx()).await.unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(
        server.requests()[0].query(),
        vec![("ids".to_owned(), r#"["OihdIimA","ryOMVRuG"]"#.to_owned())]
    );
    assert!(client.projects(&ids, &ctx()).await.unwrap().is_empty());
}

#[tokio::test]
async fn too_many_requests_waits_for_reset_then_retries() {
    let server = MockServer::start(|_, index| match index {
        0 => Response::status(429)
            .header("X-Ratelimit-Remaining", 0)
            .header("X-Ratelimit-Reset", 1),
        _ => Response::json(PROJECT),
    })
    .await;
    let started = Instant::now();
    let project = client(&server).project("sodium", &ctx()).await.unwrap();
    assert!(project.is_some());
    assert_eq!(server.requests().len(), 2);
    assert!(started.elapsed() >= Duration::from_millis(900));
}

#[tokio::test]
async fn used_up_quota_delays_the_next_request() {
    let server = MockServer::start(|_, _| {
        Response::json(PROJECT)
            .header("X-Ratelimit-Remaining", 0)
            .header("X-Ratelimit-Reset", 1)
    })
    .await;
    let client = client(&server);
    client.project("sodium", &ctx()).await.unwrap();
    let started = Instant::now();
    client.clone().project("sodium", &ctx()).await.unwrap();
    assert!(started.elapsed() >= Duration::from_millis(900));
}

#[tokio::test]
async fn rate_limit_wait_can_be_cancelled() {
    let server =
        MockServer::start(|_, _| Response::status(429).header("X-Ratelimit-Reset", 60)).await;
    let client = client(&server);
    let ctx = ctx();
    let cancel = ctx.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel.cancel();
    });
    let started = Instant::now();
    let err = client.project("sodium", &ctx).await.unwrap_err();
    assert!(err.is_cancelled());
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn server_errors_are_retried_then_reported() {
    let server = MockServer::start(|_, _| Response::status(503)).await;
    let err = client(&server).project("sodium", &ctx()).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    assert_eq!(err.params.get("status").map(String::as_str), Some("503"));
    assert_eq!(server.requests().len(), 4);
}

#[tokio::test]
async fn invalid_json_and_missing_list_are_errors() {
    let server = MockServer::start(|req, _| match req.path() {
        "/v2/search" => Response::json(b"{\"hits\":"),
        _ => Response::status(404),
    })
    .await;
    let client = client(&server);
    let err = client
        .search(&SearchQuery::new("x"), &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::CommonJson);

    let err = client
        .project_versions("missing", &VersionFilter::default(), &ctx())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NetHttpStatus);
    assert_eq!(err.params.get("status").map(String::as_str), Some("404"));
}

#[tokio::test]
async fn version_from_hash_returns_none_for_unknown_files() {
    let server = MockServer::start(|req, _| match req.path() {
        p if p.ends_with(OLD_SODIUM) => Response::json(VERSION_FILE),
        _ => Response::status(404),
    })
    .await;
    let client = client(&server);
    let version = client
        .version_from_hash(OLD_SODIUM, HashAlgorithm::Sha1, &ctx())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(version.version_number, "mc1.20.1-0.5.1");
    let request = &server.requests()[0];
    assert_eq!(request.path(), format!("/v2/version_file/{OLD_SODIUM}"));
    assert_eq!(
        request.query(),
        vec![("algorithm".to_owned(), "sha1".to_owned())]
    );

    let missing = client
        .version_from_hash(UNKNOWN, HashAlgorithm::Sha1, &ctx())
        .await
        .unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn versions_from_hashes_posts_hashes() {
    let server = MockServer::start(|_, _| Response::json(VERSION_FILES)).await;
    let hashes = vec![OLD_SODIUM.to_owned(), IRIS.to_owned(), UNKNOWN.to_owned()];
    let found = client(&server)
        .versions_from_hashes(&hashes, HashAlgorithm::Sha1, &ctx())
        .await
        .unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[OLD_SODIUM].project_id, "AANobbMI");
    let iris = &found[IRIS];
    assert_eq!(iris.project_id, "YL57xq9U");
    assert!(
        iris.required_dependencies()
            .any(|d| d.project_id.as_deref() == Some("AANobbMI"))
    );

    let request = &server.requests()[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.path(), "/v2/version_files");
    assert_eq!(request.header("content-type"), Some("application/json"));
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "hashes": hashes, "algorithm": "sha1" })
    );
}

#[tokio::test]
async fn latest_versions_from_hashes_finds_updates() {
    let server = MockServer::start(|_, _| Response::json(VERSION_FILES_UPDATE)).await;
    let hashes = vec![OLD_SODIUM.to_owned(), UNKNOWN.to_owned()];
    let filter = VersionFilter::for_instance("1.20.1", "fabric");
    let latest = client(&server)
        .latest_versions_from_hashes(&hashes, HashAlgorithm::Sha1, &filter, &ctx())
        .await
        .unwrap();
    assert_eq!(latest.len(), 1);
    let newest = &latest[OLD_SODIUM];
    assert_eq!(newest.version_number, "mc1.20.1-0.5.13-fabric");
    assert_ne!(newest.primary_file().unwrap().hashes.sha1, OLD_SODIUM);

    let request = &server.requests()[0];
    assert_eq!(request.path(), "/v2/version_files/update");
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "hashes": hashes,
            "algorithm": "sha1",
            "loaders": ["fabric"],
            "game_versions": ["1.20.1"],
        })
    );
}

#[tokio::test]
async fn empty_hash_lists_send_nothing() {
    let server = MockServer::start(|_, _| Response::status(500)).await;
    let client = client(&server);
    let filter = VersionFilter::default();
    let found = client
        .versions_from_hashes(&[], HashAlgorithm::Sha1, &ctx())
        .await
        .unwrap();
    assert!(found.is_empty());
    let latest = client
        .latest_versions_from_hashes(&[], HashAlgorithm::Sha512, &filter, &ctx())
        .await
        .unwrap();
    assert!(latest.is_empty());
    assert!(server.requests().is_empty());
}

/// 联网：搜索 Sodium，取 Fabric 1.20.1 的最新版本并下载、校验主文件，再按哈希反查与检查更新。
/// 运行：`cargo test -p tmcl-resources --test modrinth -- --ignored`
#[tokio::test]
#[ignore = "需要联网"]
async fn online_search_and_download() {
    let http = HttpClient::new().unwrap();
    let client = ModrinthClient::new(&http).unwrap();
    let ctx = ctx();
    let query = SearchQuery {
        project_type: Some(ProjectType::Mod),
        game_versions: vec!["1.20.1".into()],
        loaders: vec!["fabric".into()],
        limit: 5,
        ..SearchQuery::new("sodium")
    };
    let result = client.search(&query, &ctx).await.unwrap();
    let hit = result.hits.iter().find(|h| h.slug == "sodium").unwrap();

    let versions = client
        .project_versions(
            &hit.project_id,
            &VersionFilter::for_instance("1.20.1", "fabric"),
            &ctx,
        )
        .await
        .unwrap();
    let file = versions[0].primary_file().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let item = file.download_item(dir.path());
    let downloader = Downloader::new(http, DownloadOptions::default());
    downloader.download_one(&item, &ctx).await.unwrap();
    assert!(
        tmcl_net::is_valid(&item.path, item.size, item.sha1.as_deref())
            .await
            .unwrap()
    );

    let sha1 = tmcl_net::sha1_file(&item.path).await.unwrap();
    let found = client
        .version_from_hash(&sha1, HashAlgorithm::Sha1, &ctx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id, versions[0].id);
    let latest = client
        .latest_versions_from_hashes(
            &[OLD_SODIUM.to_owned()],
            HashAlgorithm::Sha1,
            &VersionFilter::for_instance("1.20.1", "fabric"),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(latest[OLD_SODIUM].id, versions[0].id);

    let deps = client.project_dependencies("iris", &ctx).await.unwrap();
    assert!(deps.projects.iter().any(|p| p.slug == "sodium"));
    let iris = client
        .project_versions(
            "iris",
            &VersionFilter::for_instance("1.20.1", "fabric"),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        iris[0]
            .dependencies
            .iter()
            .any(|d| d.dependency_type == DependencyType::Required
                && d.project_id.as_deref() == Some(hit.project_id.as_str()))
    );
}
