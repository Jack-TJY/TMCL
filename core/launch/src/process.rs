//! 游戏进程：启动、按行批量转发 stdout/stderr、崩溃识别与强制结束。

use std::process::Stdio;
use std::time::Duration;

use tmcl_common::{AppError, AppResult, ErrorCode};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::args::{LaunchCommand, mask};

/// 日志来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    /// 标准输出。
    Stdout,
    /// 标准错误。
    Stderr,
}

/// 游戏退出信息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitInfo {
    /// 退出码；被信号结束时为 `None`。
    pub exit_code: Option<i32>,
    /// 是否视为崩溃：未被强制结束，且退出码非 0 或日志中出现崩溃特征（见 [`is_crash_line`]）。
    pub crashed: bool,
    /// 是否经 [`KillHandle::kill`] 强制结束。
    pub killed: bool,
}

/// 运行中的游戏进程。
#[derive(Debug)]
pub struct GameProcess {
    child: Child,
    pid: u32,
    lines: mpsc::UnboundedReceiver<(LogStream, String)>,
    kill: CancellationToken,
    secrets: Vec<String>,
}

/// 强制结束游戏进程的句柄，可在其他任务中使用；`Default` 得到不关联进程的句柄（测试用）。
#[derive(Debug, Clone, Default)]
pub struct KillHandle(CancellationToken);

impl KillHandle {
    /// 请求强制结束；进程已退出时不做任何事。
    pub fn kill(&self) {
        self.0.cancel();
    }

    /// 是否已请求强制结束。
    pub fn is_killed(&self) -> bool {
        self.0.is_cancelled()
    }
}

/// 启动游戏进程；Java 无法执行时返回 `LAUNCH_SPAWN_FAILED`。
pub fn spawn(command: &LaunchCommand) -> AppResult<GameProcess> {
    std::fs::create_dir_all(&command.cwd).map_err(|e| {
        AppError::new(ErrorCode::CommonIo)
            .with_param("path", command.cwd.display())
            .with_detail(e.to_string())
    })?;
    let mut cmd = Command::new(&command.program);
    cmd.args(&command.args)
        .current_dir(&command.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false);
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW：不为 java.exe 额外弹出控制台窗口。
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| {
        AppError::new(ErrorCode::LaunchSpawnFailed)
            .with_param("java", command.program.display())
            .with_detail(e.to_string())
    })?;
    let pid = child.id().unwrap_or(0);
    let (tx, lines) = mpsc::unbounded_channel();
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(forward(stdout, LogStream::Stdout, tx.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(forward(stderr, LogStream::Stderr, tx));
    }
    Ok(GameProcess {
        child,
        pid,
        lines,
        kill: CancellationToken::new(),
        secrets: command.secrets.clone(),
    })
}

/// 日志行是否带有崩溃特征：游戏崩溃报告、JVM 致命错误、主线程未捕获异常、JVM 无法创建或内存不足。
pub fn is_crash_line(line: &str) -> bool {
    const SIGNATURES: &[&str] = &[
        "---- Minecraft Crash Report ----",
        "#@!@# Game crashed!",
        "A fatal error has been detected by the Java Runtime Environment",
        "Exception in thread \"main\"",
        "Could not create the Java Virtual Machine",
        "java.lang.OutOfMemoryError",
    ];
    SIGNATURES.iter().any(|s| line.contains(s))
}

impl GameProcess {
    /// 进程 ID。
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// 强制结束句柄。
    pub fn kill_handle(&self) -> KillHandle {
        KillHandle(self.kill.clone())
    }

    /// 运行到进程退出：日志按来源分批交给 `on_batch`（同一来源的连续行最多攒 `interval`），
    /// 访问令牌替换为 `***`；期间响应 [`KillHandle::kill`]。
    pub async fn run(
        mut self,
        interval: Duration,
        mut on_batch: impl FnMut(LogStream, Vec<String>),
    ) -> AppResult<ExitInfo> {
        let mut pending: Option<(LogStream, Vec<String>)> = None;
        let mut flush = |pending: &mut Option<(LogStream, Vec<String>)>| {
            if let Some((stream, lines)) = pending.take() {
                on_batch(stream, lines);
            }
        };
        let mut ticker = tokio::time::interval(interval);
        let mut killed = false;
        let mut crash_seen = false;
        let mut streams_open = true;
        let status = loop {
            tokio::select! {
                _ = self.kill.cancelled(), if !killed => {
                    killed = true;
                    // 进程可能恰好已退出，start_kill 失败无需处理。
                    let _ = self.child.start_kill();
                }
                line = self.lines.recv(), if streams_open => match line {
                    Some((stream, line)) => {
                        crash_seen |= is_crash_line(&line);
                        let line = mask(&line, &self.secrets);
                        match &mut pending {
                            Some((s, lines)) if *s == stream => lines.push(line),
                            _ => {
                                flush(&mut pending);
                                pending = Some((stream, vec![line]));
                            }
                        }
                    }
                    None => streams_open = false,
                },
                _ = ticker.tick() => flush(&mut pending),
                status = self.child.wait(), if !streams_open => break status?,
            }
        };
        flush(&mut pending);
        let exit_code = status.code();
        Ok(ExitInfo {
            exit_code,
            crashed: !killed && (exit_code != Some(0) || crash_seen),
            killed,
        })
    }
}

async fn forward(
    reader: impl AsyncRead + Unpin,
    stream: LogStream,
    tx: mpsc::UnboundedSender<(LogStream, String)>,
) {
    // 按字节读取再有损解码：Windows 上 Java 可能以 GBK 等本地编码输出，
    // 不能因为非 UTF-8 而停止读取，否则管道写满会卡住游戏。
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf);
                let line = line.trim_end_matches(['\r', '\n']).to_owned();
                // 接收端已丢弃时继续读取并丢弃，保证管道不被写满。
                let _ = tx.send((stream, line));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_crash_signatures() {
        assert!(is_crash_line("---- Minecraft Crash Report ----"));
        assert!(is_crash_line(
            "#@!@# Game crashed! Crash report saved to: #@!@# C:\\x.txt"
        ));
        assert!(is_crash_line(
            "Exception in thread \"main\" java.lang.NoClassDefFoundError: x"
        ));
        assert!(is_crash_line(
            "Error: Could not create the Java Virtual Machine."
        ));
        assert!(!is_crash_line("[Render thread/INFO]: Sound engine started"));
        assert!(!is_crash_line("[main/WARN]: Exception loading blockstate"));
    }
}
