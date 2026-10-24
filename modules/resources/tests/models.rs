//! 用录制的 Modrinth 真实响应（`fixtures/modrinth/`，2026-10-24，长文本与列表已截短）
//! 检查响应结构的解析。

use std::path::Path;

use tmcl_resources::modrinth::{
    DependencyType, Project, ProjectDependencies, ProjectType, SearchResponse, SideSupport,
    Version, VersionType,
};

const SEARCH: &[u8] = include_bytes!("fixtures/modrinth/search.json");
const PROJECT: &[u8] = include_bytes!("fixtures/modrinth/project.json");
const PROJECT_VERSIONS: &[u8] = include_bytes!("fixtures/modrinth/project_versions.json");
const DEPENDENCIES: &[u8] = include_bytes!("fixtures/modrinth/dependencies.json");

#[test]
fn search_response_is_parsed() {
    let result: SearchResponse = serde_json::from_slice(SEARCH).unwrap();
    assert_eq!(result.total_hits, 39);
    assert_eq!(result.hits.len(), 2);
    let hit = &result.hits[0];
    assert_eq!(hit.project_id, "AANobbMI");
    assert_eq!(hit.slug, "sodium");
    assert_eq!(hit.project_type, ProjectType::Mod);
    assert_eq!(hit.client_side, SideSupport::Required);
    assert_eq!(hit.server_side, SideSupport::Unsupported);
    assert_eq!(hit.latest_version.as_deref(), Some("OihdIimA"));
    assert!(hit.categories.contains(&"fabric".to_owned()));
}

#[test]
fn project_is_parsed() {
    let project: Project = serde_json::from_slice(PROJECT).unwrap();
    assert_eq!(project.id, "AANobbMI");
    assert_eq!(project.title, "Sodium");
    assert_eq!(project.license.id, "LicenseRef-Polyform-Shield-1.0.0");
    assert_eq!(project.loaders, ["fabric", "neoforge", "quilt"]);
    assert_eq!(project.versions.len(), 3);
    assert_eq!(project.gallery.len(), 1);
    assert!(project.source_url.is_some());
}

#[test]
fn versions_give_primary_file_and_download_item() {
    let versions: Vec<Version> = serde_json::from_slice(PROJECT_VERSIONS).unwrap();
    let latest = &versions[0];
    assert_eq!(latest.id, "OihdIimA");
    assert_eq!(latest.version_number, "mc1.20.1-0.5.13-fabric");
    assert_eq!(latest.version_type, VersionType::Release);
    assert_eq!(latest.game_versions, ["1.20.1"]);
    assert_eq!(latest.required_dependencies().count(), 0);

    let file = latest.primary_file().unwrap();
    assert_eq!(file.hashes.sha1, "bcdbf37d9494e405cba210d7a80ed58e756297b5");
    let item = file.download_item(Path::new("mods"));
    assert_eq!(
        item.path,
        Path::new("mods").join("sodium-fabric-0.5.13+mc1.20.1.jar")
    );
    assert_eq!(item.url, file.url);
    assert_eq!(item.size, Some(971_552));
    assert_eq!(item.sha1.as_deref(), Some(file.hashes.sha1.as_str()));
}

#[test]
fn download_item_keeps_only_the_file_name() {
    let mut versions: Vec<Version> = serde_json::from_slice(PROJECT_VERSIONS).unwrap();
    let mut file = versions.remove(0).files.remove(0);
    file.filename = "../../evil.jar".into();
    let item = file.download_item(Path::new("mods"));
    assert_eq!(item.path, Path::new("mods").join("evil.jar"));
}

#[test]
fn primary_file_falls_back_to_first_file() {
    let mut versions: Vec<Version> = serde_json::from_slice(PROJECT_VERSIONS).unwrap();
    let mut version = versions.remove(0);
    version.files[0].primary = false;
    assert_eq!(version.primary_file(), version.files.first());
    version.files.clear();
    assert_eq!(version.primary_file(), None);
}

#[test]
fn dependencies_are_parsed() {
    let deps: ProjectDependencies = serde_json::from_slice(DEPENDENCIES).unwrap();
    assert_eq!(deps.projects.len(), 1);
    assert!(deps.versions.is_empty());
}

#[test]
fn unknown_enum_values_fall_back() {
    let dep: tmcl_resources::modrinth::Dependency = serde_json::from_str(
        r#"{"version_id":null,"project_id":"P7dR8mSH","file_name":null,"dependency_type":"required"}"#,
    )
    .unwrap();
    assert_eq!(dep.dependency_type, DependencyType::Required);

    let dep: tmcl_resources::modrinth::Dependency =
        serde_json::from_str(r#"{"dependency_type":"something-new"}"#).unwrap();
    assert_eq!(dep.dependency_type, DependencyType::Other);
    assert_eq!(dep.project_id, None);

    let kind: ProjectType = serde_json::from_str(r#""minigame""#).unwrap();
    assert_eq!(kind, ProjectType::Other);
    assert_eq!(kind.as_api_str(), None);
    let side: SideSupport = serde_json::from_str(r#""unknown""#).unwrap();
    assert_eq!(side, SideSupport::Unknown);
}
