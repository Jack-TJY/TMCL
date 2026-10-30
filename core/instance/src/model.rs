//! 实例模型 `instance.json`（PLAN.md 第 3.5 节）与 `schema` 迁移。
//!
//! - 读取：先按 JSON 解析，检查 `schema`，按顺序执行 [`migrate`] 中的迁移，
//!   再反序列化为 [`InstanceConfig`]。缺失的可选字段取默认值，未知字段被忽略。
//! - 结构变化时提升 [`INSTANCE_SCHEMA`]，在 [`migrate`] 中追加 `n → n+1` 的
//!   迁移步骤并补测试（AGENTS.md §6.2 第 5 条）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tmcl_common::config::MemoryConfig;
use tmcl_common::{AppError, AppResult, ErrorCode};
use ts_rs::TS;

/// 当前程序写出的 `instance.json` 结构版本。
pub const INSTANCE_SCHEMA: u32 = 2;

/// 实例名称的最大长度（按字符计）。
pub const MAX_NAME_CHARS: usize = 64;

/// 模组加载器种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LoaderKind {
    /// Fabric。
    Fabric,
    /// Quilt。
    Quilt,
    /// Forge。
    Forge,
    /// NeoForge。
    Neoforge,
}

impl LoaderKind {
    /// 序列化后的字符串，例如 `fabric`。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::Neoforge => "neoforge",
        }
    }

    /// 由 [`as_str`](Self::as_str) 的结果解析，未知值返回 `None`。
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Fabric, Self::Quilt, Self::Forge, Self::Neoforge]
            .into_iter()
            .find(|k| k.as_str() == s)
    }
}

/// 模组加载器及其版本；实例的 `loader` 为 `null` 表示原版。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LoaderSpec {
    /// 加载器种类。
    pub kind: LoaderKind,
    /// 加载器版本，例如 `0.16.5`。
    pub version: String,
}

/// Java 选择方式。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum JavaSetting {
    /// 按版本 JSON 自动选择。
    #[default]
    Auto,
    /// 使用指定的 Java 可执行文件。
    Path {
        /// `java` / `javaw` 的路径。
        path: String,
    },
}

/// 游戏内存（MB）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct MemorySetting {
    /// 使用 `config.json` 中的全局内存；为 `false` 时使用下面的自定义值。
    pub follow_global: bool,
    /// 自定义的 `-Xms`。
    pub min_mb: u32,
    /// 自定义的 `-Xmx`。
    pub max_mb: u32,
}

impl Default for MemorySetting {
    fn default() -> Self {
        Self {
            follow_global: true,
            min_mb: 1024,
            max_mb: 4096,
        }
    }
}

impl MemorySetting {
    /// 启动时实际使用的 `(-Xms, -Xmx)`：跟随全局时取 `global`，否则取自定义值。
    pub fn effective(&self, global: &MemoryConfig) -> (u32, u32) {
        if self.follow_global {
            (global.min_mb, global.max_mb)
        } else {
            (self.min_mb, self.max_mb)
        }
    }
}

/// 游戏窗口。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct WindowSetting {
    /// 宽度（像素）。
    pub width: u32,
    /// 高度（像素）。
    pub height: u32,
    /// 是否全屏。
    pub fullscreen: bool,
}

impl Default for WindowSetting {
    fn default() -> Self {
        Self {
            width: 854,
            height: 480,
            fullscreen: false,
        }
    }
}

/// `instances/<id>/instance.json` 的内容。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct InstanceConfig {
    /// 结构版本，见 [`INSTANCE_SCHEMA`]。
    pub schema: u32,
    /// 实例 ID（UUID v4），与目录名一致。
    pub id: String,
    /// 显示名称。
    pub name: String,
    /// 游戏版本 ID，例如 `1.20.1`。
    pub game_version: String,
    /// 模组加载器，`null` 表示原版。
    #[serde(default)]
    pub loader: Option<LoaderSpec>,
    /// Java 选择方式。
    #[serde(default)]
    pub java: JavaSetting,
    /// 内存设置。
    #[serde(default)]
    pub memory: MemorySetting,
    /// 追加的 JVM 参数。
    #[serde(default)]
    pub jvm_args: Vec<String>,
    /// 追加的游戏参数。
    #[serde(default)]
    pub game_args: Vec<String>,
    /// 窗口设置。
    #[serde(default)]
    pub window: WindowSetting,
    /// 分组名，`null` 表示未分组。
    #[serde(default)]
    pub group: Option<String>,
    /// 创建时间（RFC 3339，UTC）。
    pub created_at: String,
    /// 最近一次启动时间（RFC 3339，UTC），从未启动为 `null`。
    #[serde(default)]
    pub last_played_at: Option<String>,
}

