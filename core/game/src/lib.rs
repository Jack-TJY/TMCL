//! tmcl-game：版本清单、版本 JSON、依赖库/资源/natives 补全。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。

pub mod install;
pub mod manifest;
pub mod version;

pub use install::{GameFiles, InstallOptions, InstallReport, install_version, parse_version};
pub use manifest::{MANIFEST_URL, ManifestEntry, VersionManifest, VersionType};
pub use version::{RuleEnv, VersionJson, rules_allow};
