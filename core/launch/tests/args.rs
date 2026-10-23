//! 启动命令拼装的离线测试。

use std::collections::BTreeMap;
use std::path::PathBuf;

use tmcl_common::DataPaths;
use tmcl_game::{GameFiles, RuleEnv, VersionJson, parse_version, resolve};
use tmcl_launch::args::{LaunchAccount, LaunchSpec, build_command_for};
use tmcl_launch::classpath::classpath;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/versions")
        .join(name);
    std::fs::read(path).unwrap()
}

fn windows() -> RuleEnv {
    RuleEnv {
        os_name: "windows",
        arch: "x86_64",
        os_version: None,
        features: BTreeMap::new(),
    }
}

fn spec(version_json: &[u8], id: &str) -> LaunchSpec {
    LaunchSpec {
        java: PathBuf::from("java"),
        version: parse_version(id, version_json).unwrap(),
        game_dir: PathBuf::from("game"),
        game_assets_dir: PathBuf::from("root/shared/assets"),
        natives_dir: PathBuf::from("natives"),
        account: LaunchAccount {
            name: "Dev".into(),
            uuid: "380df991f603344ca090369bad2a924a".into(),
            access_token: "secret-token-value".into(),
            user_type: "legacy".into(),
        },
        min_memory_mb: 512,
        max_memory_mb: 2048,
        jvm_args: Vec::new(),
        game_args: Vec::new(),
        window: None,
        fullscreen: false,
    }
}

fn value_after<'a>(args: &'a [String], flag: &str) -> &'a str {
    let i = args.iter().position(|a| a == flag).unwrap();
    &args[i + 1]
}

#[test]
fn builds_1_20_1_command_for_windows() {
    let files = GameFiles::new(DataPaths::at("root"));
    let spec = spec(&fixture("1.20.1.json"), "1.20.1");
    let cmd = build_command_for(&files, &spec, &windows());
    let args = &cmd.args;

    assert_eq!(&args[..2], ["-Xms512M", "-Xmx2048M"]);
    assert!(args.contains(&"-Djava.library.path=natives".to_owned()));
    assert!(args.contains(&"-Dminecraft.launcher.brand=TMCL".to_owned()));
    // 只有 macOS 需要 -XstartOnFirstThread，只有 32 位需要 -Xss1M。
    assert!(
        !args
            .iter()
            .any(|a| a == "-XstartOnFirstThread" || a == "-Xss1M")
    );
    assert!(args.iter().any(|a| a.starts_with("-XX:HeapDumpPath=")));

    let cp = value_after(args, "-cp");
    let entries: Vec<&str> = cp.split(';').collect();
    assert_eq!(entries.len(), 65, "64 个库 + 客户端 jar");
    assert_eq!(
        entries.len(),
        classpath(&files, &spec.version, &windows()).len()
    );
    assert!(entries.last().unwrap().ends_with("1.20.1.jar"));
    assert!(entries.iter().any(|e| e.contains("natives-windows")));
    assert!(!entries.iter().any(|e| e.contains("natives-linux")));

    let log_arg = args
        .iter()
        .find(|a| a.starts_with("-Dlog4j.configurationFile="))
        .unwrap();
    assert!(log_arg.ends_with("client-1.12.xml"));

    let main = args
        .iter()
        .position(|a| a == "net.minecraft.client.main.Main")
        .unwrap();
    assert!(args.iter().position(|a| a == "-cp").unwrap() < main);
    assert_eq!(value_after(args, "--username"), "Dev");
    assert_eq!(value_after(args, "--version"), "1.20.1");
    assert_eq!(value_after(args, "--gameDir"), "game");
    assert_eq!(value_after(args, "--assetIndex"), "5");
    assert_eq!(
        value_after(args, "--uuid"),
        "380df991f603344ca090369bad2a924a"
    );
    assert_eq!(value_after(args, "--accessToken"), "secret-token-value");
    assert_eq!(value_after(args, "--userType"), "legacy");
    assert_eq!(value_after(args, "--versionType"), "release");
    assert!(!args.iter().any(|a| a.contains("${")), "不应残留占位符");
    assert!(
        !args
            .iter()
            .any(|a| a == "--demo" || a == "--width" || a.starts_with("--quickPlay"))
    );
    assert_eq!(cmd.cwd, PathBuf::from("game"));
}

#[test]
fn custom_resolution_adds_width_and_height() {
    let files = GameFiles::new(DataPaths::at("root"));
    let mut spec = spec(&fixture("1.20.1.json"), "1.20.1");
    spec.window = Some((1280, 720));
    let env = windows().with_feature("has_custom_resolution", true);
    let cmd = build_command_for(&files, &spec, &env);
    assert_eq!(value_after(&cmd.args, "--width"), "1280");
    assert_eq!(value_after(&cmd.args, "--height"), "720");
}

#[test]
fn redacted_command_hides_token() {
    let files = GameFiles::new(DataPaths::at("root"));
    let cmd = build_command_for(&files, &spec(&fixture("1.20.1.json"), "1.20.1"), &windows());
    let line = cmd.redacted();
    assert!(!line.contains("secret-token-value"));
    assert!(line.contains("--accessToken ***"));
}

