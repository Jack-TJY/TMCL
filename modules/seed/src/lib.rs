//! Offline seed map for default Java Edition overworlds at block Y=64.
//!
//! CPU-bound queries are synchronous. Run them on a blocking worker when
//! integrating with an async application. Structure positions are start chunks.

mod map;
mod native;
mod save;
mod structures;

use std::str::FromStr;
use tmcl_common::{AppError, AppResult, ErrorCode};

pub use map::{MAX_ZOOM, TILE_SIZE, Tile, biome_at, biome_color, cached_tile_png};
pub use save::{SaveInfo, read_level_dat};
pub use structures::{
    ChunkBounds, StructureKind, StructureStart, dimension_of, structure_starts, supports,
};

const WORLD_LIMIT: i32 = 30_000_000;

/// Default Java Edition dimension for structure placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Dimension {
    Nether = -1,
    Overworld = 0,
    End = 1,
}

impl Dimension {
    /// Identifier used in the IPC layer.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Overworld => "overworld",
            Self::Nether => "the_nether",
            Self::End => "the_end",
        }
    }
}

impl FromStr for Dimension {
    type Err = AppError;

    fn from_str(dimension: &str) -> AppResult<Self> {
        match dimension {
            "overworld" => Ok(Self::Overworld),
            "the_nether" => Ok(Self::Nether),
            "the_end" => Ok(Self::End),
            _ => Err(AppError::new(ErrorCode::SeedInvalidInput).with_param("dimension", dimension)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameVersion {
    mc: i32,
    id: &'static str,
    label: &'static str,
    aliases: &'static [&'static str],
}

const V_B1_7: GameVersion = GameVersion {
    mc: 1,
    id: "b1.7",
    label: "Beta 1.7",
    aliases: &["Beta 1.7"],
};
const V_B1_8: GameVersion = GameVersion {
    mc: 2,
    id: "b1.8",
    label: "Beta 1.8",
    aliases: &["Beta 1.8"],
};
const V_1_0: GameVersion = GameVersion {
    mc: 3,
    id: "1.0",
    label: "1.0",
    aliases: &[],
};
const V_1_1: GameVersion = GameVersion {
    mc: 4,
    id: "1.1",
    label: "1.1",
    aliases: &[],
};
const V_1_2: GameVersion = GameVersion {
    mc: 5,
    id: "1.2",
    label: "1.2",
    aliases: &[],
};
const V_1_3: GameVersion = GameVersion {
    mc: 6,
    id: "1.3",
    label: "1.3",
    aliases: &[],
};
const V_1_4: GameVersion = GameVersion {
    mc: 7,
    id: "1.4",
    label: "1.4",
    aliases: &[],
};
const V_1_5: GameVersion = GameVersion {
    mc: 8,
    id: "1.5",
    label: "1.5",
    aliases: &[],
};
const V_1_6: GameVersion = GameVersion {
    mc: 9,
    id: "1.6",
    label: "1.6",
    aliases: &[],
};
const V_1_7: GameVersion = GameVersion {
    mc: 10,
    id: "1.7",
    label: "1.7",
    aliases: &[],
};
const V_1_8: GameVersion = GameVersion {
    mc: 11,
    id: "1.8",
    label: "1.8",
    aliases: &[],
};
const V_1_9: GameVersion = GameVersion {
    mc: 12,
    id: "1.9",
    label: "1.9",
    aliases: &[],
};
const V_1_10: GameVersion = GameVersion {
    mc: 13,
    id: "1.10",
    label: "1.10",
    aliases: &[],
};
const V_1_11: GameVersion = GameVersion {
    mc: 14,
    id: "1.11",
    label: "1.11",
    aliases: &[],
};
const V_1_12_2: GameVersion = GameVersion {
    mc: 15,
    id: "1.12.2",
    label: "1.12.2",
    aliases: &["1.12"],
};
const V_1_13: GameVersion = GameVersion {
    mc: 16,
    id: "1.13",
    label: "1.13.2",
    aliases: &["1.13.2"],
};
const V_1_14: GameVersion = GameVersion {
    mc: 17,
    id: "1.14",
    label: "1.14.4",
    aliases: &["1.14.4"],
};
const V_1_15: GameVersion = GameVersion {
    mc: 18,
    id: "1.15",
    label: "1.15.2",
    aliases: &["1.15.2"],
};
const V_1_16_1: GameVersion = GameVersion {
    mc: 19,
    id: "1.16.1",
    label: "1.16.1",
    aliases: &[],
};
const V_1_16_5: GameVersion = GameVersion {
    mc: 20,
    id: "1.16.5",
    label: "1.16.5",
    aliases: &["1.16"],
};
const V_1_17_1: GameVersion = GameVersion {
    mc: 21,
    id: "1.17.1",
    label: "1.17.1",
    aliases: &["1.17"],
};
const V_1_18: GameVersion = GameVersion {
    mc: 22,
    id: "1.18",
    label: "1.18.2",
    aliases: &["1.18.2"],
};
const V_1_19_2: GameVersion = GameVersion {
    mc: 23,
    id: "1.19.2",
    label: "1.19.2",
    aliases: &[],
};
const V_1_19_4: GameVersion = GameVersion {
    mc: 24,
    id: "1.19.4",
    label: "1.19.4",
    aliases: &["1.19"],
};
const V_1_20_6: GameVersion = GameVersion {
    mc: 25,
    id: "1.20.6",
    label: "1.20.6",
    aliases: &["1.20"],
};
const V_1_21_1: GameVersion = GameVersion {
    mc: 26,
    id: "1.21.1",
    label: "1.21.1",
    aliases: &[],
};
const V_1_21_2: GameVersion = GameVersion {
    mc: 27,
    id: "1.21.2",
    label: "1.21.2",
    aliases: &[],
};
const V_1_21_3: GameVersion = GameVersion {
    mc: 28,
    id: "1.21.3",
    label: "1.21.3",
    aliases: &[],
};
const V_1_21_4: GameVersion = GameVersion {
    mc: 29,
    id: "1.21.4",
    label: "1.21.4",
    aliases: &[],
};
const V_1_21_5: GameVersion = GameVersion {
    mc: 30,
    id: "1.21.5",
    label: "1.21.5",
    aliases: &[],
};
const V_1_21_6: GameVersion = GameVersion {
    mc: 31,
    id: "1.21.6",
    label: "1.21.6",
    aliases: &[],
};
const V_1_21_7: GameVersion = GameVersion {
    mc: 32,
    id: "1.21.7",
    label: "1.21.7",
    aliases: &[],
};
const V_1_21_8: GameVersion = GameVersion {
    mc: 33,
    id: "1.21.8",
    label: "1.21.8",
    aliases: &[],
};
const V_1_21_9: GameVersion = GameVersion {
    mc: 34,
    id: "1.21.9",
    label: "1.21.9",
    aliases: &[],
};
const V_1_21_10: GameVersion = GameVersion {
    mc: 35,
    id: "1.21.10",
    label: "1.21.10",
    aliases: &[],
};
const V_1_21_11: GameVersion = GameVersion {
    mc: 36,
    id: "1.21.11",
    label: "1.21.11",
    aliases: &["1.21"],
};
const V_26_1: GameVersion = GameVersion {
    mc: 37,
    id: "26.1",
    label: "26.1",
    aliases: &[],
};
const V_26_1_1: GameVersion = GameVersion {
    mc: 38,
    id: "26.1.1",
    label: "26.1.1",
    aliases: &[],
};
const V_26_1_2: GameVersion = GameVersion {
    mc: 39,
    id: "26.1.2",
    label: "26.1.2",
    aliases: &[],
};
const V_26_2: GameVersion = GameVersion {
    mc: 40,
    id: "26.2",
    label: "26.2",
    aliases: &[],
};
const V_26_3: GameVersion = GameVersion {
    mc: 41,
    id: "26.3",
    label: "26.3",
    aliases: &[],
};

/// Every release the pinned engine models, oldest first.
const ALL_VERSIONS: &[GameVersion] = &[
    V_B1_7, V_B1_8, V_1_0, V_1_1, V_1_2, V_1_3, V_1_4, V_1_5, V_1_6, V_1_7, V_1_8, V_1_9, V_1_10,
    V_1_11, V_1_12_2, V_1_13, V_1_14, V_1_15, V_1_16_1, V_1_16_5, V_1_17_1, V_1_18, V_1_19_2,
    V_1_19_4, V_1_20_6, V_1_21_1, V_1_21_2, V_1_21_3, V_1_21_4, V_1_21_5, V_1_21_6, V_1_21_7,
    V_1_21_8, V_1_21_9, V_1_21_10, V_1_21_11, V_26_1, V_26_1_1, V_26_1_2, V_26_2, V_26_3,
];

#[allow(non_upper_case_globals)]
impl GameVersion {
    /// Canonical identifier used in cache paths and IPC.
    pub fn as_str(self) -> &'static str {
        self.id
    }
    /// Human-readable release name.
    pub fn label(self) -> &'static str {
        self.label
    }
    /// Numeric engine version, passed unchanged to the C bridge.
    pub fn engine(self) -> i32 {
        self.mc
    }
    /// True when this release is at least as new as `other`.
    pub fn at_least(self, other: Self) -> bool {
        self.mc >= other.mc
    }
    pub fn all() -> &'static [GameVersion] {
        ALL_VERSIONS
    }
    pub const Java1_12_2: GameVersion = V_1_12_2;
    pub const Java1_18: GameVersion = V_1_18;
    pub const Java26_3: GameVersion = V_26_3;
}

impl FromStr for GameVersion {
    type Err = AppError;
    fn from_str(version: &str) -> AppResult<Self> {
        ALL_VERSIONS
            .iter()
            .copied()
            .find(|candidate| candidate.id == version || candidate.aliases.contains(&version))
            .ok_or_else(|| {
                AppError::new(ErrorCode::SeedUnsupportedVersion).with_param("version", version)
            })
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
