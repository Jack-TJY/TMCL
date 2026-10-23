//! 加载器版本列表的离线测试。

mod common;

use common::{GAME, downloader, endpoints};
use serde_json::json;
use tmcl_common::{ErrorCode, TaskCtx};
use tmcl_loader::list::forge_versions;
use tmcl_loader::{LoaderKind, list_versions};
use tmcl_net::testing::TestServer;
use tmcl_net::{PrefixMirror, Sources};

async fn server() -> TestServer {
    TestServer::start(Vec::<(String, Vec<u8>)>::new()).await
}

#[tokio::test]
async fn lists_fabric_and_quilt_versions() {
    let server = server().await;
    server.route(
        format!("/fabric/v2/versions/loader/{GAME}"),
        serde_json::to_vec(&json!([
            {"loader": {"version": "0.17.0", "stable": false}},
            {"loader": {"version": "0.16.5", "stable": true}},
            {"loader": {"version": "0.16.4", "stable": true}},
        ]))
        .unwrap(),
    );
    server.route(
        format!("/quilt/v3/versions/loader/{GAME}"),
        serde_json::to_vec(&json!([
            {"loader": {"version": "0.27.0-beta.1"}},
            {"loader": {"version": "0.26.4"}},
        ]))
        .unwrap(),
    );
    let ctx = TaskCtx::noop("t");
    let fabric = list_versions(
        &downloader(),
        &endpoints(&server),
        LoaderKind::Fabric,
        GAME,
        &ctx,
    )
    .await
    .unwrap();
    let summary: Vec<_> = fabric
        .iter()
        .map(|v| (v.version.as_str(), v.stable, v.recommended))
        .collect();
    assert_eq!(
        summary,
        [
            ("0.17.0", false, false),
            ("0.16.5", true, true),
            ("0.16.4", true, false)
        ]
    );
    let quilt = list_versions(
        &downloader(),
        &endpoints(&server),
        LoaderKind::Quilt,
        GAME,
        &ctx,
    )
    .await
    .unwrap();
    let summary: Vec<_> = quilt
        .iter()
        .map(|v| (v.version.as_str(), v.stable, v.recommended))
        .collect();
    assert_eq!(
        summary,
        [("0.27.0-beta.1", false, false), ("0.26.4", true, true)]
    );
    // 不认识的游戏版本（服务器 404）是空列表。
    let none = list_versions(
        &downloader(),
        &endpoints(&server),
        LoaderKind::Fabric,
        "0.0.1",
        &ctx,
    )
    .await
    .unwrap();
    assert!(none.is_empty());
}

fn metadata(versions: &[&str]) -> Vec<u8> {
    let items: String = versions
        .iter()
        .map(|v| format!("<version>{v}</version>"))
        .collect();
    format!("<metadata><versioning><versions>{items}</versions></versioning></metadata>")
        .into_bytes()
}

#[tokio::test]
async fn lists_forge_with_promotions() {
    let server = server().await;
    server.route(
        "/forge/net/minecraftforge/forge/maven-metadata.xml",
        metadata(&[
            "1.20.1-47.1.0",
            "1.20.1-47.10.0",
            "1.20.1-47.2.0",
            "1.19.4-45.0.0",
        ]),
    );
    let ctx = TaskCtx::noop("t");
    // 没有 promotions 时以最新的正式版为推荐。
    let list = list_versions(
        &downloader(),
        &endpoints(&server),
        LoaderKind::Forge,
        GAME,
        &ctx,
    )
    .await
    .unwrap();
    let versions: Vec<_> = list.iter().map(|v| v.version.as_str()).collect();
    assert_eq!(versions, ["47.10.0", "47.2.0", "47.1.0"]);
    assert!(list[0].recommended);

    server.route(
        "/forge-files/net/minecraftforge/forge/promotions_slim.json",
        serde_json::to_vec(&json!({"promos": {
            "1.20.1-latest": "47.10.0", "1.20.1-recommended": "47.2.0"
        }}))
        .unwrap(),
    );
    let list = list_versions(
        &downloader(),
        &endpoints(&server),
        LoaderKind::Forge,
        GAME,
        &ctx,
    )
    .await
    .unwrap();
    let recommended: Vec<_> = list
        .iter()
        .filter(|v| v.recommended)
        .map(|v| v.version.as_str())
        .collect();
    assert_eq!(recommended, ["47.2.0"]);

    let err = list_versions(
        &downloader(),
        &endpoints(&server),
        LoaderKind::Forge,
        "1.6.4",
        &ctx,
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::LoaderUnsupportedVersion);
    assert_eq!(err.params["game"], "1.6.4");
}

