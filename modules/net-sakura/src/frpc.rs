//! 官方 frpc：运行时下载并校验（不随安装包分发），以隐藏子进程运行并解析日志得到联机地址。
//!
//! 下载信息来自 `GET /system/clients` 的 `frpc` 分类，每个架构是一个未压缩的可执行文件，
//! 按 `size` 与 `hash`（MD5）校验，存到 `<data_root>/cache/sakura-frpc/<版本>/`。
//! 启动参数 `-f <访问密钥>:<隧道 ID>`；日志中 `>>` 与 `<<` 之间是联机地址。

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use md5::{Digest, Md5};
use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};
use tmcl_net::HttpClient;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::api::SakuraClient;

/// `GET /system/clients` 中 frpc 的分类名。
pub const CATEGORY: &str = "frpc";
/// 进度阶段名。
pub const STAGE: &str = "sakura_frpc";
/// 等待 frpc 打印联机地址的时限。
pub const START_TIMEOUT: Duration = Duration::from_secs(60);
/// 拿到域名地址后再等 IP 地址那一行的时间。
const IP_GRACE: Duration = Duration::from_secs(2);
/// 失败时错误详情里保留的最后几行日志。
const TAIL_LINES: usize = 8;

/// 可执行文件名。
pub fn exe_name() -> &'static str {
    if cfg!(windows) { "frpc.exe" } else { "frpc" }
}

/// 当前平台在下载信息中的架构键，例如 `windows_amd64`；不支持的平台返回 `None`。
pub fn platform_key() -> Option<String> {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "darwin",
        "freebsd" => "freebsd",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "x86" => "386",
        "aarch64" => "arm64",
        "arm" => "armv7",
        "riscv64" => "riscv64",
        "loongarch64" => "loong64",
        _ => return None,
    };
    Some(format!("{os}_{arch}"))
}

fn unavailable() -> AppError {
    AppError::new(ErrorCode::SakuraFrpcUnavailable).with_param(
        "platform",
        format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
    )
}

/// 确保 `dir` 下有最新版本的 frpc 并校验通过，返回其路径；旧版本会被删除。
///
/// 下载信息取不到但本地已有任一版本时使用本地版本。
pub async fn ensure_frpc(
    api: &SakuraClient,
    http: &HttpClient,
    dir: &Path,
    ctx: &TaskCtx,
) -> AppResult<PathBuf> {
    let key = platform_key().ok_or_else(unavailable)?;
    let clients = match api.clients(ctx).await {
        Ok(clients) => clients,
        Err(e) if e.is_cancelled() => return Err(e),
        Err(e) => {
            if let Some(existing) = local_versions(dir).into_iter().next() {
                tracing::warn!("frpc 下载信息不可用，使用本地版本：{e}");
                return Ok(existing);
            }
            return Err(e);
        }
    };
    let category = clients.get(CATEGORY).ok_or_else(unavailable)?;
    let file = category.archs.get(&key).ok_or_else(unavailable)?;
    let (Some(hash), Some(size)) = (file.hash.as_deref(), file.size) else {
        return Err(unavailable());
    };
    if !file.url.starts_with("https://") && !file.url.starts_with("http://127.0.0.1") {
        return Err(unavailable().with_detail("download url is not https"));
    }
    // 版本号只用作目录名，去掉可能的路径字符。
    let version: String = category
        .ver
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    if version.is_empty() || version.starts_with('.') {
        return Err(unavailable().with_detail("invalid version"));
    }
    let expected = hash.to_ascii_lowercase();
    let path = dir.join(&version).join(exe_name());
    if !is_valid(&path, size, &expected).await? {
        download(http, &file.url, &path, size, &expected, ctx).await?;
    }
    remove_other_versions(dir, &version).await;
    Ok(path)
}

/// 本地已有的 frpc，按版本目录名倒序。
fn local_versions(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path().join(exe_name()))
        .filter(|p| p.is_file())
        .collect();
    found.sort();
    found.reverse();
    found
}

