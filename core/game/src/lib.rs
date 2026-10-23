//! tmcl-game：版本清单、版本 JSON、依赖库/资源/natives 补全。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。

pub mod arguments;
pub mod install;
pub mod manifest;
pub mod rules;
pub mod version;

pub use install::{GameFiles, InstallOptions, InstallReport, install_version};
pub use manifest::{
    MANIFEST_MAX_AGE, MANIFEST_URL, ManifestEntry, ManifestPolicy, ManifestStore, VersionManifest,
    VersionType, load_manifest,
};
pub use rules::{RuleEnv, rules_allow};
pub use version::{Library, LibraryFile, VersionJson, maven_path, parse_version, resolve};
