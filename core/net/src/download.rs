//! 文件下载：写 `.part` 临时文件、边下边算 SHA1、校验通过后 rename。
//!
//! 已存在且大小、SHA1 均匹配的文件直接跳过。这是 M0-09 的最小实现，
//! 分片、续传、重试与镜像回退由 M1-01 / M1-02 在此基础上补充。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha1::{Digest, Sha1};
use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::client::{HttpClient, map_reqwest_error};

/// 一个待下载的文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadItem {
    /// 下载地址。
    pub url: String,
    /// 目标路径。
    pub path: PathBuf,
    /// 期望大小（字节），未知时为 `None`。
    pub size: Option<u64>,
    /// 期望 SHA1（小写十六进制），未知时为 `None`。
    pub sha1: Option<String>,
}

/// 一批下载的结果统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DownloadReport {
    /// 实际下载的文件数。
    pub downloaded: u64,
    /// 已存在且校验通过而跳过的文件数。
    pub skipped: u64,
    /// 实际下载的字节数。
    pub bytes: u64,
}

/// 单个文件的处理结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 已下载，附带字节数。
    Downloaded(u64),
    /// 已存在且校验通过。
    Skipped,
}

/// 检查 `path` 是否已存在且与期望的大小和 SHA1 一致。
///
/// 没有给出 SHA1 时只比较大小；大小和 SHA1 都未知时，文件存在即视为有效。
pub async fn is_valid(path: &Path, size: Option<u64>, sha1: Option<&str>) -> AppResult<bool> {
    let meta = match fs::metadata(path).await {
        Ok(meta) if meta.is_file() => meta,
        Ok(_) => return Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(io_error(path, &e)),
    };
    if size.is_some_and(|s| s != meta.len()) {
        return Ok(false);
    }
    let Some(expected) = sha1 else {
        return Ok(true);
    };
    let actual = sha1_file(path).await?;
    Ok(actual.eq_ignore_ascii_case(expected))
}

/// 计算文件的 SHA1（小写十六进制）。
pub async fn sha1_file(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path).await.map_err(|e| io_error(path, &e))?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await.map_err(|e| io_error(path, &e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(to_hex(&hasher.finalize()))
}

/// 下载单个文件；已存在且校验通过时跳过。不上报进度，供 [`download_all`] 复用。
pub async fn download_one(
    client: &HttpClient,
    item: &DownloadItem,
    ctx: &TaskCtx,
) -> AppResult<Outcome> {
    ctx.check_cancelled()?;
    if is_valid(&item.path, item.size, item.sha1.as_deref()).await? {
        return Ok(Outcome::Skipped);
    }
    if let Some(parent) = item.path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| io_error(parent, &e))?;
    }
    let part = part_path(&item.path);
    let result = fetch_to(client, item, &part, ctx).await;
    if result.is_err() {
        // 失败或取消时不留下半个文件；删除失败不影响原错误。
        let _ = fs::remove_file(&part).await;
    }
    let bytes = result?;
    fs::rename(&part, &item.path)
        .await
        .map_err(|e| io_error(&item.path, &e))?;
    Ok(Outcome::Downloaded(bytes))
}

