//! `.mrpack` 解析与安装为新实例的离线测试。

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use tmcl_common::{DataPaths, Db, ErrorCode, TaskCtx};
use tmcl_game::InstallOptions;
use tmcl_instance::{InstanceStore, LoaderKind};
use tmcl_loader::{Endpoints, LoaderInstaller};
use tmcl_net::testing::{Fault, TestServer};
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};
use tmcl_resources::ModpackInstaller;
use tmcl_resources::modpack::mrpack::{self, parse_index, safe_relative};
use zip::write::SimpleFileOptions;

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn index(files: Value, dependencies: Value) -> Value {
    json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "1.2.0",
        "name": "Test Pack", "summary": "A pack", "files": files, "dependencies": dependencies,
    })
}

fn file(path: &str, url: &str, data: &[u8]) -> Value {
    json!({
        "path": path, "hashes": {"sha1": sha1_hex(data).to_uppercase(), "sha512": "00"},
        "downloads": [url], "fileSize": data.len(),
    })
}

#[test]
fn parses_index_and_skips_server_only_files() {
    let mut server_only = file("mods/server.jar", "https://cdn.modrinth.com/s.jar", b"s");
    server_only["env"] = json!({"client": "unsupported", "server": "required"});
    let mut optional = file("mods/opt.jar", "https://cdn.modrinth.com/o.jar", b"opt");
    optional["env"] = json!({"client": "optional", "server": "optional"});
    let mut two_urls = file("mods/a.jar", "http://example.com/a.jar", b"abc");
    two_urls["downloads"] = json!([
        "ftp://x/a.jar",
        "http://example.com/a.jar",
        "https://cdn.modrinth.com/a.jar"
    ]);
    let text = index(
        json!([two_urls, server_only, optional]),
        json!({"minecraft": "1.20.1", "fabric-loader": "0.16.5"}),
    )
    .to_string();

    let pack = parse_index(&text).unwrap();
    assert_eq!(pack.info.name, "Test Pack");
    assert_eq!(pack.info.version, "1.2.0");
    assert_eq!(pack.info.summary.as_deref(), Some("A pack"));
    assert_eq!(pack.info.game_version, "1.20.1");
    let loader = pack.info.loader.unwrap();
    assert_eq!(
        (loader.kind, loader.version.as_str()),
        (LoaderKind::Fabric, "0.16.5")
    );
    assert_eq!(pack.info.file_count, 2);
    assert_eq!(pack.info.download_size, 6);
    assert_eq!(pack.files[0].path, Path::new("mods").join("a.jar"));
    assert_eq!(pack.files[0].url, "https://cdn.modrinth.com/a.jar");
    assert_eq!(pack.files[0].sha1, sha1_hex(b"abc"));
    assert_eq!(pack.files[1].path, Path::new("mods").join("opt.jar"));

    for (key, kind) in [
        ("forge", LoaderKind::Forge),
        ("neoforge", LoaderKind::Neoforge),
        ("quilt-loader", LoaderKind::Quilt),
    ] {
        let text = index(json!([]), json!({"minecraft": "1.20.1", key: "1.0"})).to_string();
        assert_eq!(parse_index(&text).unwrap().info.loader.unwrap().kind, kind);
    }
    let vanilla = index(json!([]), json!({"minecraft": "1.20.1"})).to_string();
    assert!(parse_index(&vanilla).unwrap().info.loader.is_none());
}

