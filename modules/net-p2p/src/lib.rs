//! tmcl-p2p：P2P 直连联机（PLAN.md M2-12）。
//!
//! 功能模块 crate，只依赖 `core/*`，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//! 方案见 `docs/research/p2p.md`：以 EasyTier 为底层，实现 Scaffolding 协议，
//! 与 HMCL、PCL-CE、陶瓦联机的房间互通。
//!
//! - [`room`]：房间码生成与解析、联机中心主机名。
//! - [`easytier`]：运行时下载并校验 EasyTier。
//! - [`nodes`]：默认公共节点与用户自定义节点。
//! - [`tunnel`]：`easytier-core` 子进程与端口转发。
//! - [`punch`]：读取连接与 NAT 状态、查找联机中心。
//! - [`signal`]：Scaffolding 联机信息获取协议（联机中心服务与房客客户端）。
//! - [`lan`]：Minecraft 局域网广播的监听与发送。
//! - [`relay`]：中继接口（暂无实现）。

pub mod easytier;
pub mod lan;
pub mod nodes;
pub mod punch;
pub mod relay;
pub mod room;
pub mod signal;
pub mod tunnel;

pub use easytier::{EASYTIER_VERSION, EasyTierAsset, EasyTierBinaries, ensure_installed};
pub use lan::{Announcer, LanWorld};
pub use nodes::{DEFAULT_PUBLIC_NODES, effective_nodes, validate_node};
pub use punch::{Peer, find_center};
pub use relay::Relay;
pub use room::RoomCode;
pub use signal::{PlayerKind, PlayerProfile, SignalClient, SignalServer};
pub use tunnel::{PortForward, Proto, Tunnel, TunnelConfig, TunnelState};
