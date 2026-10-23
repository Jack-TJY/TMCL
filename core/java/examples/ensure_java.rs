//! M1-05 示例：为指定主版本找到或下载 Java。
//!
//! ```text
//! cargo run -p tmcl-java --example ensure_java -- 17 [--component java-runtime-gamma] [--data <数据目录>] [--list]
//! ```
//!
//! 先探测本机 Java，没有合适的就下载到 `<数据目录>/java/`，最后打印选中的 Java。
//! `--list` 只列出探测到的 Java。不给 `--data` 时使用 `DataPaths::resolve()`。

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use tmcl_common::{AppResult, DataPaths, ProgressSink, TaskCtx, TaskProgress, ThrottledSink};
use tmcl_java::{RuntimeInstaller, Sources, detect, ensure};
use tmcl_net::{DownloadOptions, Downloader, HttpClient};

struct Args {
    major: u32,
    component: Option<String>,
    data: Option<PathBuf>,
    list: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut major = None;
    let mut component = None;
    let mut data = None;
    let mut list = false;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--component" => component = it.next(),
            "--data" => data = it.next().map(PathBuf::from),
            "--list" => list = true,
            other if major.is_none() && !other.starts_with("--") => {
                major = Some(
                    other
                        .parse()
                        .map_err(|_| format!("主版本号无效：{other}"))?,
                );
            }
            other => return Err(format!("未知参数：{other}")),
        }
    }
    Ok(Args {
        major: major.unwrap_or(17),
        component,
        data,
        list,
    })
}

/// 把进度打印到 stderr。
struct PrintSink;

impl ProgressSink for PrintSink {
    fn report(&self, p: &TaskProgress) {
        eprintln!(
            "[{}] {}/{} 个文件，{:.1}/{:.1} MB，{:.1} MB/s",
            p.stage,
            p.done,
            p.total,
            p.bytes_done as f64 / 1e6,
            p.bytes_total as f64 / 1e6,
            p.speed_bps as f64 / 1e6
        );
    }
}

async fn run(args: Args) -> AppResult<()> {
    let paths = match args.data {
        Some(dir) => DataPaths::at(dir),
        None => DataPaths::resolve()?,
    };
    paths.ensure_dirs()?;
    let sources = Sources::system(&paths.java_dir());
    if args.list {
        for install in detect(&sources).await {
            println!("{install:?}");
        }
        return Ok(());
    }

    let downloader = Downloader::new(HttpClient::new()?, DownloadOptions::default());
    let installer = RuntimeInstaller::new(downloader, paths.java_dir());
    let ctx = TaskCtx::new(
        "java_install",
        Arc::new(ThrottledSink::with_interval(
            PrintSink,
            Duration::from_secs(1),
        )),
    );
    let install = ensure(
        &installer,
        &sources,
        args.major,
        args.component.as_deref(),
        &ctx,
    )
    .await?;
    println!("{install:?}");
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("失败：{e} {:?}", e.params);
            ExitCode::FAILURE
        }
    }
}
