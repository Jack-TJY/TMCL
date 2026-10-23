//! 版本 JSON（`<id>.json`）的数据结构、`inheritsFrom` 合并与依赖库定位。
//!
//! 规则求值见 [`crate::rules`]，启动参数模板见 [`crate::arguments`]。
//! 旧式 natives（`natives` + `classifiers`）的下载与解压由 M1-04 处理，这里只解析字段。

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use tmcl_common::{AppError, AppResult, ErrorCode};

pub use crate::arguments::{Argument, ArgumentValue, Arguments};
pub use crate::rules::{OsRule, Rule, RuleAction, RuleEnv, rules_allow};

use crate::arguments::{expand, from_legacy, legacy_jvm};

/// 官方依赖库仓库，没有 `downloads` 也没有 `url` 的库从这里下载。
pub const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";

/// `inheritsFrom` 链的最大长度，超过视为版本 JSON 损坏。
pub const MAX_INHERIT_DEPTH: usize = 8;

/// 一个版本的完整描述。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    /// 版本号，例如 `1.20.1`。
    pub id: String,
    /// 版本类型，例如 `release`。
    #[serde(rename = "type", default)]
    pub kind: String,
    /// 主类；继承版本可以省略，由父版本提供。
    #[serde(default)]
    pub main_class: String,
    /// 继承的父版本（加载器版本使用）；合并后为链上下一个未合并的父版本。
    #[serde(default)]
    pub inherits_from: Option<String>,
    /// 客户端 jar 所属的版本号；继承版本没有自己的 jar 时为父版本号。
    #[serde(default)]
    pub jar: Option<String>,
    /// 新式启动参数（1.13 起）。
    #[serde(default)]
    pub arguments: Option<Arguments>,
    /// 旧式启动参数（1.12.2 及以前），以空格分隔。
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    /// 资源索引。
    #[serde(default)]
    pub asset_index: Option<AssetIndexRef>,
    /// 资源索引 ID（与 `asset_index.id` 相同）。
    #[serde(default)]
    pub assets: Option<String>,
    /// 客户端 jar 等下载项。
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    /// 依赖库。
    #[serde(default)]
    pub libraries: Vec<Library>,
    /// 推荐的 Java 版本。
    #[serde(default)]
    pub java_version: Option<JavaVersion>,
    /// 日志配置。
    #[serde(default)]
    pub logging: Option<Logging>,
    /// 发布时间（RFC 3339）。
    #[serde(default)]
    pub release_time: Option<String>,
}

impl VersionJson {
    /// 把父版本合并进来，规则与官方启动器一致：
    ///
    /// - 标量字段（主类、资源索引、下载项、Java 版本、日志配置等）子版本有值时用子版本的；
    /// - 依赖库子版本在前，父版本中与子版本同名（`group:artifact[:classifier]`）的库被去掉；
    /// - `arguments.game` / `arguments.jvm` 按父版本在前拼接。
    pub fn inherit(self, parent: VersionJson) -> VersionJson {
        let child_keys: BTreeSet<String> = self
            .libraries
            .iter()
            .map(|l| library_key(&l.name))
            .collect();
        let mut libraries = self.libraries;
        libraries.extend(
            parent
                .libraries
                .into_iter()
                .filter(|l| !child_keys.contains(&library_key(&l.name))),
        );
        let arguments = match (parent.arguments, self.arguments) {
            (Some(mut p), Some(c)) => {
                p.game.extend(c.game);
                p.jvm.extend(c.jvm);
                Some(p)
            }
            (p, c) => c.or(p),
        };
        let jar = match (&self.jar, &self.downloads) {
            (Some(jar), _) => Some(jar.clone()),
            (None, Some(_)) => None,
            (None, None) => Some(parent.jar.unwrap_or_else(|| parent.id.clone())),
        };
        let logging = self
            .logging
            .filter(|l| l.client.is_some())
            .or(parent.logging);
        VersionJson {
            id: self.id,
            kind: non_empty(self.kind, parent.kind),
            main_class: non_empty(self.main_class, parent.main_class),
            inherits_from: parent.inherits_from,
            jar,
            arguments,
            minecraft_arguments: self.minecraft_arguments.or(parent.minecraft_arguments),
            asset_index: self.asset_index.or(parent.asset_index),
            assets: self.assets.or(parent.assets),
            downloads: self.downloads.or(parent.downloads),
            libraries,
            java_version: self.java_version.or(parent.java_version),
            logging,
            release_time: self.release_time.or(parent.release_time),
        }
    }

