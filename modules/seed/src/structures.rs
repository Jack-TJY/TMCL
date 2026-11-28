use crate::{GameVersion, WORLD_LIMIT, invalid, native};
use tmcl_common::{AppResult, ProgressUpdate, TaskCtx};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum StructureKind {
    Village = 0,
    AbandonedCamp = 1,
    /// Concentric-ring stronghold start, not the portal room.
    Stronghold = 2,
    DesertPyramid = 3,
    JunglePyramid = 4,
    OceanMonument = 5,
    /// Woodland mansion start chunk; requires the dark forest biome.
    Mansion = 6,
}

/// Inclusive chunk bounds. A query covers at most 256 chunks on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkBounds {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

/// Start chunk only: multiply each coordinate by 16 for its NW block corner.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct StructureStart {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

/// Return valid structure starts within bounds, in deterministic Z/X order.
pub fn structure_starts(
    version: GameVersion,
    seed: i64,
    kind: StructureKind,
    bounds: ChunkBounds,
    ctx: &TaskCtx,
) -> AppResult<Vec<StructureStart>> {
    ctx.check_cancelled()?;
    if kind == StructureKind::AbandonedCamp && version != GameVersion::Java26_3 {
        return Err(invalid("structure"));
    }
    // 1.12.2 mansions are not verified yet; keep the version gap explicit.
    if kind == StructureKind::Mansion && version == GameVersion::Java1_12_2 {
        return Err(invalid("structure"));
    }
    let size = |min: i32, max: i32| -> AppResult<usize> {
        if min < -WORLD_LIMIT / 16
            || max > WORLD_LIMIT / 16
            || max < min
            || i64::from(max) - i64::from(min) >= 256
        {
            return Err(invalid("chunks"));
        }
        Ok((max - min + 1) as usize)
    };
    let width = size(bounds.min_x, bounds.max_x)?;
    let height = size(bounds.min_z, bounds.max_z)?;
    let mut starts = Vec::new();
    // Check cancellation between short strips. At most one start of the
    // requested type fits per chunk, an upper bound valid for all versions.
    let mut buffer = vec![StructureStart::default(); width * height.min(32)];
    for offset in (0..height).step_by(32) {
        ctx.check_cancelled()?;
        let rows = (height - offset).min(32);
        let min_z = bounds.min_z + offset as i32;
        let strip = ChunkBounds {
            min_z,
            max_z: min_z + rows as i32 - 1,
            ..bounds
        };
        let count = native::structures(version, seed, kind, strip, &mut buffer)?;
        starts.extend_from_slice(&buffer[..count]);
        ctx.report(
            ProgressUpdate::new("seed_structures").items((offset + rows) as u64, height as u64),
        );
    }
    ctx.check_cancelled()?;
    starts.sort_unstable_by_key(|s| (s.chunk_z, s.chunk_x));
    Ok(starts)
}