async fn remove_other_versions(dir: &Path, keep: &str) {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.file_name() != keep && entry.path().is_dir() {
            // 旧版本可能正在运行（Windows 上删不掉），失败就留到下次。
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
}

/// 文件存在且大小、MD5 都一致。
async fn is_valid(path: &Path, size: u64, md5: &str) -> AppResult<bool> {
    match tokio::fs::metadata(path).await {
        Ok(meta) if meta.len() == size => {}
        _ => return Ok(false),
    }
    let bytes = tokio::fs::read(path).await?;
    Ok(hex(&Md5::digest(&bytes)) == md5)
}

async fn download(
    http: &HttpClient,
    url: &str,
    path: &Path,
    size: u64,
    md5: &str,
    ctx: &TaskCtx,
) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new(ErrorCode::CommonInternal))?;
    tokio::fs::create_dir_all(parent).await?;
    let part = path.with_extension("part");
    let result = async {
        let response = http.get(url, ctx).await?;
        let mut body = http.body(response, url);
        let mut file = tokio::fs::File::create(&part).await?;
        let mut hasher = Md5::new();
        let mut done = 0u64;
        while let Some(chunk) = body.next(ctx).await? {
            done += chunk.len() as u64;
            if done > size {
                break;
            }
            hasher.update(&*chunk);
            file.write_all(&chunk).await?;
            ctx.report(ProgressUpdate::new(STAGE).bytes(done, size));
        }
        file.flush().await?;
        drop(file);
        if done != size {
            return Err(AppError::new(ErrorCode::NetSizeMismatch)
                .with_param("url", url)
                .with_param("expected", size)
                .with_param("actual", done));
        }
        let actual = hex(&hasher.finalize());
        if actual != md5 {
            return Err(AppError::new(ErrorCode::NetHashMismatch)
                .with_param("url", url)
                .with_param("expected", md5)
                .with_param("actual", actual));
        }
        set_executable(&part)?;
        tokio::fs::rename(&part, path).await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
    }
    result
}

