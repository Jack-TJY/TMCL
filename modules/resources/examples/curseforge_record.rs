//! 录制 CurseForge 真实响应，用于生成 `tests/fixtures/curseforge/`（需要带 key 构建）。
//!
//! 用法：`cargo run -p tmcl-resources --example curseforge_record -- <输出目录>`。
//! 输出的是原始响应，提交前要截短长列表并检查不含账号信息。key 只在请求头中，不会写入输出。

use std::path::PathBuf;

use tmcl_common::TaskCtx;
use tmcl_net::HttpClient;
use tmcl_resources::curseforge::{
    ClassId, CurseForgeClient, FileFilter, ModLoaderType, SearchQuery, fingerprint, with_cdn_key,
};

/// Just Enough Items。
const JEI: u32 = 238_222;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(std::env::args().nth(1).ok_or("缺少输出目录")?);
    std::fs::create_dir_all(&out)?;
    let http = HttpClient::new()?;
    let Some(client) = CurseForgeClient::from_build(&http) else {
        eprintln!("本次构建没有 CurseForge API Key");
        std::process::exit(2);
    };
    let client = client?;
    let ctx = TaskCtx::noop("record");
    let save = |name: &str, value: &serde_json::Value| -> std::io::Result<()> {
        std::fs::write(
            out.join(name),
            serde_json::to_vec_pretty(value).unwrap_or_default(),
        )
    };

    // 原始 JSON 直接用 reqwest 取，方便截短后作为 fixture；同时用客户端解析一遍验证模型。
    let search = SearchQuery {
        class_id: Some(ClassId::Mods),
        game_version: Some("1.20.1".into()),
        loader: Some(ModLoaderType::Forge),
        limit: 2,
        ..SearchQuery::new("jei")
    };
    let page = client.search(&search, &ctx).await?;
    println!(
        "search: {} hits, first {}",
        page.pagination.total_count, page.data[0].name
    );
    let project = client.project(JEI, &ctx).await?.ok_or("JEI 不存在")?;
    println!(
        "project: {} allow={:?}",
        project.name, project.allow_mod_distribution
    );
    let files = client
        .project_files(
            JEI,
            &FileFilter {
                limit: 2,
                ..FileFilter::for_instance("1.20.1", "forge")
            },
            &ctx,
        )
        .await?;
    let file = &files.data[0];
    println!(
        "file: {} {:?} fp={}",
        file.file_name, file.download_url, file.file_fingerprint
    );

    // 下载文件并核对 SHA1 与指纹。
    let cdn = with_cdn_key(&http)?;
    let url = file.download_url.clone().ok_or("没有下载地址")?;
    let bytes = cdn.get_bytes(&url, &ctx).await?;
    println!(
        "downloaded {} bytes, fingerprint ours={} api={}",
        bytes.len(),
        fingerprint::of_bytes(&bytes),
        file.file_fingerprint
    );
    let matches = client
        .match_fingerprints(&[file.file_fingerprint], &ctx)
        .await?;
    println!("fingerprint matches: {}", matches.exact_matches.len());

    let packs = client
        .search(
            &SearchQuery {
                class_id: Some(ClassId::Modpacks),
                limit: 2,
                ..SearchQuery::new("")
            },
            &ctx,
        )
        .await?;
    println!(
        "modpacks: {}",
        packs
            .data
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    for (name, value) in raw(&client, &ctx, file.id, file.file_fingerprint).await? {
        save(name, &value)?;
    }
    Ok(())
}

async fn raw(
    client: &CurseForgeClient,
    ctx: &TaskCtx,
    file_id: u32,
    fingerprint: u32,
) -> Result<Vec<(&'static str, serde_json::Value)>, Box<dyn std::error::Error>> {
    let s = |v: &str| v.to_owned();
    let jei = JEI.to_string();
    let file = file_id.to_string();
    Ok(vec![
        (
            "search.json",
            client
                .get_value(
                    &["mods", "search"],
                    &[
                        ("gameId", s("432")),
                        ("classId", s("6")),
                        ("searchFilter", s("jei")),
                        ("gameVersion", s("1.20.1")),
                        ("modLoaderType", s("1")),
                        ("sortField", s("2")),
                        ("sortOrder", s("desc")),
                        ("pageSize", s("2")),
                    ],
                    ctx,
                )
                .await?,
        ),
        (
            "mod.json",
            client.get_value(&["mods", &jei], &[], ctx).await?,
        ),
        (
            "mod_files.json",
            client
                .get_value(
                    &["mods", &jei, "files"],
                    &[
                        ("gameVersion", s("1.20.1")),
                        ("modLoaderType", s("1")),
                        ("pageSize", s("2")),
                    ],
                    ctx,
                )
                .await?,
        ),
        (
            "file.json",
            client
                .get_value(&["mods", &jei, "files", &file], &[], ctx)
                .await?,
        ),
        (
            "fingerprints.json",
            client
                .post_value(
                    &["fingerprints", "432"],
                    &serde_json::json!({ "fingerprints": [fingerprint, 1] }),
                    ctx,
                )
                .await?,
        ),
        (
            "modpack_search.json",
            client
                .get_value(
                    &["mods", "search"],
                    &[
                        ("gameId", s("432")),
                        ("classId", s("4471")),
                        ("sortField", s("2")),
                        ("sortOrder", s("desc")),
                        ("pageSize", s("2")),
                    ],
                    ctx,
                )
                .await?,
        ),
    ])
}
