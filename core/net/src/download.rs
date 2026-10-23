//! 下载引擎：全局并发上限、指数退避重试、断点续传与 SHA1 校验。
//!
//! 每个文件先写到 `<文件名>.part`，边下边算 SHA1，校验通过后 rename。
//! 网络中断或取消时保留 `.part`，下一次尝试（或下一次运行）从已有长度用
//! `Range` 续传；服务器不支持 `Range` 时从头重下。大小或 SHA1 不符时删除
//! `.part` 并从头重试。已存在且大小、SHA1 均匹配的文件直接跳过。
//! 大于 [`DownloadOptions::chunk_threshold`] 且服务器支持 `Range` 的文件
//! 分片并发下载（`chunk.rs`）。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use reqwest::StatusCode;
use sha1::{Digest, Sha1};
use tmcl_common::config::DownloadConfig;
use tmcl_common::task::PROGRESS_INTERVAL;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;
use tokio::time::MissedTickBehavior;

use crate::chunk;
use crate::client::{HttpClient, map_reqwest_error};
use crate::progress::ProgressTracker;
use crate::retry::RetryPolicy;

/// 默认同时进行的连接数。
pub const DEFAULT_CONCURRENCY: usize = 16;
/// 默认分片阈值：大于 8 MiB 的文件分片下载。
pub const DEFAULT_CHUNK_THRESHOLD: u64 = 8 * 1024 * 1024;
/// 默认分片大小：4 MiB。
pub const DEFAULT_CHUNK_SIZE: u64 = 4 * 1024 * 1024;

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
    /// 实际下载的文件的总字节数。
    pub bytes: u64,
}

/// 单个文件的处理结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 已下载，附带文件字节数。
    Downloaded(u64),
    /// 已存在且校验通过。
    Skipped,
}

/// 下载引擎的设置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOptions {
    /// 同时进行的连接数上限（全局，对同一个 [`Downloader`] 的所有调用生效）。
    pub concurrency: usize,
    /// 可恢复错误的重试策略。
    pub retry: RetryPolicy,
    /// 大于该字节数且服务器支持 `Range` 的文件分片下载。
    pub chunk_threshold: u64,
    /// 每个分片的字节数。
    pub chunk_size: u64,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            concurrency: DEFAULT_CONCURRENCY,
            retry: RetryPolicy::default(),
            chunk_threshold: DEFAULT_CHUNK_THRESHOLD,
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }
}

impl DownloadOptions {
    /// 由 `config.json` 的 `download` 段得出设置。
    pub fn from_config(config: &DownloadConfig) -> Self {
        Self {
            concurrency: config.max_concurrency as usize,
            ..Self::default()
        }
    }
}

/// 下载引擎。克隆开销很小，克隆体共享同一个连接数上限。
#[derive(Debug, Clone)]
pub struct Downloader {
    client: HttpClient,
    options: Arc<DownloadOptions>,
    connections: Arc<Semaphore>,
}

impl Downloader {
    /// 用给定的客户端与设置创建下载引擎；并发数至少为 1。
    pub fn new(client: HttpClient, options: DownloadOptions) -> Self {
        let connections = Arc::new(Semaphore::new(options.concurrency.max(1)));
        Self {
            client,
            options: Arc::new(options),
            connections,
        }
    }

    /// 内部使用的 HTTP 客户端。
    pub fn client(&self) -> &HttpClient {
        &self.client
    }

    /// 当前设置。
    pub fn options(&self) -> &DownloadOptions {
        &self.options
    }

    /// 全局连接数上限。
    pub(crate) fn connections(&self) -> Arc<Semaphore> {
        self.connections.clone()
    }

    /// 下载单个文件；已存在且校验通过时跳过。不上报进度。
    pub async fn download_one(&self, item: &DownloadItem, ctx: &TaskCtx) -> AppResult<Outcome> {
        self.fetch(item, None, ctx).await
    }

