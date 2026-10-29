//! 游戏版本的数据版本号（`options.txt` 第一行的 `version:<n>`）。
//!
//! 1.13 起游戏读取 `options.txt` 时按 `version` 运行数据修复器（DataFixer）；缺少这一行时按
//! 版本 0 处理，从最早的修复器开始逐个运行，会把已经是新格式的值再“升级”一遍。因此在实例
//! 首次启动前新建 `options.txt` 时要写入正确的数据版本。
//!
//! 数据版本来自客户端 jar 中的 `version.json`（`world_version`，18w47b / 1.14 起才有）；
//! 1.13–1.13.2 没有该文件，使用下表。

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// 1.13.x 正式版的数据版本（这几个版本的 jar 中没有 `version.json`）。
const KNOWN: &[(&str, u32)] = &[("1.13", 1519), ("1.13.1", 1628), ("1.13.2", 1631)];

/// `version.json` 中用到的字段。
#[derive(serde::Deserialize)]
struct VersionInfo {
    world_version: u32,
}

/// 游戏版本的数据版本：先查表，再读客户端 jar；都拿不到时为 `None`。
pub fn data_version(game_version: &str, client_jar: &Path) -> Option<u32> {
    KNOWN
        .iter()
        .find(|(v, _)| *v == game_version)
        .map(|(_, n)| *n)
        .or_else(|| read_jar(client_jar))
}

fn read_jar(client_jar: &Path) -> Option<u32> {
    let file = File::open(client_jar).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut entry = archive.by_name("version.json").ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    serde_json::from_str::<VersionInfo>(&text)
        .ok()
        .map(|info| info.world_version)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    fn jar(dir: &Path, version_json: Option<&str>) -> std::path::PathBuf {
        let path = dir.join("client.jar");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        if let Some(json) = version_json {
            zip.start_file("version.json", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(json.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn reads_world_version_from_jar() {
        let dir = tempfile::tempdir().unwrap();
        let path = jar(
            dir.path(),
            Some(r#"{"id":"1.20.1","world_version":3465,"protocol_version":763}"#),
        );
        assert_eq!(data_version("1.20.1", &path), Some(3465));
    }

    #[test]
    fn falls_back_to_table_and_none() {
        let dir = tempfile::tempdir().unwrap();
        let empty = jar(dir.path(), None);
        assert_eq!(data_version("1.13.2", &empty), Some(1631));
        assert_eq!(data_version("1.12.2", &empty), None);
        assert_eq!(
            data_version("1.20.1", &dir.path().join("missing.jar")),
            None
        );
    }
}
