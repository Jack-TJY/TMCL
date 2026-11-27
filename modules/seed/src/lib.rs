//! Offline seed map for default Java Edition overworlds at block Y=64.
//!
//! CPU-bound queries are synchronous. Run them on a blocking worker when
//! integrating with an async application. Structure positions are start chunks.

mod map;
mod native;
mod structures;

use std::str::FromStr;
use tmcl_common::{AppError, AppResult, ErrorCode};

pub use map::{MAX_ZOOM, TILE_SIZE, Tile, biome_at, biome_color, cached_tile_png};
pub use structures::{ChunkBounds, StructureKind, StructureStart, structure_starts};

const WORLD_LIMIT: i32 = 30_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum GameVersion {
    Java1_12_2 = 0,
    Java1_18 = 1,
    Java26_3 = 2,
}

impl GameVersion {
    /// Canonical Java Edition release identifier used in cache paths.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Java1_12_2 => "1.12.2",
            Self::Java1_18 => "1.18",
            Self::Java26_3 => "26.3",
        }
    }
}

impl FromStr for GameVersion {
    type Err = AppError;

    fn from_str(version: &str) -> AppResult<Self> {
        match version {
            "1.12.2" => Ok(Self::Java1_12_2),
            "1.18" => Ok(Self::Java1_18),
            "26.3" => Ok(Self::Java26_3),
            _ => {
                Err(AppError::new(ErrorCode::SeedUnsupportedVersion).with_param("version", version))
            }
        }
    }
}

/// Parse an explicit signed decimal i64 or sign-extend Java String.hashCode()
/// over UTF-16 code units. An empty seed represents no chosen map.
pub fn parse_seed(input: &str) -> AppResult<i64> {
    let input = input.trim_matches(|c| c <= '\u{20}');
    if input.is_empty() {
        return Err(invalid("seed"));
    }
    if let Ok(number) = input.parse::<i64>() {
        return Ok(number);
    }
    let hash = input.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    });
    Ok(i64::from(hash))
}

fn invalid(field: &'static str) -> AppError {
    AppError::new(ErrorCode::SeedInvalidInput).with_param("field", field)
}
