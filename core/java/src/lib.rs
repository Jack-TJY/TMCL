//! tmcl-java：Java 探测、下载、版本匹配。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`detect`]：枚举本机已安装的 Java；
//! - [`probe`]：识别单个 Java 的版本、架构与发行商；
//! - [`select`]：按游戏版本要求选出 Java，或检查用户手动指定的 Java。

pub mod detect;
pub mod probe;
pub mod select;

pub use detect::{Sources, detect};
pub use probe::{Arch, JavaInstall, java_executable, parse_major, probe};
pub use select::{LEGACY_MAJOR, check_manual, required_major, select};
