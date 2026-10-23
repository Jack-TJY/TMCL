//! 进程管理的测试：以本测试程序自身作为“游戏”，避免依赖本机 Java。
//!
//! 子进程运行被忽略的 `child_game` 测试，按工作目录里的 `mode.txt` 决定行为。

use std::path::Path;
use std::time::{Duration, Instant};

use tmcl_common::ErrorCode;
use tmcl_launch::{ExitInfo, LaunchCommand, LogStream, spawn};

const TOKEN: &str = "secret-token-value";

/// 子进程入口：仅在父测试以 `--ignored --exact child_game` 启动时运行。
#[test]
#[ignore = "由其他测试作为子进程启动"]
fn child_game() {
    let mode = std::fs::read_to_string("mode.txt").unwrap_or_default();
    match mode.trim() {
        "print" => {
            println!("Setting user: Dev, session {TOKEN}");
            eprintln!("warn line");
            println!("done");
        }
        "crash" => println!("---- Minecraft Crash Report ----"),
        "fail" => std::process::exit(3),
        "sleep" => {
            println!("sleeping");
            std::thread::sleep(Duration::from_secs(60));
        }
        _ => {}
    }
}

fn game(dir: &Path, mode: &str) -> LaunchCommand {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("mode.txt"), mode).unwrap();
    LaunchCommand {
        program: std::env::current_exe().unwrap(),
        args: [
            "--ignored",
            "--exact",
            "child_game",
            "--nocapture",
            "--quiet",
        ]
        .map(String::from)
        .to_vec(),
        cwd: dir.to_path_buf(),
        secrets: vec![TOKEN.to_owned()],
    }
}

async fn run(command: &LaunchCommand) -> (Vec<(LogStream, String)>, ExitInfo) {
    let process = spawn(command).unwrap();
    assert!(process.pid() > 0);
    let mut lines = Vec::new();
    let exit = process
        .run(Duration::from_millis(50), |stream, batch| {
            lines.extend(batch.into_iter().map(|l| (stream, l)));
        })
        .await
        .unwrap();
    (lines, exit)
}

#[tokio::test]
async fn forwards_output_with_tokens_masked() {
    let dir = tempfile::tempdir().unwrap();
    let (lines, exit) = run(&game(&dir.path().join("game"), "print")).await;
    assert_eq!(exit.exit_code, Some(0));
    assert!(!exit.crashed && !exit.killed);
    assert!(lines.contains(&(LogStream::Stdout, "Setting user: Dev, session ***".into())));
    assert!(lines.contains(&(LogStream::Stderr, "warn line".into())));
    assert!(!lines.iter().any(|(_, l)| l.contains(TOKEN)), "{lines:?}");
}

#[tokio::test]
async fn nonzero_exit_or_crash_report_is_crash() {
    let dir = tempfile::tempdir().unwrap();
    let (_, exit) = run(&game(dir.path(), "fail")).await;
    assert_eq!(exit.exit_code, Some(3));
    assert!(exit.crashed);

    let (lines, exit) = run(&game(dir.path(), "crash")).await;
    assert_eq!(exit.exit_code, Some(0), "{lines:?}");
    assert!(exit.crashed, "日志出现崩溃报告即视为崩溃");
}

#[tokio::test]
async fn kill_ends_process_and_is_not_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let process = spawn(&game(dir.path(), "sleep")).unwrap();
    let killer = process.kill_handle();
    let started = Instant::now();
    let exit = process
        .run(Duration::from_millis(50), |_, batch| {
            if batch.iter().any(|l| l == "sleeping") {
                killer.kill();
            }
        })
        .await
        .unwrap();
    assert!(exit.killed);
    assert!(!exit.crashed);
    assert_ne!(exit.exit_code, Some(0));
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[tokio::test]
async fn missing_java_is_spawn_failure() {
    let dir = tempfile::tempdir().unwrap();
    let mut command = game(dir.path(), "");
    command.program = dir.path().join("no-java");
    let err = spawn(&command).unwrap_err();
    assert_eq!(err.code, ErrorCode::LaunchSpawnFailed);
    assert!(err.params.contains_key("java"));
}
