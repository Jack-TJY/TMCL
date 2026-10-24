//! tmcl-auth：微软 / 离线 / authlib-injector 账号。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。

pub mod accounts;
pub mod authlib;
pub mod config;
mod http;
pub mod microsoft;
pub mod minecraft;
pub mod offline;
pub mod secrets;
pub mod store;
pub mod xbox;
pub mod yggdrasil;

pub use accounts::{Account, AccountKind, AccountList, AuthlibServer, NewAccount};
pub use config::{ms_client_id, ms_login_enabled};
pub use microsoft::{DeviceCode, Endpoints, MsClient};
pub use offline::{OfflineAccount, offline_uuid};
pub use secrets::{KeyringStore, MemorySecretStore, SecretStore};
pub use store::MicrosoftSession;
