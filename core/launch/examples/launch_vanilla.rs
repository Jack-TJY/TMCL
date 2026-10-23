//! M0-09 端到端示例：下载原版并用离线账号启动。
//!
//! ```text
//! cargo run -p tmcl-launch --example launch_vanilla -- 1.20.1 --java <java 路径> --name Dev [--data <数据目录>]
//! ```
//!
//! 不给 `--data` 时使用 `DataPaths::resolve()`，即示例可执行文件旁的 `tmcl-data/`。
//! 离线账号仅限本地测试。

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use tmcl_auth::OfflineAccount;
use tmcl_common::{AppResult, DataPaths, ProgressSink, TaskCtx, TaskProgress, ThrottledSink};
use tmcl_game::{GameFiles, InstallOptions, install_version};
use tmcl_launch::{LaunchAccount, LaunchSpec, LogStream, build_command, spawn};
use tmcl_net::{DownloadOptions, Downloader, HttpClient};

struct Args {
    version: String,
    java: PathBuf,
    name: String,
    data: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut version = None;
    let mut java = None;
    let mut name = None;
    let mut data = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--java" => java = it.next().map(PathBuf::from),
            "--name" => name = it.next(),
            "--data" => data = it.next().map(PathBuf::from),
            other if version.is_none() && !other.starts_with("--") => version = Some(arg),
            other => return Err(format!("未知参数：{other}")),
        }
    }
    Ok(Args {
        version: version.ok_or("缺少版本号")?,
        java: java.ok_or("缺少 --java")?,
        name: name.unwrap_or_else(|| "Dev".to_owned()),
        data,
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

async fn run(args: Args) -> AppResult<i32> {
    let account = OfflineAccount::new(&args.name)?;
    let paths = match args.data {
        Some(dir) => DataPaths::at(dir),
        None => DataPaths::resolve()?,
    };
    paths.ensure_dirs()?;
    eprintln!("数据目录：{}", paths.root().display());

    let downloader = Downloader::new(HttpClient::new()?, DownloadOptions::default());
    let sink = ThrottledSink::with_interval(PrintSink, Duration::from_secs(1));
    let ctx = TaskCtx::new("game_install", Arc::new(sink));
    let report = install_version(
        &downloader,
        &paths,
        &args.version,
        &InstallOptions::default(),
        &ctx,
    )
    .await?;
    eprintln!(
        "补全完成：下载 {} 个（{:.1} MB），跳过 {} 个",
        report.downloaded,
        report.bytes as f64 / 1e6,
        report.skipped
    );

    let files = GameFiles::new(paths.clone());
    let instance_id = format!("dev-{}", args.version);
    let spec = LaunchSpec {
        java: args.java,
        version: files.load_version(&args.version)?,
        game_dir: paths.instance_game_dir(&instance_id),
        natives_dir: paths.cache_dir().join("natives").join(&args.version),
        account: LaunchAccount {
            name: account.name.clone(),
            uuid: account.uuid.simple().to_string(),
            access_token: "0".to_owned(),
            user_type: "legacy".to_owned(),
        },
        min_memory_mb: 512,
        max_memory_mb: 2048,
        window: None,
    };
    let command = build_command(&files, &spec);
    eprintln!("启动命令：{}", command.redacted());

    let process = spawn(&command)?;
    eprintln!("游戏进程 PID {}", process.pid());
    let exit = process
        .wait_with_logs(|stream, line| match stream {
            LogStream::Stdout => println!("{line}"),
            LogStream::Stderr => eprintln!("{line}"),
        })
        .await?;
    eprintln!("游戏退出：{:?}，崩溃：{}", exit.exit_code, exit.crashed);
    Ok(exit.exit_code.unwrap_or(1))
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!(
                "用法：launch_vanilla <版本> --java <java 路径> [--name <用户名>] [--data <数据目录>]"
            );
            return ExitCode::from(2);
        }
    };
    match run(args).await {
        Ok(0) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("失败：{e}（params: {:?}）", e.params);
            ExitCode::FAILURE
        }
    }
}
