//! tmcl-net：HTTP 客户端、下载引擎、下载源与镜像。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。

mod chunk;
pub mod client;
pub mod download;
pub mod progress;
pub mod retry;
pub mod source;
#[cfg(feature = "test-util")]
pub mod testing;

pub use client::{Body, HttpClient, Timeouts, USER_AGENT};
pub use download::{
    DownloadItem, DownloadOptions, DownloadReport, Downloader, Outcome, download_all, download_one,
    is_valid, sha1_file,
};
pub use progress::ProgressTracker;
pub use retry::{RetryPolicy, is_retryable};
pub use source::{Candidate, DownloadSource, Official, PrefixMirror, Sources};
