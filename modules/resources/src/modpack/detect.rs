//! 识别拖入的压缩包是哪种包：Modrinth `.mrpack`、CurseForge 整合包、TMCL 实例包 `.tmclpack`，
//! 或其他启动器（MultiMC / Prism Launcher）导出的实例。只看 zip 中的文件名，与扩展名无关。

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tmcl_common::AppResult;
use ts_rs::TS;
use zip::ZipArchive;

use super::curseforge::MANIFEST_FILE;
use super::mrpack::INDEX_FILE;

/// `.tmclpack` 根目录的配置文件，见 `tmcl_instance` 的导入导出。
const TMCLPACK_CONFIG: &str = "instance.json";

/// MultiMC / Prism Launcher 实例目录中的标志文件。
const MULTIMC_FILES: [&str; 2] = ["instance.cfg", "mmc-pack.json"];

/// 识别出的包类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum PackKind {
    /// TMCL 实例包（根目录有 `instance.json`），用 `instance_import` 导入。
    Tmclpack,
    /// Modrinth 整合包（根目录有 `modrinth.index.json`）。
    Mrpack,
    /// CurseForge 整合包（根目录有 `manifestType` 为 `minecraftModpack` 的 `manifest.json`）。
    Curseforge,
    /// MultiMC / Prism Launcher 导出的实例（有 `instance.cfg` 或 `mmc-pack.json`），暂不支持导入。
    Multimc,
    /// 不是 zip，或不是以上任何一种。
    Unknown,
}

/// 识别 `path` 指向的包；只读取 zip 目录与 CurseForge 清单，不解压。
///
/// 文件不存在或无法读取返回 `COMMON_IO`；不是 zip 返回 [`PackKind::Unknown`]。
pub fn detect(path: &Path) -> AppResult<PackKind> {
    let file = File::open(path).map_err(|e| crate::install::io_error(path, &e))?;
    let Ok(mut archive) = ZipArchive::new(BufReader::new(file)) else {
        return Ok(PackKind::Unknown);
    };
    if archive.index_for_name(INDEX_FILE).is_some() {
        return Ok(PackKind::Mrpack);
    }
    if is_curseforge_manifest(&mut archive) {
        return Ok(PackKind::Curseforge);
    }
    if archive.index_for_name(TMCLPACK_CONFIG).is_some() {
        return Ok(PackKind::Tmclpack);
    }
    if archive
        .file_names()
        .any(|name| name.is_ok_and(|n| is_multimc_file(&n)))
    {
        return Ok(PackKind::Multimc);
    }
    Ok(PackKind::Unknown)
}

/// 根目录有 `manifest.json` 且 `manifestType` 为 `minecraftModpack`；清单损坏时不算。
fn is_curseforge_manifest<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> bool {
    let Ok(mut entry) = archive.by_name(MANIFEST_FILE) else {
        return false;
    };
    let mut text = String::new();
    if entry.read_to_string(&mut text).is_err() {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}'))
        .ok()
        .and_then(|v| {
            v.get("manifestType")?
                .as_str()
                .map(|s| s == "minecraftModpack")
        })
        .unwrap_or(false)
}

/// MultiMC 导出的 zip 常把实例放在一个顶层目录里，所以根目录与第一层目录都算。
fn is_multimc_file(name: &str) -> bool {
    let parts: Vec<&str> = name.split('/').collect();
    match parts.as_slice() {
        [file] | [_, file] => MULTIMC_FILES.contains(file),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    fn zip_with(dir: &Path, entries: &[(&str, &str)]) -> std::path::PathBuf {
        let path = dir.join("pack.zip");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        for (name, body) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    fn kind(entries: &[(&str, &str)]) -> PackKind {
        let dir = tempfile::tempdir().unwrap();
        detect(&zip_with(dir.path(), entries)).unwrap()
    }

    #[test]
    fn recognizes_each_kind() {
        assert_eq!(kind(&[("modrinth.index.json", "{}")]), PackKind::Mrpack);
        assert_eq!(
            kind(&[("manifest.json", r#"{"manifestType":"minecraftModpack"}"#)]),
            PackKind::Curseforge
        );
        assert_eq!(
            kind(&[(
                "manifest.json",
                "\u{feff}{\"manifestType\":\"minecraftModpack\"}"
            )]),
            PackKind::Curseforge
        );
        assert_eq!(
            kind(&[("instance.json", "{}"), ("game/options.txt", "")]),
            PackKind::Tmclpack
        );
        assert_eq!(
            kind(&[
                ("My Pack/instance.cfg", ""),
                ("My Pack/.minecraft/options.txt", "")
            ]),
            PackKind::Multimc
        );
        assert_eq!(kind(&[("mmc-pack.json", "{}")]), PackKind::Multimc);
    }

    #[test]
    fn unknown_for_other_zips_and_non_zips() {
        assert_eq!(kind(&[("readme.txt", "hi")]), PackKind::Unknown);
        assert_eq!(
            kind(&[("manifest.json", r#"{"name":"x"}"#)]),
            PackKind::Unknown
        );
        assert_eq!(kind(&[("a/b/instance.cfg", "")]), PackKind::Unknown);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skin.png");
        std::fs::write(&path, b"\x89PNG").unwrap();
        assert_eq!(detect(&path).unwrap(), PackKind::Unknown);
    }

    #[test]
    fn missing_file_is_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = detect(&dir.path().join("nope.zip")).unwrap_err();
        assert_eq!(err.code, tmcl_common::ErrorCode::CommonIo);
    }
}
