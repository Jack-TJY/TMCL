//! EasyTier 下载、校验与解压（本地 HTTP 服务器，离线）。

use std::io::Write;

use sha2::{Digest, Sha256};
use tmcl_common::{DataPaths, ErrorCode, TaskCtx};
use tmcl_net::testing::TestServer;
use tmcl_net::{DownloadOptions, Downloader, HttpClient, RetryPolicy};
use tmcl_p2p::easytier::{self, EasyTierAsset, install_dir, installed};
use zip::write::SimpleFileOptions;

fn make_zip(entries: &[&str]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in entries {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(name.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn downloader() -> Downloader {
    Downloader::new(
        HttpClient::new().unwrap(),
        DownloadOptions {
            retry: RetryPolicy::none(),
            ..DownloadOptions::default()
        },
    )
}

fn asset(server: &TestServer, body: &[u8], sha256: String) -> EasyTierAsset {
    EasyTierAsset {
        platform: "linux-x86_64".to_owned(),
        url: server.url("/et.zip"),
        sha256,
        size: body.len() as u64,
    }
}

#[tokio::test]
async fn installs_once_and_reuses() {
    let body = make_zip(&[
        "easytier-linux-x86_64/easytier-core",
        "easytier-linux-x86_64/easytier-cli",
        "easytier-linux-x86_64/easytier-web",
    ]);
    let server = TestServer::start([("/et.zip", body.clone())]).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let asset = asset(&server, &body, sha256(&body));
    let ctx = TaskCtx::noop("test");

    let bins = easytier::install(&downloader(), &paths, &asset, &ctx)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(&bins.core).unwrap(),
        b"easytier-linux-x86_64/easytier-core"
    );
    assert!(bins.cli.is_file());
    assert!(!install_dir(&paths).join("easytier-web").exists());
    assert_eq!(installed(&paths, &asset), Some(bins.clone()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&bins.core).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
    }
    // 压缩包是中间产物，安装后删除。
    assert_eq!(std::fs::read_dir(paths.cache_dir()).unwrap().count(), 0);

    let again = easytier::install(&downloader(), &paths, &asset, &ctx)
        .await
        .unwrap();
    assert_eq!(again, bins);
    assert_eq!(server.hits(), 1);
}

#[tokio::test]
async fn rejects_hash_mismatch() {
    let body = make_zip(&[
        "easytier-linux-x86_64/easytier-core",
        "easytier-linux-x86_64/easytier-cli",
    ]);
    let server = TestServer::start([("/et.zip", body.clone())]).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let asset = asset(&server, &body, "0".repeat(64));

    let err = easytier::install(&downloader(), &paths, &asset, &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::P2pBinaryInvalid);
    assert!(!install_dir(&paths).exists());
    assert_eq!(installed(&paths, &asset), None);
    assert_eq!(std::fs::read_dir(paths.cache_dir()).unwrap().count(), 0);
}

#[tokio::test]
async fn rejects_archive_missing_files() {
    let body = make_zip(&["easytier-linux-x86_64/easytier-core"]);
    let server = TestServer::start([("/et.zip", body.clone())]).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = DataPaths::at(dir.path());
    let asset = asset(&server, &body, sha256(&body));

    let err = easytier::install(&downloader(), &paths, &asset, &TaskCtx::noop("test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::P2pBinaryInvalid);
    assert_eq!(
        err.params.get("file").map(String::as_str),
        Some("easytier-cli")
    );
    assert_eq!(installed(&paths, &asset), None);
}

#[test]
fn current_platform_has_pinned_asset() {
    if let Ok(asset) = EasyTierAsset::current() {
        assert_eq!(asset.sha256.len(), 64);
        assert!(
            asset
                .url
                .starts_with("https://github.com/EasyTier/EasyTier/releases/download/")
        );
        assert!(asset.url.ends_with(&format!(
            "{}-{}.zip",
            asset.platform,
            easytier::EASYTIER_VERSION
        )));
    }
}
