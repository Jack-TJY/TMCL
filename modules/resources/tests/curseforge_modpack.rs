//! CurseForge 整合包（`manifest.json`）安装为新实例的离线测试：CurseForge API 由本地模拟服务器应答，
//! 原版与文件 CDN 由 `TestServer` 提供，不联网。

#[allow(dead_code)] // 与 modrinth.rs 共用，部分辅助函数只在那边使用。
mod support;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use support::{MockServer, Response};
use tmcl_common::{DataPaths, Db, ErrorCode, TaskCtx};
use tmcl_game::InstallOptions;
use tmcl_instance::InstanceStore;
use tmcl_loader::LoaderInstaller;
use tmcl_net::testing::TestServer;
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};
use tmcl_resources::ModpackInstaller;
use tmcl_resources::curseforge::CurseForgeClient;
use tmcl_resources::modpack::ModpackFormat;
use zip::write::SimpleFileOptions;

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 本地服务器上的最小原版 `fake` 与三个文件。
async fn cdn() -> (TestServer, InstallOptions) {
    let server = TestServer::start(Vec::<(String, Vec<u8>)>::new()).await;
    let base = server.url("");
    let add = |path: &str, data: &[u8]| {
        server.route(path, data.to_vec());
        json!({"sha1": sha1_hex(data), "size": data.len(), "url": format!("{base}{path}")})
    };
    let mut index_ref = add("/index.json", br#"{"objects": {}}"#);
    index_ref["id"] = json!("fake-index");
    let client = add("/client.jar", b"client jar");
    let version = json!({
        "id": "fake", "type": "release", "mainClass": "Main",
        "arguments": {"game": [], "jvm": []},
        "assetIndex": index_ref, "assets": "fake-index",
        "downloads": {"client": client}, "libraries": [],
    });
    let entry = add("/fake.json", version.to_string().as_bytes());
    let manifest = json!({
        "latest": {"release": "fake", "snapshot": "fake"},
        "versions": [{"id": "fake", "type": "release", "url": entry["url"],
            "time": "", "releaseTime": "", "sha1": entry["sha1"]}],
    });
    add("/manifest.json", manifest.to_string().as_bytes());
    add("/files/a.jar", b"mod a");
    add("/files/pack.zip", b"resource pack");
    let options = InstallOptions {
        manifest_url: server.url("/manifest.json"),
        resources_url: server.url("/res"),
    };
    (server, options)
}

/// CurseForge 文件 JSON；`url` 为 `None` 表示作者禁止第三方分发。
fn cf_file(id: u32, mod_id: u32, name: &str, url: Option<String>, data: &[u8]) -> Value {
    json!({
        "id": id, "modId": mod_id, "displayName": name, "fileName": name,
        "releaseType": 1, "fileLength": data.len(), "downloadUrl": url,
        "hashes": [{"value": sha1_hex(data), "algo": 1}, {"value": "x", "algo": 2}],
        "gameVersions": ["fake"], "dependencies": [], "fileFingerprint": 1,
    })
}

fn cf_mod(id: u32, name: &str, slug: &str, class_id: u32) -> Value {
    json!({
        "id": id, "name": name, "slug": slug, "classId": class_id,
        "links": {"websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}")},
        "allowModDistribution": id != 3,
    })
}

struct Env {
    _dir: tempfile::TempDir,
    paths: DataPaths,
    pack: PathBuf,
    installer: ModpackInstaller,
    api: MockServer,
    _cdn: TestServer,
}

