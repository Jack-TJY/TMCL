//! tmcl-net：HTTP 客户端、下载引擎、下载源与镜像。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。

pub mod client;
pub mod download;

pub use client::{HttpClient, USER_AGENT};
pub use download::{
    DownloadItem, DownloadReport, Outcome, download_all, download_one, is_valid, sha1_file,
};
