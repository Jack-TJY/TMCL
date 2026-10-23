//! natives 解压的离线测试：旧式排除前缀、新式只取本地库并放到根目录、跳出目录的条目被忽略。

use std::io::Write;
use std::path::Path;

use serde_json::json;
use tmcl_common::{DataPaths, ErrorCode};
use tmcl_game::{GameFiles, RuleEnv, VersionJson, extract_natives};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

fn write_jar(path: &Path, entries: &[(&str, &[u8])]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, data) in entries {
        if name.ends_with('/') {
            zip.add_directory(*name, SimpleFileOptions::default())
                .unwrap();
        } else {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
    }
    zip.finish().unwrap();
}

fn version() -> VersionJson {
    serde_json::from_value(json!({
        "id": "v", "mainClass": "Main",
        "libraries": [
            {"name": "old:platform:1",
             "natives": {"linux": "natives-linux"},
             "extract": {"exclude": ["META-INF/"]},
             "downloads": {"classifiers": {"natives-linux": {
                 "path": "old/platform/1/platform-1-natives-linux.jar",
                 "sha1": "00", "size": 1, "url": "https://example.invalid/old.jar"}}}},
            {"name": "org.lwjgl:lwjgl:3.3.1:natives-linux",
             "downloads": {"artifact": {
                 "path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar",
                 "sha1": "00", "size": 1, "url": "https://example.invalid/new.jar"}}},
            {"name": "org.lwjgl:lwjgl:3.3.1:natives-windows",
             "rules": [{"action": "allow", "os": {"name": "windows"}}],
             "downloads": {"artifact": {
                 "path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar",
                 "sha1": "00", "size": 1, "url": "https://example.invalid/win.jar"}}},
            {"name": "org.lwjgl:lwjgl:3.3.1",
             "downloads": {"artifact": {
                 "path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar",
                 "sha1": "00", "size": 1, "url": "https://example.invalid/lwjgl.jar"}}},
        ],
    }))
    .unwrap()
}

#[test]
fn extracts_old_and_new_style_natives() {
    let dir = tempfile::tempdir().unwrap();
    let files = GameFiles::new(DataPaths::at(dir.path()));
    write_jar(
        &files.library("old/platform/1/platform-1-natives-linux.jar"),
        &[
            ("META-INF/", b""),
            ("META-INF/MANIFEST.MF", b"manifest"),
            ("liblwjgl.so", b"old lwjgl"),
            ("sub/libopenal.so", b"openal"),
            ("../escape.so", b"evil"),
        ],
    );
    write_jar(
        &files.library("org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar"),
        &[
            ("META-INF/MANIFEST.MF", b"manifest"),
            ("linux/x64/org/lwjgl/liblwjgl3.so", b"new lwjgl"),
            ("linux/x64/org/lwjgl/liblwjgl3.so.sha1", b"hash"),
        ],
    );

    let dest = dir.path().join("natives");
    let count =
        extract_natives(&files, &version(), &RuleEnv::new("linux", "x86_64"), &dest).unwrap();
    assert_eq!(count, 3);
    assert_eq!(
        std::fs::read(dest.join("liblwjgl.so")).unwrap(),
        b"old lwjgl"
    );
    assert_eq!(
        std::fs::read(dest.join("sub/libopenal.so")).unwrap(),
        b"openal"
    );
    assert_eq!(
        std::fs::read(dest.join("liblwjgl3.so")).unwrap(),
        b"new lwjgl"
    );
    assert!(!dest.join("META-INF").exists());
    assert!(!dest.join("liblwjgl3.so.sha1").exists());
    assert!(!dir.path().join("escape.so").exists());
}

#[test]
fn missing_or_broken_jar_is_io_error() {
    let dir = tempfile::tempdir().unwrap();
    let files = GameFiles::new(DataPaths::at(dir.path()));
    let env = RuleEnv::new("linux", "x86_64");
    let dest = dir.path().join("natives");
    let err = extract_natives(&files, &version(), &env, &dest).unwrap_err();
    assert_eq!(err.code, ErrorCode::CommonIo);

    let jar = files.library("old/platform/1/platform-1-natives-linux.jar");
    std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
    std::fs::write(&jar, b"not a zip").unwrap();
    let err = extract_natives(&files, &version(), &env, &dest).unwrap_err();
    assert_eq!(err.code, ErrorCode::CommonIo);
}
