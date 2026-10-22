//! 进程管理的测试：用 cargo 自身代替 Java，避免依赖本机环境。

use std::path::PathBuf;

use tmcl_common::ErrorCode;
use tmcl_launch::{LaunchCommand, LogStream, spawn};

fn command(program: PathBuf, args: &[&str], cwd: PathBuf) -> LaunchCommand {
    LaunchCommand {
        program,
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd,
    }
}

#[tokio::test]
async fn forwards_output_and_reports_exit() {
    let cargo = PathBuf::from(std::env::var("CARGO").unwrap());
    let dir = tempfile::tempdir().unwrap();
    let process = spawn(&command(
        cargo.clone(),
        &["--version"],
        dir.path().join("game"),
    ))
    .unwrap();
    assert!(process.pid() > 0);
    let mut stdout = Vec::new();
    let exit = process
        .wait_with_logs(|stream, line| {
            if stream == LogStream::Stdout {
                stdout.push(line.to_owned());
            }
        })
        .await
        .unwrap();
    assert_eq!(exit.exit_code, Some(0));
    assert!(!exit.crashed);
    assert!(stdout.iter().any(|l| l.starts_with("cargo ")), "{stdout:?}");
    assert!(dir.path().join("game").is_dir(), "工作目录应被创建");

    let process = spawn(&command(cargo, &["--no-such-flag"], dir.path().into())).unwrap();
    let exit = process.wait_with_logs(|_, _| {}).await.unwrap();
    assert!(exit.crashed);
}

#[tokio::test]
async fn missing_java_is_spawn_failure() {
    let dir = tempfile::tempdir().unwrap();
    let err = spawn(&command(dir.path().join("no-java"), &[], dir.path().into())).unwrap_err();
    assert_eq!(err.code, ErrorCode::LaunchSpawnFailed);
    assert!(err.params.contains_key("java"));
}