impl InstanceConfig {
    /// 以默认设置新建一个实例配置（新 ID，创建时间为当前时间）。
    pub fn new(name: &str, game_version: &str, loader: Option<LoaderSpec>) -> AppResult<Self> {
        Ok(Self {
            schema: INSTANCE_SCHEMA,
            id: uuid::Uuid::new_v4().to_string(),
            name: normalize_name(name)?,
            game_version: game_version.to_owned(),
            loader,
            java: JavaSetting::default(),
            memory: MemorySetting::default(),
            jvm_args: Vec::new(),
            game_args: Vec::new(),
            window: WindowSetting::default(),
            group: None,
            created_at: now_rfc3339(),
            last_played_at: None,
        })
    }

    /// 解析 `instance.json` 的文本，必要时执行 `schema` 迁移。
    ///
    /// JSON 损坏或缺少必需字段返回 `INSTANCE_CORRUPTED`；
    /// `schema` 缺失、为 0 或高于 [`INSTANCE_SCHEMA`] 返回 `INSTANCE_SCHEMA_UNSUPPORTED`。
    pub fn from_json(text: &str) -> AppResult<Self> {
        let mut value: Value = serde_json::from_str(text).map_err(corrupted)?;
        let schema = value.get("schema").and_then(Value::as_u64).unwrap_or(0);
        if schema == 0 || schema > u64::from(INSTANCE_SCHEMA) {
            return Err(AppError::new(ErrorCode::InstanceSchemaUnsupported)
                .with_param("schema", schema)
                .with_param("supported", INSTANCE_SCHEMA));
        }
        migrate(&mut value, schema);
        serde_json::from_value(value).map_err(corrupted)
    }

    /// 序列化为带缩进的 JSON 文本，`schema` 总是当前版本。
    pub fn to_json(&self) -> AppResult<String> {
        let mut config = self.clone();
        config.schema = INSTANCE_SCHEMA;
        Ok(serde_json::to_string_pretty(&config)? + "\n")
    }
}

/// 把 `value` 从 `from` 版本逐步迁移到 [`INSTANCE_SCHEMA`]。
///
/// 新增步骤时按 `n → n+1` 顺序追加分支。
fn migrate(value: &mut Value, from: u64) {
    let current = u64::from(INSTANCE_SCHEMA);
    let mut version = from;
    while version < current {
        if version == 1 {
            migrate_1_to_2(value);
        }
        version += 1;
    }
    value["schema"] = Value::from(current);
}

/// 1 → 2：新增 `memory.follow_global`。内存仍是旧默认值（1024 / 4096）的实例改为跟随全局，
/// 改过内存的实例保留为自定义。
fn migrate_1_to_2(value: &mut Value) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    let memory = root
        .entry("memory")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(memory) = memory.as_object_mut() else {
        return;
    };
    let defaults = MemorySetting::default();
    let field = |key: &str, default: u32| {
        memory
            .get(key)
            .and_then(Value::as_u64)
            .unwrap_or(u64::from(default))
    };
    let untouched = field("min_mb", defaults.min_mb) == u64::from(defaults.min_mb)
        && field("max_mb", defaults.max_mb) == u64::from(defaults.max_mb);
    memory.insert("follow_global".to_owned(), Value::Bool(untouched));
}

/// 去掉首尾空白并校验名称：不能为空，不超过 [`MAX_NAME_CHARS`] 个字符，不含控制字符。
pub fn normalize_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    let valid = !trimmed.is_empty()
        && trimmed.chars().count() <= MAX_NAME_CHARS
        && !trimmed.chars().any(char::is_control);
    if valid {
        Ok(trimmed.to_owned())
    } else {
        Err(AppError::new(ErrorCode::InstanceInvalidName).with_param("max", MAX_NAME_CHARS))
    }
}

/// 当前 UTC 时间的 RFC 3339 表示，精确到秒。
pub fn now_rfc3339() -> String {
    let now = OffsetDateTime::now_utc();
    let now = now.replace_nanosecond(0).unwrap_or(now);
    now.format(&Rfc3339).unwrap_or_default()
}

