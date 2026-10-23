//! 下载源测速：对每个候选源下载同一组文件，统计成功率、吞吐、首字节延迟与
//! SHA1 一致率，输出 Markdown 表格（M1-02，结果见 `docs/research/mirrors.md`）。
//!
//! ```text
//! cargo run -p tmcl-net --example bench_sources --release -- [轮数] [标签]
//! ```
//!
//! 文件组：版本清单、1.20.1 版本 JSON、client.jar、20 个依赖库、100 个资源文件、
//! Forge 安装器与 Fabric Loader。期望的 SHA1 取自官方元数据（maven 文件为官方
//! `.sha1` 的值）。代理由系统环境变量决定，可分别在直连与代理下各跑一次。
//! 文件只在内存中计算 SHA1，不写盘。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Url;
use serde_json::Value;
use sha1::{Digest, Sha1};
use tmcl_common::TaskCtx;
use tmcl_net::{DownloadSource, HttpClient, Official, PrefixMirror};
use tokio::sync::Semaphore;

type BoxError = Box<dyn std::error::Error>;

const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const VERSION: &str = "1.20.1";
const ASSET_COUNT: usize = 100;
const LIBRARY_COUNT: usize = 20;
const CONCURRENCY: usize = 8;
/// 单个请求等待响应头的总时限。
const HEADER_TIMEOUT: Duration = Duration::from_secs(60);
/// 单个响应体读取的总时限。
const BODY_TIMEOUT: Duration = Duration::from_secs(300);
/// Forge 安装器与 Fabric Loader，以及官方 maven `.sha1` 文件中的校验和（写死，
/// 避免国内直连时取不到官方 `.sha1` 而无法开测）。
const MAVEN_FILES: [(&str, &str); 2] = [
    (
        "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar",
        "ded43dd18b3a1dd5098b114c28432224d72bd9f7",
    ),
    (
        "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar",
        "9c2be53d84ec5c4fb58bb5b8d9c8a629047c9ee9",
    ),
];

/// 一个待测文件。
struct Target {
    group: &'static str,
    url: String,
    sha1: String,
}

/// 一次下载的测量结果。
#[derive(Default)]
struct Sample {
    ok: bool,
    sha1_ok: bool,
    ttfb: Duration,
    bytes: u64,
    final_host: String,
    /// 失败原因（错误码或 HTTP 状态码），成功时为空。
    error: String,
}

impl Sample {
    fn failed(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            ..Self::default()
        }
    }
}

/// 某个源在某组文件上的汇总。
#[derive(Default)]
struct Stats {
    attempted: usize,
    ok: usize,
    sha1_ok: usize,
    bytes: u64,
    wall: Duration,
    ttfb: Vec<Duration>,
    hosts: BTreeMap<String, usize>,
    failures: BTreeMap<String, usize>,
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let mut args = std::env::args().skip(1);
    let rounds: usize = args.next().map(|s| s.parse()).transpose()?.unwrap_or(3);
    let label = args.next().unwrap_or_else(|| "unlabeled".to_owned());
    let client = HttpClient::new()?;
    let ctx = TaskCtx::noop("bench");

    let targets = collect_targets(&client, &ctx).await?;
    let sources: Vec<Arc<dyn DownloadSource>> = vec![
        Arc::new(Official),
        Arc::new(PrefixMirror::bmclapi()),
        Arc::new(ustc()),
    ];

    println!("## {label}\n");
    println!(
        "轮数 {rounds}，文件 {} 个，资源文件并发 {CONCURRENCY}。\n",
        targets.len()
    );
    println!(
        "| 源 | 文件组 | 成功率 | SHA1 一致 | 吞吐 (MiB/s) | 首字节中位数 (ms) | 实际服务主机 | 失败原因 |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for source in &sources {
        let mut all: BTreeMap<&str, Stats> = BTreeMap::new();
        for _ in 0..rounds {
            for (group, stats) in run_round(&client, source.clone(), &targets).await {
                merge(all.entry(group).or_default(), stats);
            }
        }
        for (group, stats) in &all {
            print_row(source.name(), group, stats);
        }
    }
    Ok(())
}