#[test]
fn rejects_invalid_and_unsupported_packs() {
    let ok = |path: &str| file(path, "https://cdn.modrinth.com/a.jar", b"a");
    let deps = json!({"minecraft": "1.20.1"});
    let mut no_sha1 = ok("mods/a.jar");
    no_sha1["hashes"] = json!({"sha512": "00"});
    let mut bad_scheme = ok("mods/a.jar");
    bad_scheme["downloads"] = json!(["file:///etc/passwd"]);
    let mut invalid = vec![
        "not json".to_owned(),
        index(json!([]), json!({})).to_string(),
        index(json!([]), json!({"minecraft": "../1.20.1"})).to_string(),
        index(json!([no_sha1]), deps.clone()).to_string(),
        index(json!([bad_scheme]), deps.clone()).to_string(),
    ];
    for path in [
        "../evil.jar",
        "/abs.jar",
        "mods\\a.jar",
        "C:/a.jar",
        "mods/./a.jar",
        "mods//a.jar",
        "",
    ] {
        invalid.push(index(json!([ok(path)]), deps.clone()).to_string());
    }
    for text in invalid {
        let err = parse_index(&text).unwrap_err();
        assert_eq!(err.code, ErrorCode::ResourcesModpackInvalid, "{text}");
    }

    let mut v2 = index(json!([]), deps.clone());
    v2["formatVersion"] = json!(2);
    let mut other_game = index(json!([]), deps);
    other_game["game"] = json!("minetest");
    for (text, name) in [
        (v2.to_string(), "formatVersion 2"),
        (other_game.to_string(), "minetest"),
        (
            index(json!([]), json!({"minecraft": "1.20.1", "liteloader": "1"})).to_string(),
            "liteloader",
        ),
    ] {
        let err = parse_index(&text).unwrap_err();
        assert_eq!(err.code, ErrorCode::ResourcesModpackUnsupported, "{text}");
        assert_eq!(err.params["name"], name);
    }
    let two = index(
        json!([]),
        json!({"minecraft": "1", "forge": "1", "fabric-loader": "1"}),
    );
    let err = parse_index(&two.to_string()).unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesModpackUnsupported);
}

#[test]
fn safe_relative_paths() {
    assert_eq!(
        safe_relative("config/a b/c.toml").unwrap(),
        Path::new("config").join("a b").join("c.toml")
    );
    for bad in ["", "..", "a/../b", "./a", "/a", "a/", "a\\b", "C:a", "a\0b"] {
        assert!(safe_relative(bad).is_none(), "{bad:?}");
    }
}

fn write_pack(path: &Path, index: &Value, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options = SimpleFileOptions::default();
    zip.start_file(mrpack::INDEX_FILE, options).unwrap();
    zip.write_all(index.to_string().as_bytes()).unwrap();
    for (name, data) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

/// 本地服务器上的最小原版 `fake`（只有客户端 jar，资源索引为空）与两个 mod 文件。
async fn fake_server() -> (TestServer, InstallOptions) {
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
    add("/mods/a.jar", b"mod a");
    add("/mods/b.jar", b"mod b");
    let options = InstallOptions {
        manifest_url: server.url("/manifest.json"),
        resources_url: server.url("/res"),
    };
    (server, options)
}

struct Env {
    _dir: tempfile::TempDir,
    paths: DataPaths,
    pack: PathBuf,
    installer: ModpackInstaller,
    server: TestServer,
}

async fn env(mod_b: &[u8]) -> Env {
    env_with(mod_b, json!({"minecraft": "fake"})).await
}

async fn env_with(mod_b: &[u8], dependencies: Value) -> Env {
    let (server, options) = fake_server().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path().join("data"));
    paths.ensure_dirs().unwrap();
    let pack = dir.path().join("test.mrpack");
    let index = index(
        json!([
            file("mods/a.jar", &server.url("/mods/a.jar"), b"mod a"),
            file("mods/b.jar", &server.url("/mods/b.jar"), mod_b),
        ]),
        dependencies,
    );
    write_pack(
        &pack,
        &index,
        &[
            ("overrides/config/a.toml", b"common"),
            ("overrides/options.txt", b"lang:en_us"),
            ("client-overrides/options.txt", b"lang:zh_cn"),
            ("overrides/../escape.txt", b"bad"),
            ("other/ignored.txt", b"x"),
        ],
    );
    let downloader = Downloader::new(
        HttpClient::new().unwrap(),
        DownloadOptions {
            retry: RetryPolicy::none(),
            ..DownloadOptions::default()
        },
    );
    let endpoints = Endpoints {
        fabric_meta: server.url(""),
        ..Endpoints::default()
    };
    let loader = LoaderInstaller::new(downloader, paths.clone())
        .with_endpoints(endpoints)
        .with_game_options(options.clone());
    let installer = ModpackInstaller::new(loader, options);
    Env {
        _dir: dir,
        paths,
        pack,
        installer,
        server,
    }
}

fn instance_count(paths: &DataPaths) -> usize {
    let db = Db::open(&paths.db_file()).unwrap();
    let store = InstanceStore::new(paths, &db);
    store.reconcile().unwrap();
    store.list().unwrap().len()
}

