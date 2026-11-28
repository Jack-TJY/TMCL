use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;
use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};

use crate::{Dimension, GameVersion, WORLD_LIMIT, invalid, native};

/// PNG width and height in pixels.
pub const TILE_SIZE: i32 = 256;
/// Zoom 0 samples one block per pixel; zoom N samples every 2^N blocks.
pub const MAX_ZOOM: u8 = 12;
const CACHE_FORMAT: &str = "seedatlas-y64-v2";
const MAX_CACHED_BYTES: u64 = 128 * 1024;
const STRIP_ROWS: i32 = 8;

/// Tile indexes increase east (+X) and south (+Z).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    pub x: i32,
    pub z: i32,
    pub zoom: u8,
}

impl Tile {
    fn grid(self) -> AppResult<(i32, i32, i32)> {
        if self.zoom > MAX_ZOOM {
            return Err(invalid("zoom"));
        }
        let step = 1i32 << self.zoom;
        let axis = |index: i32| -> AppResult<i32> {
            let origin = i64::from(index) * i64::from(TILE_SIZE * step);
            let end = origin + i64::from((TILE_SIZE - 1) * step);
            if origin < -i64::from(WORLD_LIMIT) || end > i64::from(WORLD_LIMIT) {
                return Err(invalid("tile"));
            }
            Ok(origin as i32)
        };
        Ok((axis(self.x)?, axis(self.z)?, step))
    }

    fn cache_path(
        self,
        root: &Path,
        version: GameVersion,
        dimension: Dimension,
        seed: i64,
    ) -> PathBuf {
        root.join(CACHE_FORMAT)
            .join(version.as_str())
            .join(dimension.as_str())
            .join(seed.to_string())
            .join(self.zoom.to_string())
            .join(format!("{}_{}.png", self.x, self.z))
    }
}

/// Get the raw biome ID at a single block in the fixed Y=64 plane.
pub fn biome_at(
    version: GameVersion,
    dimension: Dimension,
    seed: i64,
    x: i32,
    z: i32,
) -> AppResult<u8> {
    if !(-WORLD_LIMIT..=WORLD_LIMIT).contains(&x) || !(-WORLD_LIMIT..=WORLD_LIMIT).contains(&z) {
        return Err(invalid("block"));
    }
    let mut id = [0];
    native::biomes(version, dimension, seed, (x, z), 1, 1, &mut id)?;
    Ok(id[0])
}

/// TMCL's map palette. PNG palette indexes preserve the engine's biome IDs.
pub fn biome_color(id: u8) -> [u8; 3] {
    match id {
        0 => [56, 112, 164],
        24 => [35, 68, 114],
        7 => [90, 156, 207],
        10 | 11 => [165, 210, 230],
        44..=50 => [37, 125 + (50 - id) * 9, 157 + (50 - id) * 8],
        1 | 129 => [149, 185, 100],
        2 | 17 | 130 => [223, 197, 130],
        3 | 20 | 34 | 131 | 162 => [132, 139, 121],
        4 | 18 | 132 => [76, 140, 76],
        5 | 19 | 133 => [66, 118, 105],
        6 | 134 | 184 => [81, 107, 71],
        12 | 13 | 140 => [217, 233, 235],
        14 | 15 => [163, 123, 157],
        16 | 25 | 26 => [226, 209, 164],
        21..=23 | 149 | 151 | 168 | 169 => [42, 121, 61],
        27 | 28 | 155 | 156 => [110, 157, 95],
        29 | 157 => [55, 99, 64],
        30 | 31 | 158 => [159, 191, 179],
        32 | 33 | 160 | 161 => [81, 113, 87],
        35 | 36 | 163 | 164 => [184, 172, 94],
        37..=39 | 165..=167 => [190, 119, 78],
        174 => [140, 117, 98],
        175 => [104, 145, 98],
        177 => [146, 188, 142],
        178..=182 => [
            190 + (id - 178) * 8,
            201 + (id - 178) * 6,
            200 + (id - 178) * 6,
        ],
        183 => [58, 83, 89],
        185 => [211, 157, 174],
        186 => [157, 167, 158],
        187 => [170, 153, 84],
        8 => [96, 30, 30],
        9 => [24, 20, 32],
        40 => [196, 190, 150],
        41 => [186, 180, 140],
        42 => [208, 200, 158],
        43 => [166, 160, 122],
        170 => [104, 76, 58],
        171 => [136, 40, 58],
        172 => [38, 104, 96],
        173 => [86, 82, 88],
        188 => [141, 151, 76],
        _ => [131, 129, 137],
    }
}

/// Load or render a palette-indexed PNG. The cache key includes engine/palette
/// revision, world profile, game version, seed, zoom and tile indexes.
pub fn cached_tile_png(
    cache_root: &Path,
    version: GameVersion,
    dimension: Dimension,
    seed: i64,
    tile: Tile,
    ctx: &TaskCtx,
) -> AppResult<Vec<u8>> {
    let (x, z, step) = tile.grid()?;
    ctx.check_cancelled()?;
    let path = tile.cache_path(cache_root, version, dimension, seed);
    if let Some(bytes) = read_cached(&path)? {
        ctx.check_cancelled()?;
        report_rows(ctx, TILE_SIZE);
        return Ok(bytes);
    }
    let mut ids = vec![0; (TILE_SIZE * TILE_SIZE) as usize];
    for (index, strip) in ids
        .chunks_mut((TILE_SIZE * STRIP_ROWS) as usize)
        .enumerate()
    {
        ctx.check_cancelled()?;
        let row = index as i32 * STRIP_ROWS;
        native::biomes(
            version,
            dimension,
            seed,
            (x, z + row * step),
            step,
            TILE_SIZE,
            strip,
        )?;
        report_rows(ctx, row + STRIP_ROWS);
    }
    ctx.check_cancelled()?;
    let bytes = encode(&ids)
        .map_err(|err| AppError::new(ErrorCode::CommonInternal).with_detail(err.to_string()))?;
    let parent = path.parent().ok_or_else(|| invalid("cache"))?;
    fs::create_dir_all(parent)?;
    // A unique sibling and an atomic replace prevent concurrent writers from
    // publishing a partial tile (or from truncating each other's temp files).
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(&bytes)?;
    ctx.check_cancelled()?;
    temp.persist(&path).map_err(|err| err.error)?;
    Ok(bytes)
}

fn report_rows(ctx: &TaskCtx, rows: i32) {
    ctx.report(ProgressUpdate::new("seed_tile").items(rows as u64, TILE_SIZE as u64));
}

fn encode(ids: &[u8]) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    let palette = (0..=255).flat_map(biome_color).collect::<Vec<_>>();
    {
        let mut encoder = png::Encoder::new(&mut bytes, TILE_SIZE as u32, TILE_SIZE as u32);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(palette);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(ids)?;
        writer.finish()?;
    }
    Ok(bytes)
}

fn read_cached(path: &Path) -> AppResult<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_CACHED_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CACHED_BYTES || !valid_png(&bytes) {
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn valid_png(bytes: &[u8]) -> bool {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: 1024 * 1024 });
    let Ok(mut reader) = decoder.read_info() else {
        return false;
    };
    let info = reader.info();
    if info.width != TILE_SIZE as u32
        || info.height != TILE_SIZE as u32
        || info.color_type != png::ColorType::Indexed
        || info.bit_depth != png::BitDepth::Eight
    {
        return false;
    }
    let mut ids = vec![0; (TILE_SIZE * TILE_SIZE) as usize];
    reader.next_frame(&mut ids).is_ok() && reader.finish().is_ok()
}