/// USTC 镜像站上的 BMCLAPI maven 缓存，只覆盖部分 maven 仓库。
fn ustc() -> PrefixMirror {
    const ROOT: &str = "https://mirrors.ustc.edu.cn/bmclapi";
    PrefixMirror::new(
        "ustc",
        [
            ("https://libraries.minecraft.net", ROOT),
            ("https://maven.minecraftforge.net", ROOT),
            ("https://maven.fabricmc.net", ROOT),
        ],
    )
}

async fn collect_targets(client: &HttpClient, ctx: &TaskCtx) -> Result<Vec<Target>, BoxError> {
    let manifest_bytes = client.get_bytes(MANIFEST, ctx).await?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)?;
    let mut targets = vec![Target {
        group: "1 版本清单",
        url: MANIFEST.to_owned(),
        sha1: hex(&Sha1::digest(&manifest_bytes)),
    }];

    let entry = manifest["versions"]
        .as_array()
        .and_then(|v| v.iter().find(|v| v["id"] == VERSION))
        .ok_or("版本清单中没有 1.20.1")?;
    let version_url = str_of(&entry["url"])?;
    targets.push(Target {
        group: "2 版本 JSON",
        url: version_url.to_owned(),
        sha1: str_of(&entry["sha1"])?.to_owned(),
    });

    let version: Value = client.get_json(version_url, ctx).await?;
    let client_jar = &version["downloads"]["client"];
    targets.push(Target {
        group: "3 client.jar",
        url: str_of(&client_jar["url"])?.to_owned(),
        sha1: str_of(&client_jar["sha1"])?.to_owned(),
    });

    let libraries = version["libraries"].as_array().ok_or("缺少 libraries")?;
    for artifact in libraries
        .iter()
        .filter_map(|l| l["downloads"]["artifact"].as_object())
        .take(LIBRARY_COUNT)
    {
        targets.push(Target {
            group: "4 依赖库",
            url: str_of(&artifact["url"])?.to_owned(),
            sha1: str_of(&artifact["sha1"])?.to_owned(),
        });
    }

    let index: Value = client
        .get_json(str_of(&version["assetIndex"]["url"])?, ctx)
        .await?;
    let objects = index["objects"].as_object().ok_or("缺少 objects")?;
    let mut hashes: Vec<&str> = objects
        .values()
        .filter_map(|o| o["hash"].as_str())
        .collect();
    hashes.sort_unstable();
    hashes.dedup();
    for hash in hashes.into_iter().take(ASSET_COUNT) {
        targets.push(Target {
            group: "5 资源文件",
            url: format!(
                "https://resources.download.minecraft.net/{}/{hash}",
                &hash[..2]
            ),
            sha1: hash.to_owned(),
        });
    }

    for (url, sha1) in MAVEN_FILES {
        targets.push(Target {
            group: "6 Forge/Fabric",
            url: url.to_owned(),
            sha1: sha1.to_owned(),
        });
    }
    Ok(targets)
}

/// 用 `source` 下载一遍所有它提供的文件，按文件组汇总。
async fn run_round(
    client: &HttpClient,
    source: Arc<dyn DownloadSource>,
    targets: &[Target],
) -> BTreeMap<&'static str, Stats> {
    let mut groups: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for target in targets {
        let Some(url) = Url::parse(&target.url)
            .ok()
            .and_then(|u| source.rewrite(&u))
        else {
            continue;
        };
        groups
            .entry(target.group)
            .or_default()
            .push(format!("{url} {}", target.sha1));
    }

    let mut result = BTreeMap::new();
    for (group, items) in groups {
        let limit = Arc::new(Semaphore::new(CONCURRENCY));
        let started = Instant::now();
        let mut set = tokio::task::JoinSet::new();
        for item in items {
            let client = client.clone();
            let limit = limit.clone();
            set.spawn(async move {
                let _permit = limit.acquire_owned().await.ok()?;
                let (url, sha1) = item.split_once(' ')?;
                Some(measure(&client, url, sha1).await)
            });
        }
        let mut stats = Stats::default();
        while let Some(joined) = set.join_next().await {
            let sample = joined
                .ok()
                .flatten()
                .unwrap_or_else(|| Sample::failed("内部错误"));
            stats.attempted += 1;
            if sample.ok {
                stats.ok += 1;
                stats.bytes += sample.bytes;
                stats.ttfb.push(sample.ttfb);
                *stats.hosts.entry(sample.final_host).or_default() += 1;
            } else {
                *stats.failures.entry(sample.error).or_default() += 1;
            }
            if sample.sha1_ok {
                stats.sha1_ok += 1;
            }
        }
        stats.wall = started.elapsed();
        eprintln!(
            "{} {group}: {}/{} 成功，用时 {:.1} s",
            source.name(),
            stats.ok,
            stats.attempted,
            stats.wall.as_secs_f64()
        );
        result.insert(group, stats);
    }
    result
}

