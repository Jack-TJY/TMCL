//! 大文件分片下载。
//!
//! 先用 `Range: bytes=0-0` 探测服务器是否支持 `Range`；不支持时退回单连接下载。
//! 支持时按 [`DownloadOptions::chunk_size`](crate::DownloadOptions::chunk_size)
//! 切成若干分片，每个分片写到 `<文件名>.<序号>.part`，各占一个全局连接许可并发下载。
//! 分片中断时保留已写入的部分，下一次尝试从断点续传该分片。全部分片完成后
//! 按顺序合并为 `<文件名>.part` 并校验大小与 SHA1，校验失败时删除所有临时文件。

use std::path::{Path, PathBuf};

use reqwest::StatusCode;
use sha1::{Digest, Sha1};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;

use crate::download::{
    DownloadItem, Downloader, ItemProgress, acquire, content_range_start, io_error, part_len,
    to_hex, verify,
};

/// 一次分片下载尝试，返回合并后 `part` 的字节数。服务器不支持 `Range` 时退回单连接。
pub(crate) async fn attempt(
    engine: &Downloader,
    item: &DownloadItem,
    size: u64,
    part: &Path,
    progress: &ItemProgress,
    ctx: &TaskCtx,
) -> AppResult<u64> {
    let chunks = plan(size, engine.options().chunk_size);
    if !supports_range(engine, item, size, ctx).await? {
        tracing::debug!(url = %item.url, "服务器不支持 Range，改为单连接下载");
        remove_chunks(part, chunks.len()).await;
        return engine.attempt(item, part, progress, ctx).await;
    }

    let mut existing = 0;
    for (index, &(start, end)) in chunks.iter().enumerate() {
        existing += part_len(&chunk_path(part, index))
            .await?
            .min(end + 1 - start);
    }
    progress.set(existing);

    // 子令牌：一个分片失败时停止其余分片，不影响整个任务。
    let chunk_ctx = TaskCtx {
        cancel: ctx.cancel.child_token(),
        ..ctx.clone()
    };
    let mut set = JoinSet::new();
    for (index, (start, end)) in chunks.iter().copied().enumerate() {
        let engine = engine.clone();
        let url = item.url.clone();
        let path = chunk_path(part, index);
        let progress = progress.clone();
        let ctx = chunk_ctx.clone();
        set.spawn(
            async move { fetch_chunk(&engine, &url, &path, start, end, &progress, &ctx).await },
        );
    }
    let mut first_error: Option<AppError> = None;
    while let Some(joined) = set.join_next().await {
        let result = joined
            .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))
            .and_then(|r| r);
        if let Err(e) = result {
            chunk_ctx.cancel.cancel();
            if first_error.as_ref().is_none_or(AppError::is_cancelled) {
                first_error = Some(e);
            }
        }
    }
    ctx.check_cancelled()?;
    if let Some(e) = first_error {
        return Err(e);
    }

    let (written, sha1) = merge(part, chunks.len()).await?;
    remove_chunks(part, chunks.len()).await;
    if let Err(e) = verify(item, written, &sha1) {
        let _ = fs::remove_file(part).await;
        progress.set(0);
        return Err(e);
    }
    Ok(written)
}

/// 把 `[0, size)` 切成每段至多 `chunk_size` 字节的闭区间。
fn plan(size: u64, chunk_size: u64) -> Vec<(u64, u64)> {
    let chunk_size = chunk_size.max(1);
    (0..size.div_ceil(chunk_size))
        .map(|i| {
            let start = i * chunk_size;
            (start, (start + chunk_size).min(size) - 1)
        })
        .collect()
}

/// `foo.jar.part` → `foo.jar.<index>.part`。
fn chunk_path(part: &Path, index: usize) -> PathBuf {
    let stem = part.file_stem().unwrap_or_default().to_os_string();
    let mut name = stem;
    name.push(format!(".{index}.part"));
    part.with_file_name(name)
}