#[test]
fn legacy_minecraft_arguments_are_supported() {
    let json = br#"{
        "id": "old", "type": "release", "mainClass": "net.minecraft.client.Minecraft",
        "minecraftArguments": "--username ${auth_player_name} --session ${auth_session} --assetsDir ${game_assets}",
        "assets": "legacy",
        "libraries": [{"name": "a:b:1", "downloads": {"artifact":
            {"path": "a/b/1/b-1.jar", "sha1": "00", "size": 1, "url": "https://example.invalid/b.jar"}}}]
    }"#;
    let files = GameFiles::new(DataPaths::at("root"));
    let mut spec = spec(json, "old");
    spec.window = Some((800, 600));
    let linux = RuleEnv {
        os_name: "linux",
        ..windows()
    }
    .with_feature("has_custom_resolution", true);
    let cmd = build_command_for(&files, &spec, &linux);
    let args = &cmd.args;
    assert!(args.contains(&"-Djava.library.path=natives".to_owned()));
    assert_eq!(value_after(args, "-cp").split(':').count(), 2);
    assert_eq!(value_after(args, "--username"), "Dev");
    assert_eq!(value_after(args, "--session"), "secret-token-value");
    assert_eq!(value_after(args, "--width"), "800");
    let main = args
        .iter()
        .position(|a| a == "net.minecraft.client.Minecraft")
        .unwrap();
    assert!(main < args.iter().position(|a| a == "--username").unwrap());
}

fn resolved(id: &str) -> VersionJson {
    resolve(id, |v| parse_version(v, &fixture(&format!("{v}.json")))).unwrap()
}

#[test]
fn custom_args_and_fullscreen_are_appended() {
    let files = GameFiles::new(DataPaths::at("root"));
    let mut spec = spec(&fixture("1.20.1.json"), "1.20.1");
    spec.jvm_args = vec!["-XX:+UseG1GC".into(), "-Dgame.dir=${game_directory}".into()];
    spec.game_args = vec!["--server".into(), "example.org".into()];
    spec.window = Some((1280, 720));
    spec.fullscreen = true;
    let cmd = build_command_for(&files, &spec, &windows());
    let args = &cmd.args;
    let pos = |a: &str| args.iter().position(|x| x == a).unwrap();
    assert!(pos("-cp") < pos("-XX:+UseG1GC"));
    assert!(pos("-XX:+UseG1GC") < pos("net.minecraft.client.main.Main"));
    assert!(args.contains(&"-Dgame.dir=game".to_owned()));
    assert!(pos("--fullscreen") < pos("--server"));
    assert_eq!(args.last().unwrap(), "example.org");
    // 全屏时规则环境不启用 has_custom_resolution，不传窗口大小。
    assert!(!args.iter().any(|a| a == "--width"));
}

#[test]
fn fabric_inherits_vanilla_classpath_and_arguments() {
    let files = GameFiles::new(DataPaths::at("root"));
    let mut spec = spec(&fixture("1.20.1.json"), "1.20.1");
    spec.version = resolved("fabric-loader-0.20.1-1.20.1");
    let cmd = build_command_for(&files, &spec, &windows());
    let args = &cmd.args;
    assert!(args.contains(&"net.fabricmc.loader.impl.launch.knot.KnotClient".to_owned()));
    assert!(args.contains(&"-DFabricMcEmu= net.minecraft.client.main.Main ".to_owned()));
    assert_eq!(
        value_after(args, "--version"),
        "fabric-loader-0.20.1-1.20.1"
    );
    let cp: Vec<&str> = value_after(args, "-cp").split(';').collect();
    assert!(cp.iter().any(|e| e.ends_with("fabric-loader-0.20.1.jar")));
    assert!(cp.iter().any(|e| e.ends_with("intermediary-1.20.1.jar")));
    // 客户端 jar 仍是父版本的。
    assert!(cp.last().unwrap().ends_with("1.20.1.jar"));
    assert!(cp.last().unwrap().contains("versions"));
}

#[test]
fn legacy_1_7_10_uses_natives_dir_and_skips_native_jars_on_classpath() {
    let files = GameFiles::new(DataPaths::at("root"));
    let spec = spec(&fixture("1.7.10.json"), "1.7.10");
    let cmd = build_command_for(&files, &spec, &windows());
    let args = &cmd.args;
    assert!(args.contains(&"-Djava.library.path=natives".to_owned()));
    let cp = value_after(args, "-cp");
    assert!(
        !cp.contains("natives-windows"),
        "旧式 natives 只解压，不进 classpath"
    );
    assert!(cp.contains("lwjgl-2.9.1.jar"));
    assert_eq!(value_after(args, "--assetIndex"), "1.7.10");
    assert!(!args.iter().any(|a| a.contains("${")), "不应残留占位符");
}

#[test]
fn game_assets_placeholder_uses_virtual_dir() {
    let json = br#"{
        "id": "old", "type": "release", "mainClass": "net.minecraft.client.Minecraft",
        "minecraftArguments": "--assetsDir ${game_assets}", "assets": "pre-1.6", "libraries": []
    }"#;
    let files = GameFiles::new(DataPaths::at("root"));
    let mut spec = spec(json, "old");
    spec.game_assets_dir = PathBuf::from("game/resources");
    let cmd = build_command_for(&files, &spec, &windows());
    assert_eq!(value_after(&cmd.args, "--assetsDir"), "game/resources");
}

#[test]
fn real_token_is_listed_as_secret_but_offline_placeholder_is_not() {
    let files = GameFiles::new(DataPaths::at("root"));
    let mut spec = spec(&fixture("1.20.1.json"), "1.20.1");
    let cmd = build_command_for(&files, &spec, &windows());
    assert_eq!(cmd.secrets, vec!["secret-token-value".to_owned()]);
    spec.account = LaunchAccount::offline("Dev", "380df991f603344ca090369bad2a924a");
    let cmd = build_command_for(&files, &spec, &windows());
    assert!(cmd.secrets.is_empty());
    assert_eq!(value_after(&cmd.args, "--accessToken"), "0");
}
