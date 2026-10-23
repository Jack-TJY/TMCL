//! M1-09 手测示例：列出并安装模组加载器。
//!
//! ```text
//! cargo run -p tmcl-loader --example install_loader -- <fabric|quilt> <游戏版本> [加载器版本] [--data <数据目录>]
//! ```
//!
//! 不给加载器版本时安装推荐版本。安装完成后打印版本号，可交给
//! `cargo run -p tmcl-launch --example launch_vanilla -- <版本号> --java <路径> --data <数据目录>` 启动。

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use tmcl_common::{AppResult, DataPaths, ProgressSink, TaskCtx, TaskProgress, ThrottledSink};
use tmcl_loader::{LoaderInstaller, LoaderKind, display_name, fabric, list_versions, quilt};
use tmcl_net::{DownloadOptions, Downloader, HttpClient};

struct PrintSink;

impl ProgressSink for PrintSink {
    fn report(&self, p: &TaskProgress) {
        eprintln!("[{}] {}/{}", p.stage, p.done, p.total);
    }
}

async fn run() -> AppResult<()> {
    let mut positional = Vec::new();
    let mut data = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--data" => data = it.next().map(PathBuf::from),
            _ => positional.push(arg),
        }
    }
    let (kind, game) = match positional.as_slice() {
        [kind, game, ..] => (kind.as_str(), game.as_str()),
        _ => {
            eprintln!(
                "用法：install_loader <fabric|quilt> <游戏版本> [加载器版本] [--data <目录>]"
            );
            std::process::exit(2);
        }
    };
    let kind = match kind {
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        other => {
            eprintln!("不支持的加载器：{other}");
            std::process::exit(2);
        }
    };
    let paths = match data {
        Some(dir) => DataPaths::at(dir),
        None => DataPaths::resolve()?,
    };
    paths.ensure_dirs()?;
    let downloader = Downloader::new(HttpClient::new()?, DownloadOptions::default());
    let installer = LoaderInstaller::new(downloader.clone(), paths);
    let ctx = TaskCtx::new(
        "install_loader",
        Arc::new(ThrottledSink::with_interval(
            PrintSink,
            Duration::from_secs(1),
        )),
    );

    let loader = match positional.get(2) {
        Some(v) => v.clone(),
        None => {
            let versions =
                list_versions(&downloader, installer.endpoints(), kind, game, &ctx).await?;
            eprintln!("共 {} 个版本", versions.len());
            versions
                .into_iter()
                .find(|v| v.recommended)
                .map(|v| v.version)
                .unwrap_or_default()
        }
    };
    eprintln!("安装 {} {loader}", display_name(kind));
    let installed = match kind {
        LoaderKind::Fabric => fabric::install(&installer, game, &loader, &ctx).await?,
        _ => quilt::install(&installer, game, &loader, &ctx).await?,
    };
    eprintln!("{:?}", installed.report);
    println!("{}", installed.version_id);
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("失败：{e}");
            ExitCode::FAILURE
        }
    }
}