#[tokio::test]
async fn lists_neoforge_including_legacy_1_20_1() {
    let server = server().await;
    server.route(
        "/neoforge/net/neoforged/neoforge/maven-metadata.xml",
        metadata(&["21.1.1", "21.1.10", "21.0.5-beta", "20.4.80"]),
    );
    server.route(
        "/neoforge/net/neoforged/forge/maven-metadata.xml",
        metadata(&["1.20.1-47.1.5", "1.20.1-47.1.106"]),
    );
    let ctx = TaskCtx::noop("t");
    let list = |game: &'static str| {
        let (downloader, endpoints, ctx) = (downloader(), endpoints(&server), ctx.clone());
        async move {
            list_versions(&downloader, &endpoints, LoaderKind::Neoforge, game, &ctx)
                .await
                .unwrap()
                .into_iter()
                .map(|v| v.version)
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(list("1.21.1").await, ["21.1.10", "21.1.1"]);
    assert_eq!(list("1.21").await, ["21.0.5-beta"]);
    assert_eq!(list("1.20.1").await, ["47.1.106", "47.1.5"]);
}

#[tokio::test]
async fn forge_prefers_bmclapi_lists_when_mirror_is_enabled() {
    let server = server().await;
    // 镜像的 maven-metadata.xml 是旧的，不应被使用。
    server.route(
        "/mirror/net/minecraftforge/forge/maven-metadata.xml",
        metadata(&["1.20.1-47.0.1"]),
    );
    server.route(
        "/bmclapi/forge/minecraft/1.7.10",
        serde_json::to_vec(&json!([
            {"version": "10.13.4.1614", "branch": "1.7.10", "build": 1614},
            {"version": "10.13.0.1150", "branch": null, "build": 1150},
        ]))
        .unwrap(),
    );
    server.route(
        "/bmclapi/forge/promos",
        serde_json::to_vec(&json!([
            {"name": "1.7.10-recommended", "build": {"version": "10.13.4.1614"}},
            {"name": "latest", "build": null},
        ]))
        .unwrap(),
    );
    let mut sources = Sources::new("test-mirror");
    sources.add_mirror(PrefixMirror::new(
        "test-mirror",
        [(server.url("/forge"), server.url("/mirror"))],
    ));
    let downloader = downloader().with_sources(sources);
    let endpoints = endpoints(&server);
    let ctx = TaskCtx::noop("t");

    let versions = forge_versions(&downloader, &endpoints, "1.7.10", &ctx)
        .await
        .unwrap();
    assert_eq!(
        versions,
        [
            (
                "10.13.4.1614".to_owned(),
                "1.7.10-10.13.4.1614-1.7.10".to_owned()
            ),
            ("10.13.0.1150".to_owned(), "1.7.10-10.13.0.1150".to_owned()),
        ]
    );
    let list = list_versions(&downloader, &endpoints, LoaderKind::Forge, "1.7.10", &ctx)
        .await
        .unwrap();
    assert!(list[0].recommended && !list[1].recommended);
    assert!(
        server
            .requests()
            .iter()
            .all(|r| !r.path.ends_with("maven-metadata.xml"))
    );

    // BMCLAPI 没有该版本时直连官方 maven（不经镜像改写）。
    server.route(
        "/forge/net/minecraftforge/forge/maven-metadata.xml",
        metadata(&["1.20.1-47.2.0"]),
    );
    let versions = forge_versions(&downloader, &endpoints, GAME, &ctx)
        .await
        .unwrap();
    assert_eq!(
        versions,
        [("47.2.0".to_owned(), "1.20.1-47.2.0".to_owned())]
    );
}