    /// 客户端 jar 所属的版本号。
    pub fn jar_id(&self) -> &str {
        self.jar.as_deref().unwrap_or(&self.id)
    }

    /// 需要的 Java 主版本号；旧版本 JSON 没有 `javaVersion` 时为 8。
    pub fn java_major(&self) -> u32 {
        self.java_version.as_ref().map_or(8, |j| j.major_version)
    }

    /// 适用于 `env` 的依赖库，按版本 JSON 中的顺序。
    pub fn libraries_for<'a>(&'a self, env: &'a RuleEnv) -> impl Iterator<Item = &'a Library> + 'a {
        self.libraries.iter().filter(move |l| l.applies(env))
    }

    /// 按规则展开的游戏参数模板。
    ///
    /// 有 `minecraftArguments` 的旧版本以它为基础（启用 `has_custom_resolution` 时追加窗口大小），
    /// 再追加加载器提供的 `arguments.game`。
    pub fn game_arguments(&self, env: &RuleEnv) -> Vec<String> {
        let extra = self
            .arguments
            .as_ref()
            .map(|a| a.game.as_slice())
            .unwrap_or_default();
        match &self.minecraft_arguments {
            Some(legacy) => {
                let mut args = expand(&from_legacy(legacy), env);
                args.extend(expand(extra, env));
                args
            }
            None => expand(extra, env),
        }
    }

    /// 按规则展开的 JVM 参数模板（不含内存参数与日志配置参数）。
    ///
    /// 旧版本没有 JVM 参数模板，使用与官方启动器相同的默认值，再追加加载器的 `arguments.jvm`。
    pub fn jvm_arguments(&self, env: &RuleEnv) -> Vec<String> {
        let extra = self
            .arguments
            .as_ref()
            .map(|a| a.jvm.as_slice())
            .unwrap_or_default();
        if self.minecraft_arguments.is_some() || self.arguments.is_none() {
            let mut args = expand(legacy_jvm(), env);
            args.extend(expand(extra, env));
            args
        } else {
            expand(extra, env)
        }
    }
}

fn non_empty(child: String, parent: String) -> String {
    if child.is_empty() { parent } else { child }
}

/// 解析版本 JSON；格式错误、或既没有 `mainClass` 也没有 `inheritsFrom` 时返回
/// `GAME_VERSION_JSON_INVALID`。
pub fn parse_version(id: &str, bytes: &[u8]) -> AppResult<VersionJson> {
    let invalid = |detail: String| {
        AppError::new(ErrorCode::GameVersionJsonInvalid)
            .with_param("version", id)
            .with_detail(detail)
    };
    let version: VersionJson = serde_json::from_slice(bytes).map_err(|e| invalid(e.to_string()))?;
    if version.main_class.is_empty() && version.inherits_from.is_none() {
        return Err(invalid("missing mainClass".to_owned()));
    }
    Ok(version)
}

/// 读取 `id` 并沿 `inheritsFrom` 逐级合并父版本，`load` 按版本号读取单个版本 JSON。
///
/// 父版本缺失时返回 `load` 的错误；出现循环或链过长时返回 `GAME_VERSION_JSON_INVALID`。
pub fn resolve(
    id: &str,
    mut load: impl FnMut(&str) -> AppResult<VersionJson>,
) -> AppResult<VersionJson> {
    let mut version = load(id)?;
    let mut chain = vec![id.to_owned()];
    while let Some(parent_id) = version.inherits_from.clone() {
        if chain.contains(&parent_id) || chain.len() >= MAX_INHERIT_DEPTH {
            return Err(AppError::new(ErrorCode::GameVersionJsonInvalid)
                .with_param("version", id)
                .with_detail(format!(
                    "inheritsFrom chain: {} -> {parent_id}",
                    chain.join(" -> ")
                )));
        }
        let parent = load(&parent_id)?;
        chain.push(parent_id);
        version = version.inherit(parent);
    }
    if version.main_class.is_empty() {
        return Err(AppError::new(ErrorCode::GameVersionJsonInvalid)
            .with_param("version", id)
            .with_detail("missing mainClass"));
    }
    Ok(version)
}

/// 资源索引引用。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    /// 索引 ID，例如 `5`。
    pub id: String,
    /// 索引文件 SHA1。
    pub sha1: String,
    /// 索引文件大小。
    pub size: u64,
    /// 所有资源文件的总大小。
    #[serde(default)]
    pub total_size: Option<u64>,
    /// 索引文件地址。
    pub url: String,
}