#[cfg(unix)]
fn set_executable(path: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> AppResult<()> {
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// frpc 的一行日志。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogLine {
    /// `使用 >>frp-xxx.com:10086<< 连接你的隧道`。
    Address(String),
    /// `或使用 IP 地址连接: >>1.2.3.4:10086<<`。
    IpAddress(String),
    /// 错误级别（`[E]` / `[F]`）的日志。
    Error,
    Other,
}

/// 去掉 ANSI 颜色后解析一行日志。格式没有正式文档，解析尽量宽松。
pub fn parse_line(line: &str) -> LogLine {
    let line = strip_ansi(line);
    if let Some(start) = line.find(">>") {
        let rest = &line[start + 2..];
        if let Some(end) = rest.find("<<") {
            let address = rest[..end].trim();
            if !address.is_empty() && address.contains(':') {
                return if line[..start].contains("IP") {
                    LogLine::IpAddress(address.to_owned())
                } else {
                    LogLine::Address(address.to_owned())
                };
            }
        }
    }
    if line.contains("[E]") || line.contains("[F]") {
        return LogLine::Error;
    }
    LogLine::Other
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI 序列：ESC [ 参数 … 结束字母。
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// frpc 打印出的联机地址。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Addresses {
    /// 域名形式，推荐分享给朋友。
    pub address: String,
    /// IP 形式；部分节点没有。
    pub ip_address: Option<String>,
}

/// 正在运行的 frpc。丢弃时结束进程。
#[derive(Debug)]
pub struct Frpc {
    child: Child,
}

impl Frpc {
    /// 启动 frpc 并等待联机地址。进程提前退出或超时返回 `SAKURA_FRPC_FAILED`，
    /// 详情为脱敏后的最后几行日志。
    pub async fn start(
        exe: &Path,
        key: &str,
        tunnel_id: u64,
        timeout: Duration,
        ctx: &TaskCtx,
    ) -> AppResult<(Self, Addresses)> {
        let mut command = Command::new(exe);
        command
            .arg("-f")
            .arg(format!("{key}:{tunnel_id}"))
            .args(["-n", "--disable_log_color"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW：frpc 是控制台程序，不弹出窗口。
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().map_err(|e| {
            AppError::new(ErrorCode::SakuraFrpcFailed).with_detail(format!("spawn: {e}"))
        })?;
        let (tx, mut rx) = mpsc::unbounded_channel();
        if let Some(out) = child.stdout.take() {
            forward(out, key.to_owned(), tx.clone());
        }
        if let Some(err) = child.stderr.take() {
            forward(err, key.to_owned(), tx);
        }

        let mut tail: Vec<String> = Vec::new();
        let mut found = Addresses::default();
        let failed = |tail: &[String], why: &str| {
            AppError::new(ErrorCode::SakuraFrpcFailed)
                .with_detail(format!("{why}\n{}", tail.join("\n")))
        };
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        loop {
            let line = tokio::select! {
                biased;
                _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
                _ = &mut deadline => return Err(failed(&tail, "timed out")),
                line = rx.recv() => line,
            };
            let Some(line) = line else {
                // 两路输出都关闭了：进程已退出。
                let status = child.wait().await.ok();
                return Err(failed(&tail, &format!("exited: {status:?}")));
            };
            remember(&mut tail, &line);
            match parse_line(&line) {
                LogLine::Address(a) => {
                    found.address = a;
                    break;
                }
                // 个别节点可能只有 IP 一行，先记下，没有域名时用它。
                LogLine::IpAddress(ip) => found.ip_address = Some(ip),
                LogLine::Error | LogLine::Other => {}
            }
        }
        // IP 那一行通常紧跟在域名后面。
        let grace = tokio::time::sleep(IP_GRACE);
        tokio::pin!(grace);
        while found.ip_address.is_none() {
            let line = tokio::select! {
                _ = &mut grace => break,
                line = rx.recv() => line,
            };
            let Some(line) = line else { break };
            if let LogLine::IpAddress(ip) = parse_line(&line) {
                found.ip_address = Some(ip);
            }
        }
        // 之后的日志只写入启动器日志。
        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                tracing::debug!(target: "frpc", "{line}");
            }
        });
        Ok((Self { child }, found))
    }

    /// 进程是否仍在运行。
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// 结束进程。
    pub async fn stop(mut self) {
        let _ = self.child.kill().await;
    }
}

fn remember(tail: &mut Vec<String>, line: &str) {
    if tail.len() == TAIL_LINES {
        tail.remove(0);
    }
    tail.push(line.to_owned());
}

/// 逐行读取输出，替换掉访问密钥后发给 `tx`。
fn forward(
    stream: impl AsyncRead + Unpin + Send + 'static,
    key: String,
    tx: mpsc::UnboundedSender<String>,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = if key.is_empty() {
                line
            } else {
                line.replace(&key, "***")
            };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_address_lines() {
        assert_eq!(
            parse_line("2026/10/30 12:00:00 [I] [sakura] 使用 >>frp-abc.com:10086<< 连接你的隧道"),
            LogLine::Address("frp-abc.com:10086".into())
        );
        assert_eq!(
            parse_line("\u{1b}[1;34m2026/10/30 [I] 或使用 IP 地址连接: >>1.2.3.4:10086<<\u{1b}[0m"),
            LogLine::IpAddress("1.2.3.4:10086".into())
        );
        assert_eq!(
            parse_line("2026/10/30 [E] [service.go:1] login failed"),
            LogLine::Error
        );
        assert_eq!(
            parse_line("2026/10/30 [I] start proxy success"),
            LogLine::Other
        );
        assert_eq!(parse_line(">><<"), LogLine::Other);
    }

    #[test]
    fn platform_key_is_known_on_ci_targets() {
        let key = platform_key().unwrap();
        assert!(key.contains('_'), "{key}");
    }
}
