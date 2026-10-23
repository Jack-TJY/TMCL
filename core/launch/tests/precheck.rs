//! 启动前检查的离线测试：用不需要下载任何文件的自定义版本。

use tmcl_common::{DataPaths, ErrorCode, TaskCtx};
use tmcl_game::InstallOptions;
use tmcl_instance::{InstanceConfig, JavaSetting, LoaderKind, LoaderSpec};
use tmcl_java::RuntimeInstaller;
use tmcl_launch::{LaunchAccount, LaunchEnv, prepare};
use tmcl_net::{DownloadOptions, Downloader, HttpClient};

struct Fixture {
    _dir: tempfile::TempDir,
    paths: DataPaths,
    downloader: Downloader,
    java: RuntimeInstaller,
    install: InstallOptions,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path());
        paths.ensure_dirs().unwrap();
        // 没有库、资源与下载项的自定义版本：校验无需任何文件。
        let version_dir = paths.shared_versions_dir().join("custom");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join("custom.json"),
            r#"{"id":"custom","type":"release","mainClass":"Main","libraries":[]}"#,
        )
        .unwrap();
        let downloader = Downloader::new(HttpClient::new().unwrap(), DownloadOptions::default());
        let java = RuntimeInstaller::new(downloader.clone(), paths.java_dir());
        Self {
            _dir: dir,
            paths,
            downloader,
            java,
            install: InstallOptions::default(),
        }
    }

    fn env(&self) -> LaunchEnv<'_> {
        LaunchEnv {
            paths: &self.paths,
            downloader: &self.downloader,
            java: &self.java,
            install: &self.install,
        }
    }

    fn natives_dirs(&self) -> usize {
        std::fs::read_dir(self.paths.root().join("cache").join("natives"))
            .map(|d| d.count())
            .unwrap_or(0)
    }
}

fn account() -> LaunchAccount {
    LaunchAccount::offline("Dev", "380df991f603344ca090369bad2a924a")
}

#[tokio::test]
async fn loader_instance_without_loader_version_is_not_installed() {
    let f = Fixture::new();
    let loader = LoaderSpec {
        kind: LoaderKind::Fabric,
        version: "0.16.5".into(),
    };
    let instance = InstanceConfig::new("x", "1.20.1", Some(loader)).unwrap();
    let err = prepare(f.env(), &instance, account(), &TaskCtx::noop("launch"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::LaunchVersionNotInstalled);
    assert_eq!(err.params["loader"], "fabric");
}

#[tokio::test]
async fn unusable_manual_java_fails_before_natives_are_created() {
    let f = Fixture::new();
    let mut instance = InstanceConfig::new("x", "custom", None).unwrap();
    instance.java = JavaSetting::Path {
        path: f.paths.root().join("no-java").display().to_string(),
    };
    let err = prepare(f.env(), &instance, account(), &TaskCtx::noop("launch"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::JavaProbeFailed);
    assert_eq!(f.natives_dirs(), 0);
}

#[tokio::test]
async fn cancelled_task_stops_preparation() {
    let f = Fixture::new();
    let instance = InstanceConfig::new("x", "custom", None).unwrap();
    let ctx = TaskCtx::noop("launch");
    ctx.cancel.cancel();
    let err = prepare(f.env(), &instance, account(), &ctx)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::CommonCancelled);
    assert_eq!(f.natives_dirs(), 0);
}