    /// 并发下载一批文件，在 `stage` 阶段上报条目、字节与速度。
    ///
    /// 遇到第一个不可恢复的错误即取消其余下载并返回该错误。
    pub async fn download_all(
        &self,
        items: Vec<DownloadItem>,
        stage: &str,
        ctx: &TaskCtx,
    ) -> AppResult<DownloadReport> {
        let bytes_total = items.iter().filter_map(|i| i.size).sum();
        let tracker = Arc::new(ProgressTracker::new(stage, items.len() as u64, bytes_total));

        // 子令牌：本批出错时只取消本批，不影响整个任务的令牌。
        let batch_ctx = TaskCtx {
            cancel: ctx.cancel.child_token(),
            ..ctx.clone()
        };
        // 同时处理的文件数也受限，避免成千上万个文件同时计算 SHA1。
        let files = Arc::new(Semaphore::new(self.options.concurrency.max(1)));
        let mut set = JoinSet::new();
        for item in items {
            let this = self.clone();
            let ctx = batch_ctx.clone();
            let files = files.clone();
            let tracker = tracker.clone();
            set.spawn(async move {
                let _permit = acquire(files, &ctx).await?;
                this.fetch(&item, Some(tracker), &ctx).await
            });
        }

        let mut ticker = tokio::time::interval(PROGRESS_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut result = DownloadReport::default();
        let mut first_error: Option<AppError> = None;
        loop {
            let joined = tokio::select! {
                joined = set.join_next() => joined,
                _ = ticker.tick() => {
                    tracker.report(ctx);
                    continue;
                }
            };
            let Some(joined) = joined else { break };
            let outcome = joined
                .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))
                .and_then(|r| r);
            match outcome {
                Ok(Outcome::Downloaded(bytes)) => {
                    result.downloaded += 1;
                    result.bytes += bytes;
                }
                Ok(Outcome::Skipped) => result.skipped += 1,
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
        tracker.report(ctx);
        // 整个任务被取消时统一报告取消。
        ctx.check_cancelled()?;
        match first_error {
            Some(e) => Err(e),
            None => Ok(result),
        }
    }

    /// 下载一个文件（含跳过判断与重试），完成后计入 `tracker`。
    async fn fetch(
        &self,
        item: &DownloadItem,
        tracker: Option<Arc<ProgressTracker>>,
        ctx: &TaskCtx,
    ) -> AppResult<Outcome> {
        ctx.check_cancelled()?;
        if is_valid(&item.path, item.size, item.sha1.as_deref()).await? {
            if let Some(tracker) = &tracker {
                tracker.add_bytes(item.size.unwrap_or(0));
                tracker.finish_item();
            }
            return Ok(Outcome::Skipped);
        }
        if let Some(parent) = item.path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| io_error(parent, &e))?;
        }
        let part = part_path(&item.path);
        let progress = ItemProgress {
            tracker: tracker.clone(),
            counted: Arc::new(AtomicU64::new(0)),
        };
        let bytes = self
            .options
            .retry
            .run(ctx, |_| async {
                match item.size {
                    Some(size) if size > self.options.chunk_threshold => {
                        chunk::attempt(self, item, size, &part, &progress, ctx).await
                    }
                    _ => self.attempt(item, &part, &progress, ctx).await,
                }
            })
            .await?;
        fs::rename(&part, &item.path)
            .await
            .map_err(|e| io_error(&item.path, &e))?;
        if let Some(tracker) = &tracker {
            tracker.finish_item();
        }
        Ok(Outcome::Downloaded(bytes))
    }

    /// 一次单连接下载尝试：从 `.part` 已有长度续传，写完后校验，返回文件字节数。
    pub(crate) async fn attempt(
        &self,
        item: &DownloadItem,
        part: &Path,
        progress: &ItemProgress,
        ctx: &TaskCtx,
    ) -> AppResult<u64> {
        let _permit = acquire(self.connections.clone(), ctx).await?;
        let mut offset = part_len(part).await?;
        if item.size.is_some_and(|size| offset > size) {
            offset = 0;
        }
        let response = self.open(item, &mut offset, ctx).await?;
        progress.set(offset);

        let mut hasher = Sha1::new();
        let mut file = if offset > 0 {
            hash_prefix(part, offset, &mut hasher).await?;
            fs::OpenOptions::new()
                .append(true)
                .open(part)
                .await
                .map_err(|e| io_error(part, &e))?
        } else {
            fs::File::create(part)
                .await
                .map_err(|e| io_error(part, &e))?
        };
        let mut written = offset;
        if let Some(mut response) = response {
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
                progress.add(chunk.len() as u64);
            }
        }
        file.flush().await.map_err(|e| io_error(part, &e))?;
        drop(file);

        if let Err(e) = verify(item, written, &to_hex(&hasher.finalize())) {
            // 内容已损坏，续传没有意义：删掉从头再来。
            let _ = fs::remove_file(part).await;
            progress.set(0);
            return Err(e);
        }
        Ok(written)
    }

    /// 打开响应流。`offset > 0` 时尝试续传；服务器不支持续传时把 `offset` 置 0。
    /// `.part` 已经是完整大小时返回 `None`，直接进入校验。
    async fn open(
        &self,
        item: &DownloadItem,
        offset: &mut u64,
        ctx: &TaskCtx,
    ) -> AppResult<Option<reqwest::Response>> {
        if *offset == 0 {
            return self.client.get(&item.url, ctx).await.map(Some);
        }
        if item.size == Some(*offset) {
            return Ok(None);
        }
        match self.client.get_range(&item.url, *offset, None, ctx).await {
            Ok(r) if r.status() == StatusCode::PARTIAL_CONTENT => {
                if content_range_start(&r) == Some(*offset) {
                    return Ok(Some(r));
                }
                tracing::debug!(url = %item.url, "Content-Range 与请求不符，从头下载");
            }
            Ok(r) => {
                // 服务器忽略了 Range，返回的是完整内容。
                *offset = 0;
                return Ok(Some(r));
            }
            Err(e) if e.params.get("status").is_some_and(|s| s == "416") => {}
            Err(e) => return Err(e),
        }
        *offset = 0;
        self.client.get(&item.url, ctx).await.map(Some)
    }
}