async fn measure(client: &HttpClient, url: &str, expected: &str) -> Sample {
    let ctx = TaskCtx::noop("bench");
    let started = Instant::now();
    // 客户端只有连接与单次读取超时，TLS 握手等阶段可能无限挂起，这里加总时限。
    let Ok(response) = tokio::time::timeout(HEADER_TIMEOUT, client.get(url, &ctx)).await else {
        return Sample::failed("响应超时");
    };
    let response = match response {
        Ok(response) => response,
        Err(e) => {
            let reason = match e.params.get("status") {
                Some(status) => format!("HTTP {status}"),
                None => e.code.as_str().to_owned(),
            };
            return Sample::failed(reason);
        }
    };
    let ttfb = started.elapsed();
    let final_host = response.url().host_str().unwrap_or_default().to_owned();
    let Ok(body) = tokio::time::timeout(BODY_TIMEOUT, response.bytes()).await else {
        return Sample::failed("读取超时");
    };
    let Ok(body) = body else {
        return Sample::failed("读取中断");
    };
    Sample {
        ok: true,
        sha1_ok: hex(&Sha1::digest(&body)) == expected,
        ttfb,
        bytes: body.len() as u64,
        final_host,
        error: String::new(),
    }
}

fn merge(into: &mut Stats, from: Stats) {
    into.attempted += from.attempted;
    into.ok += from.ok;
    into.sha1_ok += from.sha1_ok;
    into.bytes += from.bytes;
    into.wall += from.wall;
    into.ttfb.extend(from.ttfb);
    for (host, n) in from.hosts {
        *into.hosts.entry(host).or_default() += n;
    }
    for (error, n) in from.failures {
        *into.failures.entry(error).or_default() += n;
    }
}

fn print_row(source: &str, group: &str, stats: &Stats) {
    let mut ttfb = stats.ttfb.clone();
    ttfb.sort_unstable();
    let median = ttfb
        .get(ttfb.len() / 2)
        .map_or("-".to_owned(), |d| d.as_millis().to_string());
    let speed = if stats.wall.is_zero() || stats.ok == 0 {
        "-".to_owned()
    } else {
        format!(
            "{:.2}",
            stats.bytes as f64 / 1_048_576.0 / stats.wall.as_secs_f64()
        )
    };
    // 按次数取前三个实际服务的主机，其余只计数。
    let mut hosts: Vec<(&String, &usize)> = stats.hosts.iter().collect();
    hosts.sort_by(|a, b| b.1.cmp(a.1));
    let mut shown: Vec<String> = hosts
        .iter()
        .take(3)
        .map(|(host, n)| format!("{host}×{n}"))
        .collect();
    if hosts.len() > 3 {
        shown.push(format!("另 {} 个", hosts.len() - 3));
    }
    let failures: Vec<String> = stats
        .failures
        .iter()
        .map(|(error, n)| format!("{error}×{n}"))
        .collect();
    let failures = if failures.is_empty() {
        "-".to_owned()
    } else {
        failures.join(", ")
    };
    println!(
        "| {source} | {} | {}/{} | {}/{} | {speed} | {median} | {} | {failures} |",
        &group[2..],
        stats.ok,
        stats.attempted,
        stats.sha1_ok,
        stats.attempted,
        shown.join(", "),
    );
}

fn str_of(value: &Value) -> Result<&str, BoxError> {
    value.as_str().ok_or_else(|| "元数据字段缺失".into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
