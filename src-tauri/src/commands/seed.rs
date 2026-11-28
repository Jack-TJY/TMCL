//! Seed-map IPC: keep CPU work off the command thread. Query data goes only to
//! the initiating webview's channel; normal task events carry lifecycle state.
use std::path::{Path, PathBuf};

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

/// One saved world of an instance, with the seed read from its level.dat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SeedSavedWorld {
    pub dir: String,
    pub name: String,
    pub seed: String,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
pub struct SeedSavesArgs {
    pub instance: String,
}

const MAX_SAVES: usize = 200;

/// Scan every world directory of an instance; unreadable worlds are skipped
/// instead of failing the whole list.
fn list_saves(game_dir: &Path) -> Vec<SeedSavedWorld> {
    let Ok(entries) = std::fs::read_dir(game_dir.join("saves")) else {
        return Vec::new();
    };
    let mut worlds: Vec<SeedSavedWorld> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let info = tmcl_seed::read_level_dat(&entry.path().join("level.dat")).ok()?;
            Some(SeedSavedWorld {
                dir: entry.file_name().to_string_lossy().into_owned(),
                name: info.name,
                seed: info.seed.to_string(),
                version: info.version,
            })
        })
        .collect();
    worlds.sort_by(|a, b| a.dir.cmp(&b.dir));
    worlds.truncate(MAX_SAVES);
    worlds
}

/// Structures offered for one dimension, in display order.
#[derive(Debug, Serialize, TS)]
pub struct SeedStructureCapability {
    pub dimension: SeedDimension,
    pub kinds: Vec<SeedStructureKind>,
}

#[derive(Debug, Serialize, TS)]
pub struct SeedVersionInfo {
    pub version: String,
    pub label: String,
    pub structures: Vec<SeedStructureCapability>,
}

const ALL_KINDS: [SeedStructureKind; 22] = [
    SeedStructureKind::Village,
    SeedStructureKind::Stronghold,
    SeedStructureKind::DesertPyramid,
    SeedStructureKind::JunglePyramid,
    SeedStructureKind::OceanMonument,
    SeedStructureKind::Mansion,
    SeedStructureKind::Igloo,
    SeedStructureKind::SwampHut,
    SeedStructureKind::PillagerOutpost,
    SeedStructureKind::Shipwreck,
    SeedStructureKind::Mineshaft,
    SeedStructureKind::OceanRuin,
    SeedStructureKind::RuinedPortal,
    SeedStructureKind::BuriedTreasure,
    SeedStructureKind::AncientCity,
    SeedStructureKind::TrailRuins,
    SeedStructureKind::TrialChambers,
    SeedStructureKind::AbandonedCamp,
    SeedStructureKind::Fortress,
    SeedStructureKind::Bastion,
    SeedStructureKind::RuinedPortalNether,
    SeedStructureKind::EndCity,
];

/// Version matrix and the structure kinds each version can predict. Generated
/// from the pinned engine, so the UI never keeps its own version table.
#[tauri::command]
pub fn seed_versions() -> Vec<SeedVersionInfo> {
    tmcl_seed::GameVersion::all()
        .iter()
        .map(|version| {
            let mut structures = Vec::with_capacity(3);
            for dimension in [
                SeedDimension::Overworld,
                SeedDimension::TheNether,
                SeedDimension::TheEnd,
            ] {
                let kinds: Vec<SeedStructureKind> = ALL_KINDS
                    .iter()
                    .copied()
                    .filter(|kind| {
                        tmcl_seed::dimension_of((*kind).into()) == dimension.into()
                            && tmcl_seed::supports(*version, (*kind).into())
                    })
                    .collect();
                if !kinds.is_empty() {
                    structures.push(SeedStructureCapability { dimension, kinds });
                }
            }
            SeedVersionInfo {
                version: version.as_str().to_owned(),
                label: version.label().to_owned(),
                structures,
            }
        })
        .collect()
}

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
        if let SeedMapQuery::Tiles { tiles } = &self.query
            && !(1..=MAX_TILES).contains(&tiles.len())
        {
            return Err(invalid("tiles"));
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
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
    EndCity,
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
            SeedStructureKind::EndCity => Self::EndCity,
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
                        dimension,
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

/// List worlds of one instance and their seeds. The frontend passes no paths;
/// the instance id is validated and resolved through the data layout.
#[tauri::command]
pub async fn seed_list_saves(
    state: State<'_, AppState>,
    args: SeedSavesArgs,
) -> AppResult<Vec<SeedSavedWorld>> {
    let paths = state.data()?.clone();
    if !tmcl_instance::store::is_valid_id(&args.instance)
        || !paths.instance_dir(&args.instance).is_dir()
    {
        return Err(AppError::new(ErrorCode::InstanceNotFound).with_param("id", args.instance));
    }
    let game: PathBuf = paths.instance_game_dir(&args.instance);
    tokio::task::spawn_blocking(move || list_saves(&game))
        .await
        .map_err(|e| AppError::new(ErrorCode::CommonInternal).with_detail(e.to_string()))
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
