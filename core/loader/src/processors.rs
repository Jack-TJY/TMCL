//! Forge / NeoForge 安装器的处理步骤（processors）：在本地 Java 上依次运行，生成补丁后的客户端等文件。
//!
//! 参数与输出中的占位符规则与官方安装器一致：
//!
//! - 整个参数为 `[group:artifact:version[:classifier][@ext]]` 时替换为该构件在 `libraries/` 下的路径；
//! - 其余参数中的 `{KEY}` 替换为变量（`data` 段与内置的 `SIDE`、`MINECRAFT_JAR` 等），`\` 转义下一个字符。
//!
//! 声明了 `outputs` 的步骤在全部输出已存在且 SHA1 一致时跳过，运行后逐个校验输出。

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};
use tmcl_game::maven_path;
use tmcl_net::is_valid;
use tokio::process::Command;

use crate::{LoaderKind, display_name};

/// 错误详情中保留的进程输出长度（字节，取末尾）。
const OUTPUT_TAIL: usize = 4096;

/// `install_profile.json` 中的一个处理步骤。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Processor {
    /// 处理程序 jar 的 maven 坐标。
    pub jar: String,
    /// 额外的 classpath（maven 坐标）。
    #[serde(default)]
    pub classpath: Vec<String>,
    /// 参数模板。
    #[serde(default)]
    pub args: Vec<String>,
    /// 输出文件 → 期望 SHA1，两边都是模板。
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
    /// 适用的一端（`client` / `server`），缺省为两端都运行。
    #[serde(default)]
    pub sides: Option<Vec<String>>,
}

impl Processor {
    /// 客户端安装是否需要运行该步骤。
    pub fn runs_on_client(&self) -> bool {
        self.sides
            .as_ref()
            .is_none_or(|sides| sides.iter().any(|s| s == "client"))
    }

    /// 用于错误参数的名称：处理程序的 artifact，例如 `binarypatcher`。
    pub fn name(&self) -> &str {
        self.jar.split(':').nth(1).unwrap_or(&self.jar)
    }
}

/// 占位符变量与 `libraries/` 目录。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Variables {
    libraries: PathBuf,
    values: BTreeMap<String, String>,
}

impl Variables {
    /// 以 `libraries` 为构件根目录创建，没有变量。
    pub fn new(libraries: impl Into<PathBuf>) -> Self {
        Self {
            libraries: libraries.into(),
            values: BTreeMap::new(),
        }
    }

    /// 设置变量 `key`。
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    /// 读取变量。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// maven 坐标对应的本地路径；坐标无效时返回说明。
    pub fn artifact(&self, coords: &str) -> Result<PathBuf, String> {
        let path = maven_path(coords).ok_or_else(|| format!("invalid artifact {coords}"))?;
        Ok(path
            .split('/')
            .fold(self.libraries.clone(), |acc, part| acc.join(part)))
    }

    /// 按占位符规则展开一个参数或输出模板；变量缺失或坐标无效时返回说明。
    pub fn resolve(&self, template: &str) -> Result<String, String> {
        if let Some(coords) = template.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            return Ok(self.artifact(coords)?.display().to_string());
        }
        let mut out = String::with_capacity(template.len());
        let mut chars = template.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => out.extend(chars.next()),
                '{' => {
                    let key: String = chars.by_ref().take_while(|&c| c != '}').collect();
                    let value = self
                        .get(&key)
                        .ok_or_else(|| format!("missing variable {{{key}}} in {template}"))?;
                    out.push_str(value);
                }
                c => out.push(c),
            }
        }
        Ok(out)
    }
}

/// jar 的 `META-INF/MANIFEST.MF` 中的 `Main-Class`（处理续行）。
pub fn main_class(manifest: &str) -> Option<String> {
    let mut joined: Vec<String> = Vec::new();
    for line in manifest.lines() {
        match (line.strip_prefix(' '), joined.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => joined.push(line.to_owned()),
        }
    }
    joined.iter().find_map(|line| {
        line.strip_prefix("Main-Class:")
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    })
}

