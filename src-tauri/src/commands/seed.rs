//! Seed-map IPC: keep CPU work off the command thread. Query data goes only to
//! the initiating webview's channel; normal task events carry lifecycle state.
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State, ipc::Channel};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_seed::{ChunkBounds, Dimension, GameVersion, StructureKind, Tile};
use tokio::sync::Semaphore;
use ts_rs::TS;

use crate::{
    events::{TaskStarted, spawn_task},
    state::AppState,
};

const MAX_TILES: usize = 16;
static WORKERS: Semaphore = Semaphore::const_new(2);

#[derive(Debug, Deserialize, TS)]
pub struct SeedMapArgs {
    version: String,
    seed: String,
    #[serde(default)]
    #[ts(optional)]
    dimension: Option<SeedDimension>,
    query: SeedMapQuery,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SeedDimension {
    #[default]
    Overworld,
    TheNether,
    TheEnd,
}

impl From<SeedDimension> for Dimension {
    fn from(dimension: SeedDimension) -> Self {
        match dimension {
            SeedDimension::Overworld => Self::Overworld,
            SeedDimension::TheNether => Self::Nether,
            SeedDimension::TheEnd => Self::End,
        }
    }
}

impl SeedMapArgs {
    fn inputs(&self) -> AppResult<(GameVersion, i64)> {
        let invalid = |field| AppError::new(ErrorCode::SeedInvalidInput).with_param("field", field);
        if self.seed.len() > 1024 {
            return Err(invalid("seed"));
        }
        if let SeedMapQuery::Tiles { tiles } = &self.query {
            if !(1..=MAX_TILES).contains(&tiles.len()) {
                return Err(invalid("tiles"));
            }
            if self.dimension.unwrap_or_default() != SeedDimension::Overworld {
                return Err(invalid("dimension"));
            }
        }
        Ok((self.version.parse()?, tmcl_seed::parse_seed(&self.seed)?))
    }
}

#[derive(Debug, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SeedMapQuery {
    Tiles {
        tiles: Vec<SeedTile>,
    },
    Structures {
        kind: SeedStructureKind,
        bounds: SeedChunkBounds,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
pub struct SeedTile {
    x: i32,
    z: i32,
    zoom: u8,
}
#[derive(Debug, Deserialize, TS)]
pub struct SeedChunkBounds {
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
}
#[derive(Debug, Serialize, TS)]
pub struct SeedStructureStart {
    chunk_x: i32,
    chunk_z: i32,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SeedStructureKind {
    Village,
    AbandonedCamp,
    Stronghold,
    DesertPyramid,
    JunglePyramid,
    OceanMonument,
    Mansion,
    Igloo,
    SwampHut,
    PillagerOutpost,
    Shipwreck,
    AncientCity,
    TrailRuins,
    TrialChambers,
    BuriedTreasure,
    Fortress,
    Bastion,
    Mineshaft,
    OceanRuin,
    RuinedPortal,
    RuinedPortalNether,
}
impl From<SeedStructureKind> for StructureKind {
    fn from(kind: SeedStructureKind) -> Self {
        match kind {
            SeedStructureKind::Village => Self::Village,
            SeedStructureKind::AbandonedCamp => Self::AbandonedCamp,
            SeedStructureKind::Stronghold => Self::Stronghold,
            SeedStructureKind::DesertPyramid => Self::DesertPyramid,
            SeedStructureKind::JunglePyramid => Self::JunglePyramid,
            SeedStructureKind::OceanMonument => Self::OceanMonument,
            SeedStructureKind::Mansion => Self::Mansion,
            SeedStructureKind::Igloo => Self::Igloo,
            SeedStructureKind::SwampHut => Self::SwampHut,
            SeedStructureKind::PillagerOutpost => Self::PillagerOutpost,
            SeedStructureKind::Shipwreck => Self::Shipwreck,
            SeedStructureKind::AncientCity => Self::AncientCity,
            SeedStructureKind::TrailRuins => Self::TrailRuins,
            SeedStructureKind::TrialChambers => Self::TrialChambers,
            SeedStructureKind::BuriedTreasure => Self::BuriedTreasure,
            SeedStructureKind::Fortress => Self::Fortress,
            SeedStructureKind::Bastion => Self::Bastion,
            SeedStructureKind::Mineshaft => Self::Mineshaft,
            SeedStructureKind::OceanRuin => Self::OceanRuin,
            SeedStructureKind::RuinedPortal => Self::RuinedPortal,
            SeedStructureKind::RuinedPortalNether => Self::RuinedPortalNether,
        }
    }
}

#[derive(Debug, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SeedMapResult {
    Tile {
        tile: SeedTile,
        png: Vec<u8>,
    },
    Structures {
        kind: SeedStructureKind,
        starts: Vec<SeedStructureStart>,
    },
}

impl SeedMapQuery {
    fn run(
        self,
        cache: &Path,
        version: GameVersion,
        dimension: Dimension,
        seed: i64,
        ctx: &TaskCtx,
        mut send: impl FnMut(SeedMapResult) -> AppResult<()>,
    ) -> AppResult<()> {
        match self {
            Self::Tiles { tiles } => {
                for tile in tiles {
                    let png = tmcl_seed::cached_tile_png(
                        cache,
                        version,
                        seed,
                        Tile {
                            x: tile.x,
                            z: tile.z,
                            zoom: tile.zoom,
                        },
                        ctx,
                    )?;
                    ctx.check_cancelled()?;
                    send(SeedMapResult::Tile { tile, png })?;
                }
            }
            Self::Structures { kind, bounds } => {
                let starts = tmcl_seed::structure_starts(
                    version,
                    dimension,
                    seed,
                    kind.into(),
                    ChunkBounds {
                        min_x: bounds.min_x,
                        min_z: bounds.min_z,
                        max_x: bounds.max_x,
                        max_z: bounds.max_z,
                    },
                    ctx,
                )?
                .into_iter()
                .map(|s| SeedStructureStart {
                    chunk_x: s.chunk_x,
                    chunk_z: s.chunk_z,
                })
                .collect();
                ctx.check_cancelled()?;
                send(SeedMapResult::Structures { kind, starts })?;
            }
        }
        ctx.check_cancelled()
    }
}

/// Start a cancellable map query. The frontend supplies no filesystem paths.
#[tauri::command]
pub async fn seed_map_query<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    args: SeedMapArgs,
    results: Channel<SeedMapResult>,
) -> AppResult<TaskStarted> {
    let (version, seed) = args.inputs()?;
    let cache = state.data()?.cache_dir().join("seed-map");
    Ok(spawn_task(&app, "seed_map", move |ctx| async move {
        let permit = tokio::select! {
            biased;
            _=ctx.cancel.cancelled()=>return Err(AppError::cancelled()),
            p=WORKERS.acquire()=>p.map_err(|_| AppError::new(ErrorCode::CommonInternal))?,
        };
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            args.query.run(
                &cache,
                version,
                args.dimension.unwrap_or_default().into(),
                seed,
                &ctx,
                |msg| results.send(msg).map_err(|_| AppError::cancelled()),
            )
        })
        .await
        .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))?
    }))
}

#[cfg(test)]
mod tests;