async fn env(mod_a: &'static [u8]) -> Env {
    let (cdn, options) = cdn().await;
    let files = json!({"data": [
        cf_file(11, 1, "a.jar", Some(cdn.url("/files/a.jar")), mod_a),
        cf_file(22, 2, "pack.zip", Some(cdn.url("/files/pack.zip")), b"resource pack"),
        cf_file(33, 3, "blocked.jar", None, b"blocked"),
    ]})
    .to_string()
    .into_bytes();
    let mods = json!({"data": [
        cf_mod(1, "Mod A", "mod-a", 6),
        cf_mod(2, "Pack", "pack", 12),
        cf_mod(3, "Blocked", "blocked", 6),
    ]})
    .to_string()
    .into_bytes();
    let api = MockServer::start(move |req, _| match (req.method.as_str(), req.path()) {
        ("POST", "/v1/mods/files") => Response::json(&files),
        ("POST", "/v1/mods") => Response::json(&mods),
        _ => Response::status(404),
    })
    .await;

    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path().join("data"));
    paths.ensure_dirs().unwrap();
    let pack = dir.path().join("test.zip");
    write_pack(
        &pack,
        &manifest(json!([
            {"projectID": 1, "fileID": 11, "required": true},
            {"projectID": 2, "fileID": 22, "required": true},
            {"projectID": 3, "fileID": 33, "required": true},
            {"projectID": 4, "fileID": 44, "required": false},
        ])),
        &[
            ("custom/config/a.toml", b"common"),
            ("custom/../escape.txt", b"bad"),
            ("overrides/ignored.txt", b"x"),
        ],
    );
    let downloader = Downloader::new(
        HttpClient::new().unwrap(),
        DownloadOptions {
            retry: RetryPolicy::none(),
            ..DownloadOptions::default()
        },
    );
    let loader = LoaderInstaller::new(downloader, paths.clone()).with_game_options(options.clone());
    let client =
        CurseForgeClient::with_base(&HttpClient::new().unwrap(), &api.url("/v1"), "test-api-key")
            .unwrap()
            .with_retry(RetryPolicy::with_delays([Duration::ZERO; 3]));
    let installer = ModpackInstaller::new(loader, options).with_curseforge(Some(client));
    Env {
        _dir: dir,
        paths,
        pack,
        installer,
        api,
        _cdn: cdn,
    }
}

fn manifest(files: Value) -> Value {
    json!({
        "minecraft": {"version": "fake", "modLoaders": []},
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": "CF Pack",
        "version": "2.0",
        "author": "someone",
        "files": files,
        "overrides": "custom",
    })
}

fn write_pack(path: &Path, manifest: &Value, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options = SimpleFileOptions::default();
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(manifest.to_string().as_bytes()).unwrap();
    for (name, data) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

fn instance_count(paths: &DataPaths) -> usize {
    let db = Db::open(&paths.db_file()).unwrap();
    let store = InstanceStore::new(paths, &db);
    store.reconcile().unwrap();
    store.list().unwrap().len()
}

#[tokio::test]
async fn inspect_resolves_files_and_lists_manual_downloads() {
    let env = env(b"mod a").await;
    let prepared = env.installer.inspect(&env.pack).await.unwrap();
    let info = &prepared.info;
    assert_eq!(info.format, ModpackFormat::Curseforge);
    assert_eq!(
        (info.name.as_str(), info.version.as_str()),
        ("CF Pack", "2.0")
    );
    assert_eq!(info.game_version, "fake");
    assert_eq!(info.file_count, 2);
    assert_eq!(info.download_size, 5 + 13);
    assert_eq!(info.manual_downloads.len(), 1);
    let manual = &info.manual_downloads[0];
    assert_eq!(manual.file_name, "blocked.jar");
    assert_eq!(manual.target_dir, "mods");
    assert_eq!(
        manual.page_url,
        "https://www.curseforge.com/minecraft/mc-mods/blocked/files/33"
    );
    assert_eq!(prepared.overrides, vec!["custom".to_owned()]);

    // 可选文件（fileID 44）不查询；请求带 key。
    let requests = env.api.requests();
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["fileIds"], json!([11, 22, 33]));
    assert!(
        requests
            .iter()
            .all(|r| r.header("x-api-key") == Some("test-api-key"))
    );
}

#[tokio::test]
async fn installs_files_into_class_directories_and_overrides() {
    let env = env(b"mod a").await;
    let ctx = TaskCtx::noop("modpack_install");
    let config = env.installer.install(&env.pack, None, &ctx).await.unwrap();
    assert_eq!(config.name, "CF Pack");
    assert!(config.loader.is_none());

    let game = env.paths.instance_game_dir(&config.id);
    let read = |rel: &str| std::fs::read_to_string(game.join(rel)).unwrap();
    assert_eq!(read("mods/a.jar"), "mod a");
    assert_eq!(read("resourcepacks/pack.zip"), "resource pack");
    assert_eq!(read("config/a.toml"), "common");
    assert!(!game.join("mods/blocked.jar").exists());
    assert!(!game.join("ignored.txt").exists());
    assert!(
        !env.paths
            .instance_dir(&config.id)
            .join("escape.txt")
            .exists()
    );
}

