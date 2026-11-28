use crate::{Dimension, GameVersion, StructureKind, StructureStart, invalid};
use tmcl_common::{AppError, AppResult, ErrorCode};

unsafe extern "C" {
    fn tmcl_seed_biomes(
        version: i32,
        seed: i64,
        x: i32,
        z: i32,
        step: i32,
        width: i32,
        height: i32,
        ids: *mut u8,
    ) -> i32;
    fn tmcl_seed_structures(
        version: i32,
        dimension: i32,
        seed: i64,
        kind: i32,
        min_cx: i32,
        min_cz: i32,
        max_cx: i32,
        max_cz: i32,
        out: *mut StructureStart,
        capacity: usize,
        written: *mut usize,
    ) -> i32;
}

fn check(status: i32) -> AppResult<()> {
    match status {
        0 => Ok(()),
        1 => Err(invalid("region")),
        _ => Err(AppError::new(ErrorCode::CommonInternal)
            .with_detail(format!("seed engine status {status}"))),
    }
}

pub(super) fn biomes(
    version: GameVersion,
    seed: i64,
    origin: (i32, i32),
    step: i32,
    width: i32,
    ids: &mut [u8],
) -> AppResult<()> {
    // All callers produce non-empty rectangles; validate the allocation shape
    // here as this is the boundary where the C function receives a raw pointer.
    if !(1..=256).contains(&width) || ids.is_empty() || !ids.len().is_multiple_of(width as usize) {
        return Err(invalid("biomes"));
    }
    let height = ids.len() / width as usize;
    if height > 256 {
        return Err(invalid("biomes"));
    }
    // SAFETY: ids holds width * height writable bytes throughout the call.
    check(unsafe {
        tmcl_seed_biomes(
            version as i32,
            seed,
            origin.0,
            origin.1,
            step,
            width,
            height as i32,
            ids.as_mut_ptr(),
        )
    })
}

pub(super) fn structures(
    version: GameVersion,
    dimension: Dimension,
    seed: i64,
    kind: StructureKind,
    bounds: crate::ChunkBounds,
    out: &mut [StructureStart],
) -> AppResult<usize> {
    let mut written = 0;
    // SAFETY: StructureStart has the repr(C) layout of two consecutive i32s.
    // The callee receives the exact allocation capacity and a live output count.
    check(unsafe {
        tmcl_seed_structures(
            version as i32,
            dimension as i32,
            seed,
            kind as i32,
            bounds.min_x,
            bounds.min_z,
            bounds.max_x,
            bounds.max_z,
            out.as_mut_ptr(),
            out.len(),
            &mut written,
        )
    })?;
    if written > out.len() {
        return Err(AppError::new(ErrorCode::CommonInternal).with_detail("seed engine count"));
    }
    Ok(written)
}