fn read_main_class(jar: &Path) -> Result<String, String> {
    let file = std::fs::File::open(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut entry = archive
        .by_name("META-INF/MANIFEST.MF")
        .map_err(|e| e.to_string())?;
    let mut text = String::new();
    entry.read_to_string(&mut text).map_err(|e| e.to_string())?;
    main_class(&text).ok_or_else(|| format!("{} has no Main-Class", jar.display()))
}

/// 运行处理步骤所需的环境。
#[derive(Debug, Clone, Copy)]
pub struct ProcessorEnv<'a> {
    /// `java` 可执行文件。
    pub java: &'a Path,
    /// 工作目录（临时目录）。
    pub work_dir: &'a Path,
    /// 加载器种类，用于错误参数。
    pub kind: LoaderKind,
    /// 加载器版本，用于错误参数。
    pub version: &'a str,
}

impl ProcessorEnv<'_> {
    fn failed(&self, processor: &Processor, detail: impl AsRef<str>) -> AppError {
        AppError::new(ErrorCode::LoaderProcessorFailed)
            .with_param("kind", display_name(self.kind))
            .with_param("version", self.version)
            .with_param("processor", processor.name())
            .with_detail(detail)
    }
}

/// 依次运行适用于客户端的处理步骤，阶段为 `loader_processors`，可取消（取消时结束子进程）。
///
/// 进程退出码非 0、输出缺失或 SHA1 不符时返回 `LOADER_PROCESSOR_FAILED`。
pub async fn run_processors(
    processors: &[Processor],
    vars: &Variables,
    env: ProcessorEnv<'_>,
    ctx: &TaskCtx,
) -> AppResult<()> {
    let client: Vec<&Processor> = processors.iter().filter(|p| p.runs_on_client()).collect();
    let total = client.len() as u64;
    ctx.report(ProgressUpdate::new("loader_processors").items(0, total));
    for (index, processor) in client.into_iter().enumerate() {
        ctx.check_cancelled()?;
        run_one(processor, vars, env, ctx).await?;
        ctx.report(ProgressUpdate::new("loader_processors").items(index as u64 + 1, total));
    }
    Ok(())
}

