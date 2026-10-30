//! tmcl-sakura：樱花穿透（PLAN.md M2-14，方案见 `docs/research/sakurafrp.md` 方案 A）。
//!
//! 功能模块 crate，只依赖 `core/*`，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`api`]：SakuraFrp 开放 API（账号、节点、隧道、客户端下载信息、服务条款）。
//! - [`frpc`]：运行时下载并校验官方 frpc，以隐藏子进程运行并解析联机地址。
//! - [`tunnel`]：访问密钥（系统密钥库）与一次联机会话（临时隧道的创建与删除）。

pub mod api;
pub mod frpc;
pub mod tunnel;

pub use api::{SakuraClient, SakuraNode, SakuraPolicy, SakuraTunnel, SakuraUser};
pub use frpc::ensure_frpc;
pub use tunnel::{SakuraRoom, Session, StartRequest};