/// 一个可下载文件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileDownload {
    /// SHA1。
    pub sha1: String,
    /// 大小（字节）。
    pub size: u64,
    /// 地址。
    pub url: String,
}

/// 版本级下载项。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VersionDownloads {
    /// 客户端 jar。
    pub client: FileDownload,
}

/// 一个依赖库。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Library {
    /// Maven 坐标，例如 `org.lwjgl:lwjgl:3.3.1:natives-windows`。
    pub name: String,
    /// 下载信息（官方与 Forge 格式）。
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    /// Maven 仓库地址（Fabric 等只给坐标的格式）。
    #[serde(default)]
    pub url: Option<String>,
    /// 构件 SHA1（Fabric 格式）。
    #[serde(default)]
    pub sha1: Option<String>,
    /// 构件大小（Fabric 格式）。
    #[serde(default)]
    pub size: Option<u64>,
    /// 适用规则。
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// 旧式 natives：系统名 → 分类器名（M1-04 处理）。
    #[serde(default)]
    pub natives: Option<BTreeMap<String, String>>,
    /// 旧式 natives 解压时排除的路径前缀。
    #[serde(default)]
    pub extract: Option<Extract>,
}

/// 库的下载信息。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LibraryDownloads {
    /// 主构件。
    #[serde(default)]
    pub artifact: Option<Artifact>,
    /// 旧式 natives 分类器（M1-04 处理）。
    #[serde(default)]
    pub classifiers: Option<BTreeMap<String, Artifact>>,
}

/// 库构件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Artifact {
    /// 相对 `libraries/` 的路径。
    pub path: String,
    /// SHA1。
    pub sha1: String,
    /// 大小。
    pub size: u64,
    /// 地址；Forge 由安装器生成的库为空串。
    pub url: String,
}

/// 旧式 natives 的解压设置。
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct Extract {
    /// 排除的路径前缀，例如 `META-INF/`。
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// 统一后的库主构件位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFile {
    /// 相对 `libraries/` 的路径，`/` 分隔。
    pub path: String,
    /// 下载地址；`None` 表示由加载器安装器在本地生成。
    pub url: Option<String>,
    /// SHA1，未知时为 `None`。
    pub sha1: Option<String>,
    /// 大小，未知时为 `None`。
    pub size: Option<u64>,
}

impl Library {
    /// 在给定环境下是否需要该库。
    pub fn applies(&self, env: &RuleEnv) -> bool {
        rules_allow(&self.rules, env)
    }

    /// 主构件（新式格式中 natives 也是主构件）。
    pub fn artifact(&self) -> Option<&Artifact> {
        self.downloads.as_ref()?.artifact.as_ref()
    }

    /// 主构件的位置：优先 `downloads.artifact`，没有 `downloads` 时按 Maven 坐标与 `url` 推算。
    ///
    /// 只有旧式 natives、没有主构件的库返回 `None`。
    pub fn file(&self) -> Option<LibraryFile> {
        if let Some(downloads) = &self.downloads {
            let a = downloads.artifact.as_ref()?;
            return Some(LibraryFile {
                path: a.path.clone(),
                url: Some(a.url.clone()).filter(|u| !u.is_empty()),
                sha1: Some(a.sha1.clone()).filter(|s| !s.is_empty()),
                size: Some(a.size),
            });
        }
        if self.natives.is_some() {
            return None;
        }
        let path = maven_path(&self.name)?;
        let base = self.url.as_deref().unwrap_or(LIBRARIES_URL);
        Some(LibraryFile {
            url: Some(format!("{}/{path}", base.trim_end_matches('/'))),
            path,
            sha1: self.sha1.clone(),
            size: self.size,
        })
    }
}

/// Maven 坐标 `group:artifact:version[:classifier][@ext]` 对应的相对路径。
///
/// 坐标格式不对或含路径穿越片段时返回 `None`。
pub fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<&str> = coords.split(':').collect();
    let (group, artifact, version, classifier) = match parts.as_slice() {
        [g, a, v] => (*g, *a, *v, None),
        [g, a, v, c] => (*g, *a, *v, Some(*c)),
        _ => return None,
    };
    let safe = |s: &str| !s.is_empty() && s != ".." && !s.contains(['/', '\\']);
    if ![group, artifact, version, ext]
        .into_iter()
        .chain(classifier)
        .all(safe)
    {
        return None;
    }
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!(
        "{}/{artifact}/{version}/{file}",
        group.replace('.', "/")
    ))
}