#[tokio::test]
async fn hash_mismatch_rolls_back_new_instance() {
    let env = env(b"other content").await;
    let ctx = TaskCtx::noop("modpack_install");
    let err = env
        .installer
        .install(&env.pack, None, &ctx)
        .await
        .unwrap_err();
    assert_ne!(err.code, ErrorCode::CommonInternal);
    assert_eq!(instance_count(&env.paths), 0);
}

#[tokio::test]
async fn deleted_file_is_reported() {
    let env = env(b"mod a").await;
    write_pack(
        &env.pack,
        &manifest(json!([{"projectID": 1, "fileID": 99, "required": true}])),
        &[],
    );
    let err = env.installer.inspect(&env.pack).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesVersionNotFound);
}

#[tokio::test]
async fn curseforge_pack_without_client_is_unsupported() {
    let env = env(b"mod a").await;
    let (_cdn, options) = cdn().await;
    let downloader = Downloader::new(HttpClient::new().unwrap(), DownloadOptions::default());
    let installer =
        ModpackInstaller::new(LoaderInstaller::new(downloader, env.paths.clone()), options);
    let err = installer.inspect(&env.pack).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesModpackUnsupported);
    assert_eq!(
        err.params.get("name").map(String::as_str),
        Some("curseforge")
    );
}

#[tokio::test]
async fn zip_without_any_index_is_invalid() {
    let env = env(b"mod a").await;
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&env.pack).unwrap());
    zip.start_file("readme.txt", SimpleFileOptions::default())
        .unwrap();
    zip.finish().unwrap();
    let err = env.installer.inspect(&env.pack).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesModpackInvalid);
}

#[tokio::test]
#[ignore = "联网：需要带 CurseForge API Key 构建，会下载一个整合包"]
async fn online_fetch_and_inspect_popular_pack() {
    use tmcl_resources::curseforge::{self, ClassId, FileFilter, SearchQuery};
    use tmcl_resources::modpack::source::{self, ModpackSource};
    use tmcl_resources::modrinth::ModrinthClient;

    if !curseforge::is_available() {
        eprintln!("本次构建没有 CurseForge API Key，跳过");
        return;
    }
    let http = HttpClient::new().unwrap();
    let client = CurseForgeClient::from_build(&http).unwrap().unwrap();
    let ctx = TaskCtx::noop("online");
    let query = SearchQuery {
        class_id: Some(ClassId::Modpacks),
        game_version: Some("1.20.1".into()),
        limit: 1,
        ..SearchQuery::new("")
    };
    let pack = client.search(&query, &ctx).await.unwrap().data.remove(0);
    let files = client
        .project_files(
            pack.id,
            &FileFilter {
                limit: 1,
                ..FileFilter::default()
            },
            &ctx,
        )
        .await
        .unwrap();
    let file = &files.data[0];
    let dir = tempfile::tempdir().unwrap();
    let downloader = Downloader::new(
        curseforge::with_cdn_key(&http).unwrap(),
        DownloadOptions::default(),
    );
    let fetched = source::fetch(
        &ModpackSource::Curseforge {
            project_id: pack.id,
            file_id: file.id,
        },
        &ModrinthClient::new(&http).unwrap(),
        Some(&client),
        &downloader,
        dir.path(),
        &ctx,
    )
    .await
    .unwrap();
    let paths = DataPaths::at(dir.path().join("data"));
    let installer = ModpackInstaller::new(
        LoaderInstaller::new(downloader, paths),
        InstallOptions::default(),
    )
    .with_curseforge(Some(client));
    let prepared = installer.inspect(&fetched.path).await.unwrap();
    eprintln!(
        "{} {}: {} files, {} manual, loader {:?}",
        pack.name,
        prepared.info.version,
        prepared.info.file_count,
        prepared.info.manual_downloads.len(),
        prepared.info.loader
    );
    assert_eq!(prepared.info.format, ModpackFormat::Curseforge);
    assert!(prepared.info.file_count > 0);
}
