//! 用 `tests/fixtures/versions/` 中的真实版本 JSON 检查解析、继承合并、规则与参数展开。
//!
//! 期望值由独立的 Python 脚本按官方启动器语义算出，再与这里的实现对照。

use std::path::PathBuf;

use tmcl_common::{AppError, AppResult, ErrorCode};
use tmcl_game::{RuleEnv, VersionJson, parse_version, resolve};

fn load(id: &str) -> AppResult<VersionJson> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/versions")
        .join(format!("{id}.json"));
    let bytes = std::fs::read(path)
        .map_err(|_| AppError::new(ErrorCode::GameVersionNotFound).with_param("version", id))?;
    parse_version(id, &bytes)
}

fn resolved(id: &str) -> VersionJson {
    resolve(id, load).unwrap()
}

fn windows() -> RuleEnv {
    RuleEnv::new("windows", "x86_64").with_os_version("10.0")
}

fn linux() -> RuleEnv {
    RuleEnv::new("linux", "x86_64")
}

fn macos() -> RuleEnv {
    RuleEnv::new("osx", "arm64").with_os_version("14.5")
}

/// 版本号、主类、Java 主版本、三平台适用库数量、游戏参数数量、三平台 JVM 参数数量、库总数。
type Expected = (
    &'static str,
    &'static str,
    u32,
    [usize; 3],
    usize,
    [usize; 3],
    usize,
);

const LAUNCHWRAPPER: &str = "net.minecraft.launchwrapper.Launch";
const VANILLA: &str = "net.minecraft.client.main.Main";

const EXPECTED: &[Expected] = &[
    ("a1.0.4", LAUNCHWRAPPER, 8, [10, 10, 10], 8, [8, 5, 6], 13),
    ("b1.7.3", LAUNCHWRAPPER, 8, [10, 10, 10], 6, [8, 5, 6], 13),
    ("1.7.10", VANILLA, 8, [33, 31, 32], 18, [8, 5, 6], 33),
    ("1.12.2", VANILLA, 8, [34, 34, 36], 18, [8, 5, 6], 39),
    ("1.16.5", VANILLA, 8, [41, 41, 43], 18, [8, 5, 6], 57),
    ("1.20.1", VANILLA, 17, [64, 52, 58], 22, [9, 8, 9], 88),
    ("26.3", VANILLA, 25, [74, 66, 80], 20, [13, 12, 13], 114),
    (
        "26.4-snapshot-5",
        VANILLA,
        25,
        [74, 66, 80],
        20,
        [13, 12, 13],
        114,
    ),
    (
        "fabric-loader-0.20.1-1.20.1",
        "net.fabricmc.loader.impl.launch.knot.KnotClient",
        17,
        [72, 60, 66],
        22,
        [10, 9, 10],
        96,
    ),
    (
        "1.20.1-forge-47.4.0",
        "cpw.mods.bootstraplauncher.BootstrapLauncher",
        17,
        [93, 81, 87],
        32,
        [25, 24, 25],
        117,
    ),
    (
        "1.12.2-forge-14.23.5.2860",
        LAUNCHWRAPPER,
        8,
        [53, 53, 55],
        20,
        [8, 5, 6],
        58,
    ),
];

#[test]
fn every_fixture_matches_expected_counts() {
    for &(id, main_class, java, libs, game, jvm, total) in EXPECTED {
        let v = resolved(id);
        assert_eq!(v.id, id);
        assert_eq!(v.main_class, main_class, "{id}");
        assert_eq!(v.java_major(), java, "{id}");
        assert_eq!(v.inherits_from, None, "{id}");
        assert_eq!(v.libraries.len(), total, "{id} libraries");
        for (env, (want_libs, want_jvm)) in [windows(), linux(), macos()]
            .iter()
            .zip(libs.into_iter().zip(jvm))
        {
            assert_eq!(
                v.libraries_for(env).count(),
                want_libs,
                "{id} {} libs",
                env.os_name
            );
            assert_eq!(
                v.jvm_arguments(env).len(),
                want_jvm,
                "{id} {} jvm",
                env.os_name
            );
        }
        assert_eq!(v.game_arguments(&windows()).len(), game, "{id} game");
        assert!(v.asset_index.is_some(), "{id} asset index");
        assert!(
            v.jvm_arguments(&windows())
                .iter()
                .any(|a| a == "${classpath}"),
            "{id}"
        );
    }
}

#[test]
fn version_types() {
    assert_eq!(resolved("a1.0.4").kind, "old_alpha");
    assert_eq!(resolved("b1.7.3").kind, "old_beta");
    assert_eq!(resolved("26.4-snapshot-5").kind, "snapshot");
    assert_eq!(resolved("26.3").kind, "release");
}