/// 单个文件计入批次进度的字节数，重试从头开始时能准确撤回。
/// 克隆体共享计数，供同一文件的各分片使用。
#[derive(Clone)]
pub(crate) struct ItemProgress {
    tracker: Option<Arc<ProgressTracker>>,
    counted: Arc<AtomicU64>,
}

impl ItemProgress {
    /// 累加本文件新传输的字节。
    pub(crate) fn add(&self, n: u64) {
        self.counted.fetch_add(n, Ordering::Relaxed);
        if let Some(tracker) = &self.tracker {
            tracker.add_bytes(n);
        }
    }

    /// 撤回本文件已计入的 `n` 个字节。
    pub(crate) fn sub(&self, n: u64) {
        self.counted.fetch_sub(n, Ordering::Relaxed);
        if let Some(tracker) = &self.tracker {
            tracker.sub_bytes(n);
        }
    }

    /// 把本文件已计入的字节数校正为 `now`。
    pub(crate) fn set(&self, now: u64) {
        let before = self.counted.swap(now, Ordering::Relaxed);
        if let Some(tracker) = &self.tracker {
            if now >= before {
                tracker.add_bytes(now - before);
            } else {
                tracker.sub_bytes(before - now);
            }
        }
    }
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
    let mut hasher = Sha1::new();
    hash_prefix(path, u64::MAX, &mut hasher).await?;
    Ok(to_hex(&hasher.finalize()))
}

/// 用默认设置（并发数除外）下载单个文件，见 [`Downloader::download_one`]。
pub async fn download_one(
    client: &HttpClient,
    item: &DownloadItem,
    ctx: &TaskCtx,
) -> AppResult<Outcome> {
    Downloader::new(client.clone(), DownloadOptions::default())
        .download_one(item, ctx)
        .await
}

/// 以最多 `concurrency` 个连接下载一批文件，见 [`Downloader::download_all`]。
pub async fn download_all(
    client: &HttpClient,
    items: Vec<DownloadItem>,
    concurrency: usize,
    stage: &str,
    ctx: &TaskCtx,
) -> AppResult<DownloadReport> {
    let options = DownloadOptions {
        concurrency,
        ..DownloadOptions::default()
    };
    Downloader::new(client.clone(), options)
        .download_all(items, stage, ctx)
        .await
}

/// 等待一个许可，可被取消。
pub(crate) async fn acquire(
    semaphore: Arc<Semaphore>,
    ctx: &TaskCtx,
) -> AppResult<OwnedSemaphorePermit> {
    tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => Err(AppError::cancelled()),
        p = semaphore.acquire_owned() => p.map_err(|e| {
            AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string())
        }),
    }
}

/// 校验写入的大小与 SHA1。
pub(crate) fn verify(item: &DownloadItem, written: u64, actual_sha1: &str) -> AppResult<()> {
    if let Some(expected) = item.size
        && expected != written
    {
        return Err(AppError::new(ErrorCode::NetSizeMismatch)
            .with_param("url", &item.url)
            .with_param("expected", expected)
            .with_param("actual", written));
    }
    if let Some(expected) = &item.sha1
        && !actual_sha1.eq_ignore_ascii_case(expected)
    {
        return Err(AppError::new(ErrorCode::NetHashMismatch)
            .with_param("url", &item.url)
            .with_param("expected", expected)
            .with_param("actual", actual_sha1));
    }
    Ok(())
}

/// `.part` 当前长度，不存在时为 0。
pub(crate) async fn part_len(part: &Path) -> AppResult<u64> {
    match fs::metadata(part).await {
        Ok(meta) => Ok(meta.len()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(io_error(part, &e)),
    }
}

/// 把文件的前 `limit` 个字节喂给 `hasher`。
pub(crate) async fn hash_prefix(path: &Path, limit: u64, hasher: &mut Sha1) -> AppResult<()> {
    let file = fs::File::open(path).await.map_err(|e| io_error(path, &e))?;
    let mut reader = file.take(limit);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .await
            .map_err(|e| io_error(path, &e))?;
        if n == 0 {
            return Ok(());
        }
        hasher.update(&buf[..n]);
    }
}

/// 解析 `Content-Range: bytes <start>-<end>/<total>` 的起点。
pub(crate) fn content_range_start(response: &reqwest::Response) -> Option<u64> {
    let value = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)?
        .to_str()
        .ok()?;
    value
        .strip_prefix("bytes ")?
        .split('-')
        .next()?
        .parse()
        .ok()
}

/// `foo.jar` → `foo.jar.part`。
fn part_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(40), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

pub(crate) fn io_error(path: &Path, err: &std::io::Error) -> AppError {
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

    #[test]
    fn options_follow_config() {
        let options = DownloadOptions::from_config(&DownloadConfig { max_concurrency: 4 });
        assert_eq!(options.concurrency, 4);
        assert_eq!(options.retry, RetryPolicy::default());
    }
}