async fn outputs_valid(outputs: &[(PathBuf, String)]) -> AppResult<bool> {
    for (path, sha1) in outputs {
        if !is_valid(path, None, Some(sha1)).await? {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn run_one(
    processor: &Processor,
    vars: &Variables,
    env: ProcessorEnv<'_>,
    ctx: &TaskCtx,
) -> AppResult<()> {
    let resolve = |t: &str| vars.resolve(t).map_err(|e| env.failed(processor, e));
    let outputs = processor
        .outputs
        .iter()
        .map(|(file, sha1)| Ok((PathBuf::from(resolve(file)?), resolve(sha1)?.to_lowercase())))
        .collect::<AppResult<Vec<_>>>()?;
    if !outputs.is_empty() && outputs_valid(&outputs).await? {
        tracing::debug!(processor = processor.name(), "输出已存在且校验通过，跳过");
        return Ok(());
    }

    let artifact = |c: &str| vars.artifact(c).map_err(|e| env.failed(processor, e));
    let jar = artifact(&processor.jar)?;
    let main = {
        let jar = jar.clone();
        tokio::task::spawn_blocking(move || read_main_class(&jar))
            .await
            .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?
            .map_err(|e| env.failed(processor, e))?
    };
    let mut classpath = vec![jar];
    for coords in &processor.classpath {
        classpath.push(artifact(coords)?);
    }
    let classpath =
        std::env::join_paths(&classpath).map_err(|e| env.failed(processor, e.to_string()))?;
    let args = processor
        .args
        .iter()
        .map(|a| resolve(a))
        .collect::<AppResult<Vec<_>>>()?;

    let mut cmd = Command::new(env.java);
    cmd.arg("-cp")
        .arg(classpath)
        .arg(&main)
        .args(&args)
        .current_dir(env.work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW：不弹出控制台窗口。
        cmd.creation_flags(0x0800_0000);
    }
    tracing::info!(processor = processor.name(), main = %main, "运行安装器处理步骤");
    let child = cmd.spawn().map_err(|e| {
        AppError::new(ErrorCode::LaunchSpawnFailed)
            .with_param("java", env.java.display())
            .with_detail(e.to_string())
    })?;
    let output = tokio::select! {
        output = child.wait_with_output() => output?,
        () = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
    };
    if !output.status.success() {
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        let start = text.len().saturating_sub(OUTPUT_TAIL);
        let start = (start..text.len())
            .find(|&i| text.is_char_boundary(i))
            .unwrap_or(text.len());
        return Err(env.failed(
            processor,
            format!("exit status {}: {}", output.status, &text[start..]),
        ));
    }
    for (path, sha1) in &outputs {
        if !is_valid(path, None, Some(sha1)).await? {
            return Err(env.failed(
                processor,
                format!(
                    "output {} is missing or does not match {sha1}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Variables {
        let mut vars = Variables::new("libs");
        vars.insert("SIDE", "client");
        vars.insert("MC_SLIM", "/x/slim.jar");
        vars
    }

    #[test]
    fn resolves_artifacts_variables_and_escapes() {
        let vars = vars();
        let lib = |p: &str| {
            p.split('/')
                .fold(PathBuf::from("libs"), |a, s| a.join(s))
                .display()
                .to_string()
        };
        assert_eq!(
            vars.resolve("[net.minecraft:client:1.20.1-20230612.114412:slim]")
                .unwrap(),
            lib(
                "net/minecraft/client/1.20.1-20230612.114412/client-1.20.1-20230612.114412-slim.jar"
            )
        );
        assert_eq!(
            vars.resolve("[de.oceanlabs.mcp:mcp_config:1.16.5-20210115.111550@zip]")
                .unwrap(),
            lib(
                "de/oceanlabs/mcp/mcp_config/1.16.5-20210115.111550/mcp_config-1.16.5-20210115.111550.zip"
            )
        );
        assert_eq!(vars.resolve("--side={SIDE}").unwrap(), "--side=client");
        assert_eq!(vars.resolve("{MC_SLIM}").unwrap(), "/x/slim.jar");
        assert_eq!(vars.resolve(r"\{SIDE\}").unwrap(), "{SIDE}");
        assert_eq!(vars.resolve("--task").unwrap(), "--task");
        assert!(vars.resolve("{MISSING}").unwrap_err().contains("MISSING"));
        assert!(vars.resolve("[not-a-coordinate]").is_err());
    }

    #[test]
    fn reads_main_class_with_continuation_lines() {
        let manifest = "Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.install\r\n ertools.ConsoleTool\r\nClass-Path: a.jar\r\n";
        assert_eq!(
            main_class(manifest).as_deref(),
            Some("net.minecraftforge.installertools.ConsoleTool")
        );
        assert_eq!(main_class("Manifest-Version: 1.0\n"), None);
    }

    #[test]
    fn side_filter_and_name() {
        let json = r#"[
            {"jar": "net.minecraftforge:jarsplitter:1.1.4", "sides": ["client"]},
            {"jar": "net.minecraftforge:installertools:1.4.1", "sides": ["server"]},
            {"jar": "net.minecraftforge:binarypatcher:1.1.1"}
        ]"#;
        let processors: Vec<Processor> = serde_json::from_str(json).unwrap();
        let client: Vec<_> = processors
            .iter()
            .filter(|p| p.runs_on_client())
            .map(Processor::name)
            .collect();
        assert_eq!(client, ["jarsplitter", "binarypatcher"]);
    }
}
