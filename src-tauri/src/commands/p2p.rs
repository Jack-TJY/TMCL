//! `p2p_*` 命令：P2P 直连联机（PLAN.md M2-12）。
//!
//! 目前只有 `p2p_info`，供设置页显示默认节点、关于页显示 EasyTier 版本；
//! 建房、加入房间等命令在 M2-12b 中加入。

use serde::Serialize;
use ts_rs::TS;

/// `p2p_info` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct P2pInfo {
    /// 运行时下载的 EasyTier 版本，如 `v2.6.4`。
    pub easytier_version: String,
    /// 内置的默认公共节点，按使用顺序排列。
    pub default_nodes: Vec<String>,
}

/// 读取 P2P 联机的静态信息。
#[tauri::command]
pub fn p2p_info() -> P2pInfo {
    P2pInfo {
        easytier_version: tmcl_p2p::EASYTIER_VERSION.to_owned(),
        default_nodes: tmcl_p2p::DEFAULT_PUBLIC_NODES
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
    }
}