/// 用 `bytes=0-0` 探测服务器是否支持 `Range`，并确认总长度与期望一致。
async fn supports_range(
    engine: &Downloader,
    item: &DownloadItem,
    size: u64,
    ctx: &TaskCtx,
) -> AppResult<bool> {
    let _permit = acquire(engine.connections(), ctx).await?;
    let response = engine
        .client()
        .get_range(&item.url, 0, Some(0), ctx)
        .await?;
    if response.status() != StatusCode::PARTIAL_CONTENT {
        return Ok(false);
    }
    let total = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('/').next())
        .and_then(|v| v.parse::<u64>().ok());
    Ok(total == Some(size))
}

/// 下载一个分片 `[start, end]` 到 `path`，从已有长度续传。
async fn fetch_chunk(
    engine: &Downloader,
    url: &str,
    path: &Path,
    start: u64,
    end: u64,
    progress: &ItemProgress,
    ctx: &TaskCtx,
) -> AppResult<()> {
    let _permit = acquire(engine.connections(), ctx).await?;
    let expected = end + 1 - start;
    let mut have = part_len(path).await?;
    if have > expected {
        // 不应发生（例如分片大小改过）；撤回计入的字节后重下该分片。
        progress.sub(expected);
        have = 0;
    }
    if have == expected {
        return Ok(());
    }
    let from = start + have;
    let response = engine.client().get_range(url, from, Some(end), ctx).await?;
    if response.status() != StatusCode::PARTIAL_CONTENT
        || content_range_start(&response) != Some(from)
    {
        return Err(AppError::new(ErrorCode::NetRequest)
            .with_param("url", url)
            .with_detail(format!("分片请求 {from}-{end} 未得到对应的 206 响应")));
    }
    let mut file = if have > 0 {
        fs::OpenOptions::new().append(true).open(path).await
    } else {
        fs::File::create(path).await
    }
    .map_err(|e| io_error(path, &e))?;
    let mut written = have;
    let mut body = engine.client().body(response, url);
    while let Some(chunk) = body.next(ctx).await? {
        let n = chunk.len() as u64;
        if written + n > expected {
            return Err(AppError::new(ErrorCode::NetSizeMismatch)
                .with_param("url", url)
                .with_param("expected", expected)
                .with_param("actual", written + n));
        }
        file.write_all(&chunk)
            .await
            .map_err(|e| io_error(path, &e))?;
        written += n;
        progress.add(n);
    }
    file.flush().await.map_err(|e| io_error(path, &e))?;
    if written != expected {
        return Err(AppError::new(ErrorCode::NetSizeMismatch)
            .with_param("url", url)
            .with_param("expected", expected)
            .with_param("actual", written));
    }
    Ok(())
}

/// 按顺序把各分片合并到 `part`，返回总字节数与 SHA1。
async fn merge(part: &Path, count: usize) -> AppResult<(u64, String)> {
    let mut out = fs::File::create(part)
        .await
        .map_err(|e| io_error(part, &e))?;
    let mut hasher = Sha1::new();
    let mut written = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    for index in 0..count {
        let path = chunk_path(part, index);
        let mut file = fs::File::open(&path)
            .await
            .map_err(|e| io_error(&path, &e))?;
        loop {
            let n = file.read(&mut buf).await.map_err(|e| io_error(&path, &e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])
                .await
                .map_err(|e| io_error(part, &e))?;
            written += n as u64;
        }
    }
    out.flush().await.map_err(|e| io_error(part, &e))?;
    Ok((written, to_hex(&hasher.finalize())))
}

/// 删除所有分片临时文件；删除失败不影响结果。
async fn remove_chunks(part: &Path, count: usize) {
    for index in 0..count {
        let _ = fs::remove_file(chunk_path(part, index)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_covers_whole_file() {
        assert_eq!(plan(10, 4), vec![(0, 3), (4, 7), (8, 9)]);
        assert_eq!(plan(8, 4), vec![(0, 3), (4, 7)]);
        assert_eq!(plan(3, 4), vec![(0, 2)]);
        assert!(plan(0, 4).is_empty());
    }

    #[test]
    fn chunk_path_keeps_part_suffix() {
        assert_eq!(
            chunk_path(Path::new("a/client.jar.part"), 3),
            PathBuf::from("a/client.jar.3.part")
        );
    }
}