/// 用于继承去重的库标识：`group:artifact[:classifier]`，不含版本与扩展名。
pub fn library_key(name: &str) -> String {
    let coords = name.split_once('@').map_or(name, |(c, _)| c);
    let parts: Vec<&str> = coords.split(':').collect();
    match parts.as_slice() {
        [g, a, _, c] => format!("{g}:{a}:{c}"),
        [g, a, ..] => format!("{g}:{a}"),
        _ => coords.to_owned(),
    }
}

/// 推荐的 Java 版本。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    /// 官方运行时组件名，例如 `java-runtime-gamma`。
    #[serde(default)]
    pub component: Option<String>,
    /// 主版本号，例如 17。
    pub major_version: u32,
}

/// `logging` 字段。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Logging {
    /// 客户端日志配置；Forge 版本 JSON 中为空对象。
    #[serde(default)]
    pub client: Option<LoggingClient>,
}

/// 客户端日志配置。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoggingClient {
    /// JVM 参数模板，含 `${path}` 占位符。
    pub argument: String,
    /// 配置文件。
    pub file: LoggingFile,
}

/// 日志配置文件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoggingFile {
    /// 文件名，例如 `client-1.12.xml`。
    pub id: String,
    /// SHA1。
    pub sha1: String,
    /// 大小。
    pub size: u64,
    /// 地址。
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(json: &str) -> VersionJson {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.15.0").as_deref(),
            Some("net/fabricmc/fabric-loader/0.15.0/fabric-loader-0.15.0.jar")
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.1:natives-windows").as_deref(),
            Some("org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar")
        );
        assert_eq!(
            maven_path("de.oceanlabs.mcp:mcp_config:1.20.1@zip").as_deref(),
            Some("de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip")
        );
        for bad in [
            "a:b",
            "a:b:c:d:e",
            "a::c",
            "a:..:c",
            "a:b/c:d",
            "a:b:c@x\\y",
        ] {
            assert_eq!(maven_path(bad), None, "{bad}");
        }
    }

    #[test]
    fn library_keys() {
        assert_eq!(library_key("org.ow2.asm:asm:9.5"), "org.ow2.asm:asm");
        assert_eq!(
            library_key("org.lwjgl:lwjgl:3.3.1:natives-linux"),
            "org.lwjgl:lwjgl:natives-linux"
        );
        assert_eq!(library_key("a:b:1@zip"), "a:b");
    }

    #[test]
    fn library_file_from_coordinates() {
        let lib: Library = serde_json::from_str(
            r#"{"name":"net.fabricmc:intermediary:1.20.1","url":"https://maven.fabricmc.net/"}"#,
        )
        .unwrap();
        let file = lib.file().unwrap();
        assert_eq!(
            file.url.as_deref(),
            Some(
                "https://maven.fabricmc.net/net/fabricmc/intermediary/1.20.1/intermediary-1.20.1.jar"
            )
        );
        assert_eq!(file.sha1, None);

        let lib: Library = serde_json::from_str(r#"{"name":"lzma:lzma:0.0.1"}"#).unwrap();
        assert!(lib.file().unwrap().url.unwrap().starts_with(LIBRARIES_URL));

        // 只有 natives 的旧式库没有主构件。
        let lib: Library = serde_json::from_str(
            r#"{"name":"a:b-platform:1","natives":{"linux":"natives-linux"}}"#,
        )
        .unwrap();
        assert_eq!(lib.file(), None);
    }

    #[test]
    fn resolve_detects_cycles_and_missing_parents() {
        let a = version(r#"{"id":"a","mainClass":"M","inheritsFrom":"b"}"#);
        let b = version(r#"{"id":"b","inheritsFrom":"a"}"#);
        let load = |id: &str| match id {
            "a" => Ok(a.clone()),
            "b" => Ok(b.clone()),
            other => {
                Err(AppError::new(ErrorCode::GameVersionNotFound).with_param("version", other))
            }
        };
        assert_eq!(
            resolve("a", load).unwrap_err().code,
            ErrorCode::GameVersionJsonInvalid
        );

        let c = version(r#"{"id":"c","mainClass":"M","inheritsFrom":"missing"}"#);
        let err = resolve("c", |id| {
            if id == "c" {
                Ok(c.clone())
            } else {
                Err(AppError::new(ErrorCode::GameVersionNotFound).with_param("version", id))
            }
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::GameVersionNotFound);
        assert_eq!(err.params["version"], "missing");

        let d = version(r#"{"id":"d"}"#);
        let err = resolve("d", |_| Ok(d.clone())).unwrap_err();
        assert_eq!(err.code, ErrorCode::GameVersionJsonInvalid);
    }
}