#[tokio::test]
async fn installs_vanilla_pack_as_new_instance() {
    let env = env(b"mod b").await;
    let info = env.installer.inspect(&env.pack).await.unwrap().info;
    assert_eq!((info.name.as_str(), info.file_count), ("Test Pack", 2));

    let ctx = TaskCtx::noop("modpack_install");
    let config = env
        .installer
        .install(&env.pack, Some("  "), &ctx)
        .await
        .unwrap();
    assert_eq!(config.name, "Test Pack");
    assert_eq!(config.game_version, "fake");
    assert!(config.loader.is_none());

    let game = env.paths.instance_game_dir(&config.id);
    let read = |rel: &str| std::fs::read_to_string(game.join(rel)).unwrap();
    assert_eq!(read("mods/a.jar"), "mod a");
    assert_eq!(read("mods/b.jar"), "mod b");
    assert_eq!(read("config/a.toml"), "common");
    assert_eq!(read("options.txt"), "lang:zh_cn");
    assert!(!game.join("other").exists());
    assert!(
        !env.paths
            .instance_dir(&config.id)
            .join("escape.txt")
            .exists()
    );
    assert!(
        !game
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("escape.txt")
            .exists()
    );
    assert!(
        env.paths
            .shared_versions_dir()
            .join("fake/fake.json")
            .is_file()
    );

    let named = env
        .installer
        .install(&env.pack, Some("My Pack"), &ctx)
        .await
        .unwrap();
    assert_eq!(named.name, "My Pack");
    assert_eq!(instance_count(&env.paths), 2);
}

#[tokio::test]
async fn installs_loader_and_records_it_on_instance() {
    let env = env_with(
        b"mod b",
        json!({"minecraft": "fake", "fabric-loader": "0.16.5"}),
    )
    .await;
    let profile = json!({
        "id": "fabric-loader-0.16.5-fake", "inheritsFrom": "fake", "type": "release",
        "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "arguments": {"game": [], "jvm": []}, "libraries": [],
    });
    env.server.route(
        "/v2/versions/loader/fake/0.16.5/profile/json",
        profile.to_string().into_bytes(),
    );
    let ctx = TaskCtx::noop("modpack_install");
    let config = env.installer.install(&env.pack, None, &ctx).await.unwrap();
    let loader = config.loader.unwrap();
    assert_eq!(
        (loader.kind, loader.version.as_str()),
        (LoaderKind::Fabric, "0.16.5")
    );
    let db = Db::open(&env.paths.db_file()).unwrap();
    let saved = InstanceStore::new(&env.paths, &db).get(&config.id).unwrap();
    assert_eq!(saved.loader.unwrap().kind, LoaderKind::Fabric);
    let id = "fabric-loader-0.16.5-fake";
    assert!(
        env.paths
            .shared_versions_dir()
            .join(id)
            .join(format!("{id}.json"))
            .is_file()
    );
}

#[tokio::test]
async fn missing_loader_version_rolls_back_new_instance() {
    let env = env_with(
        b"mod b",
        json!({"minecraft": "fake", "fabric-loader": "9.9.9"}),
    )
    .await;
    let ctx = TaskCtx::noop("modpack_install");
    assert!(env.installer.install(&env.pack, None, &ctx).await.is_err());
    assert_eq!(instance_count(&env.paths), 0);
}

#[tokio::test]
async fn hash_mismatch_rolls_back_new_instance() {
    let env = env(b"different").await;
    let ctx = TaskCtx::noop("modpack_install");
    assert!(env.installer.install(&env.pack, None, &ctx).await.is_err());
    assert_eq!(instance_count(&env.paths), 0);
    let left = std::fs::read_dir(env.paths.instances_dir())
        .unwrap()
        .count();
    assert_eq!(left, 0);
    assert_eq!(
        std::fs::read_dir(env.paths.trash_dir()).map_or(0, Iterator::count),
        0
    );
}

#[tokio::test]
async fn cancelled_install_rolls_back_new_instance() {
    let env = env(b"mod b").await;
    env.server.fail("/mods/b.jar", [Fault::Hang]);
    let ctx = TaskCtx::noop("modpack_install");
    let cancel = ctx.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        cancel.cancel();
    });
    let err = env
        .installer
        .install(&env.pack, None, &ctx)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::CommonCancelled);
    assert_eq!(instance_count(&env.paths), 0);
}

#[tokio::test]
async fn invalid_pack_creates_nothing() {
    let env = env(b"mod b").await;
    std::fs::write(&env.pack, b"not a zip").unwrap();
    let ctx = TaskCtx::noop("modpack_install");
    let err = env
        .installer
        .install(&env.pack, None, &ctx)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourcesModpackInvalid);
    assert_eq!(instance_count(&env.paths), 0);
}
