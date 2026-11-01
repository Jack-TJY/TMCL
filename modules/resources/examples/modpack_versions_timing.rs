//! 测量整合包版本列表（`resources_modpack_versions` 背后的调用）在真实 API 上的耗时。
//!
//! 用法：`cargo run -p tmcl-resources --example modpack_versions_timing --release`。
//! 带 CurseForge Key 构建时同时测 CurseForge；只打印耗时与条数，不打印响应内容。

use std::time::Instant;

use tmcl_common::TaskCtx;
use tmcl_net::HttpClient;
use tmcl_resources::curseforge::CurseForgeClient;
use tmcl_resources::modpack::source;
use tmcl_resources::modrinth::ModrinthClient;

/// Modrinth 整合包：Fabulously Optimized、Cobblemon Official Modpack [Fabric]。
const MODRINTH: [&str; 2] = ["fabulously-optimized", "cobblemon-fabric"];
/// CurseForge 整合包：All the Mods 9、Better MC [FABRIC]。
const CURSEFORGE: [&str; 2] = ["715572", "452013"];
/// 每个整合包测几次。
const ROUNDS: usize = 3;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let http = HttpClient::new()?;
    let ctx = TaskCtx::noop("timing");
    let modrinth = ModrinthClient::new(&http)?;
    for id in MODRINTH {
        for round in 1..=ROUNDS {
            let start = Instant::now();
            let versions = source::versions(&modrinth, id, None, &ctx).await?;
            println!(
                "modrinth {id} #{round}: {} versions in {:.2?}",
                versions.len(),
                start.elapsed()
            );
        }
    }
    let Some(curseforge) = CurseForgeClient::from_build(&http) else {
        println!("本次构建没有 CurseForge API Key，跳过 CurseForge");
        return Ok(());
    };
    let curseforge = curseforge?;
    for id in CURSEFORGE {
        for round in 1..=ROUNDS {
            let start = Instant::now();
            let versions = source::versions_curseforge(&curseforge, id, None, &ctx).await?;
            println!(
                "curseforge {id} #{round}: {} files in {:.2?}",
                versions.len(),
                start.elapsed()
            );
        }
    }
    Ok(())
}