#[test]
fn legacy_arguments() {
    let alpha = resolved("a1.0.4");
    let game = alpha.game_arguments(&windows());
    assert_eq!(game[..2], ["${auth_player_name}", "${auth_session}"]);
    assert!(game.contains(&"--tweakClass".to_owned()));
    let sized = alpha.game_arguments(&windows().with_feature("has_custom_resolution", true));
    assert_eq!(sized.len(), game.len() + 4);
    assert_eq!(sized[sized.len() - 4], "--width");

    let v = resolved("1.12.2");
    let jvm = v.jvm_arguments(&macos());
    assert_eq!(jvm[0], "-XstartOnFirstThread");
    assert!(jvm.contains(&"-Djava.library.path=${natives_directory}".to_owned()));
    assert!(v.logging.as_ref().and_then(|l| l.client.as_ref()).is_some());
}

#[test]
fn modern_arguments_follow_rules() {
    let v = resolved("1.20.1");
    assert!(
        v.jvm_arguments(&windows())
            .iter()
            .any(|a| a.starts_with("-XX:HeapDumpPath="))
    );
    // 1.16.5 只在 os.version 为 10.x 时加入 Windows 10 伪装参数，Windows 7 是 6.1。
    let old = resolved("1.16.5");
    assert!(
        old.jvm_arguments(&windows())
            .contains(&"-Dos.name=Windows 10".to_owned())
    );
    let win7 = old.jvm_arguments(&RuleEnv::new("windows", "x86_64").with_os_version("6.1"));
    assert!(!win7.contains(&"-Dos.name=Windows 10".to_owned()));
    assert!(
        v.jvm_arguments(&macos())
            .contains(&"-XstartOnFirstThread".to_owned())
    );
    assert!(
        v.jvm_arguments(&RuleEnv::new("linux", "x86"))
            .contains(&"-Xss1M".to_owned())
    );

    let demo = v.game_arguments(&windows().with_feature("is_demo_user", true));
    assert!(demo.contains(&"--demo".to_owned()));
    assert!(!v.game_arguments(&windows()).contains(&"--demo".to_owned()));
}

#[test]
fn os_version_rule_picks_lwjgl_for_old_macos() {
    let v = resolved("b1.7.3");
    let lwjgl = |env: &RuleEnv| {
        v.libraries_for(env)
            .filter(|l| l.name.starts_with("org.lwjgl.lwjgl:lwjgl:"))
            .map(|l| l.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(lwjgl(&macos()), ["org.lwjgl.lwjgl:lwjgl:2.9.0"]);
    let leopard = RuleEnv::new("osx", "x86_64").with_os_version("10.5.8");
    assert_eq!(
        lwjgl(&leopard),
        ["org.lwjgl.lwjgl:lwjgl:2.9.1-nightly-20130708-debug3"]
    );
}

#[test]
fn fabric_inherits_vanilla() {
    let v = resolved("fabric-loader-0.20.1-1.20.1");
    let parent = resolved("1.20.1");
    assert_eq!(v.jar_id(), "1.20.1");
    assert_eq!(v.kind, "release");
    assert_eq!(v.asset_index, parent.asset_index);
    assert_eq!(v.downloads, parent.downloads);
    assert_eq!(v.logging, parent.logging);
    // 子版本的库在前，并且能按 Maven 坐标推算出下载地址。
    let first = v.libraries[0].file().unwrap();
    assert_eq!(first.path, "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar");
    assert_eq!(
        first.url.as_deref(),
        Some("https://maven.fabricmc.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar")
    );
    assert_eq!(first.size, Some(126_151));
    assert!(first.sha1.is_some());
    // 父版本参数在前，加载器的 JVM 参数追加在后。
    let jvm = v.jvm_arguments(&windows());
    assert_eq!(
        jvm.last().map(String::as_str),
        Some("-DFabricMcEmu= net.minecraft.client.main.Main ")
    );
    assert_eq!(
        v.game_arguments(&windows()),
        parent.game_arguments(&windows())
    );
    assert!(v.libraries.iter().all(|l| l.file().is_some()));
}

#[test]
fn forge_inherits_and_overrides() {
    let v = resolved("1.20.1-forge-47.4.0");
    assert_eq!(v.jar_id(), "1.20.1");
    // Forge 的 `logging` 是空对象，沿用父版本的日志配置。
    assert!(v.logging.as_ref().and_then(|l| l.client.as_ref()).is_some());
    let game = v.game_arguments(&windows());
    assert_eq!(
        game[game.len() - 10..][..2],
        ["--launchTarget", "forgeclient"]
    );
    assert!(
        v.jvm_arguments(&windows())
            .contains(&"ALL-MODULE-PATH".to_owned())
    );

    let legacy = resolved("1.12.2-forge-14.23.5.2860");
    assert!(legacy.minecraft_arguments.as_deref().is_some_and(|a| {
        a.contains("--tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker")
    }));
    // Forge 自带的 log4j 2.15.0 替换了原版的旧版本。
    let log4j: Vec<_> = legacy
        .libraries
        .iter()
        .filter(|l| l.name.starts_with("org.apache.logging.log4j:log4j-core:"))
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(log4j, ["org.apache.logging.log4j:log4j-core:2.15.0"]);
    // 安装器生成的 Forge 主库没有下载地址。
    let forge = legacy.libraries[0].file().unwrap();
    assert_eq!(forge.url, None);
}
