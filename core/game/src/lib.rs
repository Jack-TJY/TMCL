//! tmcl-game：版本清单、版本 JSON、依赖库/资源/natives 补全。
//!
//! 核心层 crate，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。

pub mod arguments;
pub mod assets;
pub mod install;
pub mod libraries;
pub mod manifest;
pub mod natives;
pub mod rules;
pub mod version;

pub use assets::{AssetIndex, AssetObject, game_assets_dir, prepare_legacy_assets};
pub use install::{
    GameFiles, InstallOptions, InstallReport, VerifyReport, install_version, repair_version,
    verify_version,
};
pub use libraries::{GameFile, library_files, library_items};
pub use manifest::{
    LoadedManifest, MANIFEST_MAX_AGE, MANIFEST_URL, ManifestEntry, ManifestPolicy, ManifestStore,
    VersionManifest, VersionType, load_manifest, load_manifest_checked,
};
pub use natives::{extract_natives, new_natives_dir};
pub use rules::{RuleEnv, rules_allow};
pub use version::{Library, LibraryFile, VersionJson, maven_path, parse_version, resolve};