fn corrupted(err: serde_json::Error) -> AppError {
    AppError::new(ErrorCode::InstanceCorrupted).with_detail(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "schema": 2,
        "id": "0b6f0000-0000-4000-8000-000000000000",
        "name": "生存 1.20.1",
        "game_version": "1.20.1",
        "loader": { "kind": "fabric", "version": "0.16.5" },
        "java": { "mode": "auto" },
        "memory": { "follow_global": true, "min_mb": 1024, "max_mb": 4096 },
        "jvm_args": [],
        "game_args": [],
        "window": { "width": 854, "height": 480, "fullscreen": false },
        "group": null,
        "created_at": "2026-10-22T00:00:00Z",
        "last_played_at": null
    }"#;

    #[test]
    fn parses_plan_sample() {
        let config = InstanceConfig::from_json(SAMPLE).unwrap();
        assert_eq!(config.name, "生存 1.20.1");
        assert_eq!(
            config.loader,
            Some(LoaderSpec {
                kind: LoaderKind::Fabric,
                version: "0.16.5".into()
            })
        );
        assert_eq!(config.java, JavaSetting::Auto);
        let round = InstanceConfig::from_json(&config.to_json().unwrap()).unwrap();
        assert_eq!(round, config);
    }

    #[test]
    fn java_path_mode_round_trips() {
        let mut config = InstanceConfig::new("a", "1.20.1", None).unwrap();
        config.java = JavaSetting::Path {
            path: "/usr/bin/java".into(),
        };
        let value: Value = serde_json::from_str(&config.to_json().unwrap()).unwrap();
        assert_eq!(
            value["java"],
            serde_json::json!({ "mode": "path", "path": "/usr/bin/java" })
        );
        assert_eq!(value["loader"], Value::Null);
    }

    #[test]
    fn missing_optional_fields_take_defaults_and_unknown_ignored() {
        let text = r#"{ "schema": 1, "id": "x", "name": "n", "game_version": "1.12.2",
                        "created_at": "2026-10-22T00:00:00Z", "future_field": 1 }"#;
        let config = InstanceConfig::from_json(text).unwrap();
        assert_eq!(config.memory, MemorySetting::default());
        assert_eq!(config.window, WindowSetting::default());
        assert!(config.loader.is_none());
    }

    #[test]
    fn schema_1_memory_migrates_to_follow_global() {
        let v1 = |memory: &str| {
            format!(
                r#"{{ "schema": 1, "id": "x", "name": "n", "game_version": "1.20.1",
                     "created_at": "2026-10-22T00:00:00Z"{memory} }}"#
            )
        };
        let untouched =
            InstanceConfig::from_json(&v1(r#", "memory": { "min_mb": 1024, "max_mb": 4096 }"#))
                .unwrap();
        assert_eq!(untouched.schema, 2);
        assert_eq!(untouched.memory, MemorySetting::default());
        assert!(
            InstanceConfig::from_json(&v1(""))
                .unwrap()
                .memory
                .follow_global
        );

        let custom = InstanceConfig::from_json(&v1(r#", "memory": { "max_mb": 8192 }"#)).unwrap();
        assert_eq!(
            custom.memory,
            MemorySetting {
                follow_global: false,
                min_mb: 1024,
                max_mb: 8192
            }
        );
    }

    #[test]
    fn effective_memory_follows_global_or_custom() {
        let global = MemoryConfig {
            min_mb: 2048,
            max_mb: 6144,
        };
        let mut memory = MemorySetting::default();
        assert_eq!(memory.effective(&global), (2048, 6144));
        memory.follow_global = false;
        assert_eq!(memory.effective(&global), (1024, 4096));
    }

    #[test]
    fn broken_json_is_corrupted() {
        for text in ["{ not json", r#"{ "schema": 1, "id": "x" }"#] {
            let err = InstanceConfig::from_json(text).unwrap_err();
            assert_eq!(err.code, ErrorCode::InstanceCorrupted);
        }
    }

    #[test]
    fn unsupported_schema_rejected() {
        for schema in ["0", "3", "null"] {
            let text = SAMPLE.replace("\"schema\": 2", &format!("\"schema\": {schema}"));
            let err = InstanceConfig::from_json(&text).unwrap_err();
            assert_eq!(err.code, ErrorCode::InstanceSchemaUnsupported);
        }
    }

    #[test]
    fn name_is_trimmed_and_validated() {
        assert_eq!(normalize_name("  生存  ").unwrap(), "生存");
        let too_long = "名".repeat(MAX_NAME_CHARS + 1);
        for bad in ["", "   ", "a\nb", too_long.as_str()] {
            let err = normalize_name(bad).unwrap_err();
            assert_eq!(err.code, ErrorCode::InstanceInvalidName);
        }
    }

    #[test]
    fn loader_kind_strings() {
        for kind in [
            LoaderKind::Fabric,
            LoaderKind::Quilt,
            LoaderKind::Forge,
            LoaderKind::Neoforge,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
            assert_eq!(LoaderKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(LoaderKind::parse("rift"), None);
    }

    #[test]
    fn now_is_rfc3339_utc() {
        let now = now_rfc3339();
        assert!(now.ends_with('Z'), "{now}");
        // 形如 2026-10-22T00:00:00Z
        assert_eq!(now.len(), 20, "{now}");
        assert_eq!(&now[10..11], "T");
    }
}
