//! 中继接口（PLAN.md M2-12）。
//!
//! 中继服务器的部署与费用仍挂起（PLAN.md 第 9 节），因此这里只定义接口、没有实现。
//! 直连失败时由调用方返回 `P2P_DIRECT_FAILED` 并建议改用樱花穿透。

/// 一个可选的中继：打洞失败时为 EasyTier 额外提供一批允许中转的节点。
pub trait Relay: Send + Sync {
    /// 中继名称，用于日志。
    fn name(&self) -> &str;

    /// 追加到 EasyTier `-p` 参数中的节点地址。
    fn nodes(&self) -> Vec<String>;
}
