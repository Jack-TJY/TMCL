//! CurseForge 响应模型测试：解析 `fixtures/curseforge/` 下录制的真实响应（2026-10-30，已截短长列表）。

use std::path::Path;

use tmcl_resources::curseforge::{
    ClassId, File, FingerprintMatches, Mod, ModLoaderType, Page, RelationType, ReleaseType,
    fingerprint,
};

const SEARCH: &[u8] = include_bytes!("fixtures/curseforge/search.json");
const MOD: &[u8] = include_bytes!("fixtures/curseforge/mod.json");
const MOD_FILES: &[u8] = include_bytes!("fixtures/curseforge/mod_files.json");
const FILE: &[u8] = include_bytes!("fixtures/curseforge/file.json");
const FINGERPRINTS: &[u8] = include_bytes!("fixtures/curseforge/fingerprints.json");
const MODPACK_SEARCH: &[u8] = include_bytes!("fixtures/curseforge/modpack_search.json");

const JEI: u32 = 238_222;
const JEI_FILE: u32 = 9_352_806;

fn data<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> T {
    let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    serde_json::from_value(value["data"].clone()).unwrap()
}

#[test]
fn search_page_parses() {
    let page: Page<Mod> = serde_json::from_slice(SEARCH).unwrap();
    assert_eq!(page.pagination.total_count, 201);
    let jei = &page.data[0];
    assert_eq!((jei.id, jei.slug.as_str()), (JEI, "jei"));
    assert_eq!(jei.class_id, Some(ClassId::Mods));
    assert_eq!(jei.allow_mod_distribution, Some(true));
    assert!(!jei.game_versions().is_empty());

    let packs: Page<Mod> = serde_json::from_slice(MODPACK_SEARCH).unwrap();
    assert!(
        packs
            .data
            .iter()
            .all(|m| m.class_id == Some(ClassId::Modpacks))
    );
    assert!(packs.data[0].website_url().contains("/minecraft/modpacks/"));
}

#[test]
fn mod_links_and_file_page() {
    let jei: Mod = data(MOD);
    assert_eq!(jei.name, "Just Enough Items (JEI)");
    assert_eq!(
        jei.website_url(),
        "https://www.curseforge.com/minecraft/mc-mods/jei"
    );
    assert_eq!(
        jei.file_page_url(JEI_FILE),
        "https://www.curseforge.com/minecraft/mc-mods/jei/files/9352806"
    );
}

#[test]
fn file_exposes_hash_loaders_and_dependencies() {
    let files: Page<File> = serde_json::from_slice(MOD_FILES).unwrap();
    assert_eq!(files.data[0].id, JEI_FILE);
    assert_eq!(files.data[0].release_type, ReleaseType::Beta);

    let file: File = data(FILE);
    assert_eq!(
        file.sha1().as_deref(),
        Some("7e095c4c5b95a478002df0822975920a4201263e")
    );
    assert_eq!(file.minecraft_versions(), vec!["1.20.1".to_owned()]);
    assert_eq!(file.loaders(), vec!["forge".to_owned()]);
    assert!(
        file.dependencies
            .iter()
            .any(|d| d.relation_type == RelationType::RequiredDependency)
    );
    let item = file.download_item(Path::new("mods")).unwrap();
    assert!(item.url.starts_with("https://edge.forgecdn.net/"));
    assert_eq!(
        item.path,
        Path::new("mods").join("jei-1.20.1-forge-15.74.0.232.jar")
    );
    assert_eq!(item.size, Some(file.file_length));
}

#[test]
fn file_without_download_url_cannot_be_downloaded() {
    let mut file: File = data(FILE);
    file.download_url = None;
    assert!(file.download_item(Path::new("mods")).is_none());
    file.download_url = Some("http://edge.forgecdn.net/a.jar".into());
    assert!(file.https_download_url().is_none());
    file.download_url = Some("https://edge.forgecdn.net/a.jar".into());
    file.file_name = "../evil.jar".into();
    assert!(file.download_item(Path::new("mods")).is_none());
}

#[test]
fn fingerprints_tolerate_null_lists() {
    let matches: FingerprintMatches = data(FINGERPRINTS);
    assert_eq!(matches.exact_matches.len(), 1);
    assert_eq!(matches.exact_matches[0].id, JEI);
    assert!(matches.unmatched_fingerprints.is_empty());
}

#[test]
fn unknown_enum_values_are_kept() {
    let loader: ModLoaderType = serde_json::from_str("42").unwrap();
    assert_eq!(loader, ModLoaderType::Other(42));
    assert_eq!(loader.as_i64(), 42);
    assert_eq!(
        ModLoaderType::from_label("NeoForge"),
        Some(ModLoaderType::NeoForge)
    );
    assert_eq!(ModLoaderType::NeoForge.as_loader_str(), Some("neoforge"));
}

#[test]
fn fingerprint_of_whitespace_only_file_is_stable() {
    assert_eq!(
        fingerprint::of_bytes(b" \n\t\r"),
        fingerprint::of_bytes(b"")
    );
}