/// 以最多 `concurrency` 个并发下载一批文件，在 `stage` 阶段上报条目与字节进度。
///
/// 遇到第一个错误即取消其余下载并返回该错误。
pub async fn download_all(
    client: &HttpClient,
    items: Vec<DownloadItem>,
    concurrency: usize,
    stage: &str,
    ctx: &TaskCtx,
) -> AppResult<DownloadReport> {
    let total = items.len() as u64;
    let bytes_total: u64 = items.iter().filter_map(|i| i.size).sum();
    let (mut done, mut bytes_done) = (0u64, 0u64);
    let report = |done: u64, bytes: u64| {
        ctx.report(
            ProgressUpdate::new(stage)
                .items(done, total)
                .bytes(bytes, bytes_total),
        );
    };
    report(0, 0);

    // 子令牌：本批出错时只取消本批，不影响整个任务的令牌。
    let batch_ctx = TaskCtx {
        cancel: ctx.cancel.child_token(),
        ..ctx.clone()
    };
    let semaphore = Arc::new(Semaphore::new(concurrency.max(1)));
    let mut set = JoinSet::new();
    for item in items {
        let client = client.clone();
        let ctx = batch_ctx.clone();
        let semaphore = semaphore.clone();
        set.spawn(async move {
            let _permit = semaphore
                .acquire_owned()
                .await
                .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?;
            let outcome = download_one(&client, &item, &ctx).await?;
            Ok::<_, AppError>((outcome, item.size.unwrap_or(0)))
        });
    }

    let mut result = DownloadReport::default();
    let mut first_error: Option<AppError> = None;
    while let Some(joined) = set.join_next().await {
        let outcome = joined
            .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))
            .and_then(|r| r);
        match outcome {
            Ok((Outcome::Downloaded(bytes), _)) => {
                result.downloaded += 1;
                result.bytes += bytes;
                done += 1;
                bytes_done += bytes;
                report(done, bytes_done);
            }
            Ok((Outcome::Skipped, size)) => {
                result.skipped += 1;
                done += 1;
                bytes_done += size;
                report(done, bytes_done);
            }
            Err(e) => {
                if first_error.is_none() {
                    batch_ctx.cancel.cancel();
                    first_error = Some(e);
                } else if first_error.as_ref().is_some_and(AppError::is_cancelled)
                    && !e.is_cancelled()
                {
                    // 优先报告真正的失败原因，而不是由它引起的取消。
                    first_error = Some(e);
                }
            }
        }
    }
    // 整个任务被取消时统一报告取消。
    ctx.check_cancelled()?;
    match first_error {
        Some(e) => Err(e),
        None => Ok(result),
    }
}

/// 把响应体写入 `part`，校验大小与 SHA1，返回字节数。
async fn fetch_to(
    client: &HttpClient,
    item: &DownloadItem,
    part: &Path,
    ctx: &TaskCtx,
) -> AppResult<u64> {
    let mut response = client.get(&item.url, ctx).await?;
    let mut file = fs::File::create(part)
        .await
        .map_err(|e| io_error(part, &e))?;
    let mut hasher = Sha1::new();
    let mut written: u64 = 0;
    loop {
        let chunk = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
            c = response.chunk() => c.map_err(|e| map_reqwest_error(&item.url, &e))?,
        };
        let Some(chunk) = chunk else { break };
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| io_error(part, &e))?;
        written += chunk.len() as u64;
    }
    file.flush().await.map_err(|e| io_error(part, &e))?;
    drop(file);

    if let Some(expected) = item.size
        && expected != written
    {
        return Err(AppError::new(ErrorCode::NetSizeMismatch)
            .with_param("url", &item.url)
            .with_param("expected", expected)
            .with_param("actual", written));
    }
    if let Some(expected) = &item.sha1 {
        let actual = to_hex(&hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(AppError::new(ErrorCode::NetHashMismatch)
                .with_param("url", &item.url)
                .with_param("expected", expected)
                .with_param("actual", actual));
        }
    }
    Ok(written)
}

/// `foo.jar` → `foo.jar.part`。
fn part_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(40), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn io_error(path: &Path, err: &std::io::Error) -> AppError {
    AppError::new(ErrorCode::CommonIo)
        .with_param("path", path.display())
        .with_detail(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_path_appends_suffix() {
        assert_eq!(
            part_path(Path::new("a/b/client.jar")),
            PathBuf::from("a/b/client.jar.part")
        );
    }

    #[test]
    fn hex_is_lowercase() {
        assert_eq!(to_hex(&[0x00, 0xab, 0xff]), "00abff");
    }
}
