//! 游戏进程：启动、按行转发 stdout/stderr、等待退出与强制结束。

use std::process::Stdio;

use tmcl_common::{AppError, AppResult, ErrorCode};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::args::LaunchCommand;

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
    /// 是否视为崩溃（退出码非 0 或被信号结束）。日志崩溃特征识别由 M1-07 补充。
    pub crashed: bool,
}

/// 运行中的游戏进程。
#[derive(Debug)]
pub struct GameProcess {
    child: Child,
    pid: u32,
    lines: mpsc::UnboundedReceiver<(LogStream, String)>,
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
    Ok(GameProcess { child, pid, lines })
}

impl GameProcess {
    /// 进程 ID。
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// 读取下一行日志；两个输出流都关闭后返回 `None`。
    pub async fn next_line(&mut self) -> Option<(LogStream, String)> {
        self.lines.recv().await
    }

    /// 转发全部日志直到进程退出，返回退出信息。
    pub async fn wait_with_logs(
        mut self,
        mut on_line: impl FnMut(LogStream, &str),
    ) -> AppResult<ExitInfo> {
        while let Some((stream, line)) = self.lines.recv().await {
            on_line(stream, &line);
        }
        self.wait().await
    }

    /// 等待进程退出（不读取剩余日志）。
    pub async fn wait(&mut self) -> AppResult<ExitInfo> {
        let status = self.child.wait().await?;
        let exit_code = status.code();
        Ok(ExitInfo {
            exit_code,
            crashed: exit_code != Some(0),
        })
    }

    /// 强制结束进程。
    pub async fn kill(&mut self) -> AppResult<()> {
        self.child.kill().await?;
        Ok(())
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
