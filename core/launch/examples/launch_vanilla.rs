//! 端到端示例：准备（补全文件、选择 Java、解压 natives）并用离线账号启动原版或已安装的加载器版本。
//!
//! ```text
//! cargo run -p tmcl-launch --example launch_vanilla -- 1.20.1 [--loader <种类>:<版本>] [--java <java 路径>] [--name Dev] [--data <数据目录>] [--kill-after <秒>]
//! ```
//!
//! 不给 `--java` 时自动选择（没有合适的就下载运行时）；不给 `--data` 时使用
//! `DataPaths::resolve()`，即示例可执行文件旁的 `tmcl-data/`。`--loader forge:47.2.0` 等需先用
//! `tmcl-loader` 的 `install_loader` 示例安装。离线账号仅限本地测试。

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use tmcl_auth::OfflineAccount;
use tmcl_common::{AppResult, DataPaths, ProgressSink, TaskCtx, TaskProgress, ThrottledSink};
use tmcl_game::InstallOptions;
use tmcl_instance::{InstanceConfig, JavaSetting, LoaderKind, LoaderSpec};
use tmcl_java::RuntimeInstaller;
use tmcl_launch::{LaunchAccount, LaunchEnv, LogStream, prepare, spawn, version_id};
use tmcl_net::{DownloadOptions, Downloader, HttpClient};

struct Args {
    version: String,
    loader: Option<LoaderSpec>,
    java: Option<String>,
    name: String,
    data: Option<PathBuf>,
    kill_after: Option<u64>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        version: String::new(),
        loader: None,
        java: None,
        name: "Dev".to_owned(),
        data: None,
        kill_after: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--java" => args.java = it.next(),
            "--loader" => {
                let spec = it.next().ok_or("缺少加载器")?;
                let (kind, version) = spec.split_once(':').ok_or("加载器格式为 <种类>:<版本>")?;
                let kind = match kind {
                    "fabric" => LoaderKind::Fabric,
                    "quilt" => LoaderKind::Quilt,
                    "forge" => LoaderKind::Forge,
                    "neoforge" => LoaderKind::Neoforge,
                    other => return Err(format!("未知的加载器：{other}")),
                };
                args.loader = Some(LoaderSpec {
                    kind,
                    version: version.to_owned(),
                });
            }
            "--name" => args.name = it.next().ok_or("缺少用户名")?,
            "--data" => args.data = it.next().map(PathBuf::from),
            "--kill-after" => {
                args.kill_after = it.next().and_then(|s| s.parse().ok());
            }
            other if args.version.is_empty() && !other.starts_with("--") => args.version = arg,
            other => return Err(format!("未知参数：{other}")),
        }
    }
    if args.version.is_empty() {
        return Err("缺少版本号".to_owned());
    }
    Ok(args)
}

/// 把进度打印到 stderr。
struct PrintSink;

impl ProgressSink for PrintSink {
    fn report(&self, p: &TaskProgress) {
        eprintln!(
            "[{}] {}/{}，{:.1}/{:.1} MB",
            p.stage,
            p.done,
            p.total,
            p.bytes_done as f64 / 1e6,
            p.bytes_total as f64 / 1e6,
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
    let java = RuntimeInstaller::new(downloader.clone(), paths.java_dir());
    let install = InstallOptions::default();
    let env = LaunchEnv {
        paths: &paths,
        downloader: &downloader,
        java: &java,
        install: &install,
    };
    let mut instance = InstanceConfig::new(
        &format!("Dev {}", args.version),
        &args.version,
        args.loader.clone(),
    )?;
    instance.id = format!("dev-{}", version_id(&instance));
    instance.memory.min_mb = 512;
    instance.memory.max_mb = 2048;
    if let Some(path) = args.java {
        instance.java = JavaSetting::Path { path };
    }

    let sink = ThrottledSink::with_interval(PrintSink, Duration::from_secs(1));
    let ctx = TaskCtx::new("launch", Arc::new(sink));
    let account = LaunchAccount::offline(&account.name, &account.uuid.simple().to_string());
    let prepared = prepare(env, &instance, account, &ctx).await?;
    eprintln!(
        "补全 {} 个文件；Java {}（{}）",
        prepared.repaired,
        prepared.java.version,
        prepared.java.path.display()
    );
    for w in &prepared.warnings {
        eprintln!("警告：{w}");
    }
    eprintln!("启动命令：{}", prepared.command.redacted());

    let process = spawn(&prepared.command)?;
    eprintln!("游戏进程 PID {}", process.pid());
    if let Some(secs) = args.kill_after {
        let killer = process.kill_handle();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(secs)).await;
            eprintln!("强制结束");
            killer.kill();
        });
    }
    let exit = process
        .run(Duration::from_millis(100), |stream, lines| {
            for line in lines {
                match stream {
                    LogStream::Stdout => println!("{line}"),
                    LogStream::Stderr => eprintln!("{line}"),
                }
            }
        })
        .await?;
    eprintln!(
        "游戏退出：{:?}，崩溃：{}，强制结束：{}",
        exit.exit_code, exit.crashed, exit.killed
    );
    let _ = std::fs::remove_dir_all(&prepared.natives_dir);
    Ok(exit.exit_code.unwrap_or(1))
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!(
                "用法：launch_vanilla <版本> [--java <java 路径>] [--name <用户名>] [--data <数据目录>] [--kill-after <秒>]"
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
