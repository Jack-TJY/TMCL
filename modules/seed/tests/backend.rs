use tmcl_common::TaskCtx;
use tmcl_seed::{
    ChunkBounds, GameVersion::*, StructureKind::*, StructureStart, Tile, biome_at, cached_tile_png,
    parse_seed, structure_starts,
};

#[test]
fn vanilla_biomes() {
    for (version, seed, x, z, expected) in [
        (Java1_12_2, 0, 0, 0, 4),
        (Java1_12_2, -1, 0, 0, 24),
        (Java1_18, 0, 0, 0, 7),
        (Java1_18, -1, 0, 0, 24),
        (Java26_3, 0, 0, 0, 7),
        (Java26_3, -1, 0, 0, 24),
        (Java26_3, 0, 2784, -1280, 188),
        (Java26_3, -1, 672, 1984, 188),
        (Java1_12_2, 12345, 0, 0, 133),
        (Java1_18, 12345, 0, 0, 0),
        (Java26_3, 12345, 0, 0, 0),
        (Java26_3, 12345, 608, -736, 188),
    ] {
        assert_eq!(
            biome_at(version, seed, x, z).unwrap(),
            expected,
            "{version:?}/{seed} at {x},{z}"
        );
    }
}

#[test]
fn vanilla_structure_starts() {
    let ctx = TaskCtx::noop("sample");
    for (v, seed, kind, x, z) in [
        (Java1_12_2, 0, Village, -25, 21),
        (Java1_12_2, -1, Village, 10, -56),
        (Java1_18, 0, Village, 38, 45),
        (Java1_18, -1, Village, 58, -48),
        (Java26_3, 0, Village, 38, 45),
        (Java26_3, -1, Village, 58, -48),
        (Java26_3, 0, AbandonedCamp, 11, -11),
        (Java26_3, -1, AbandonedCamp, 53, 0),
        (Java1_12_2, 12345, Village, -43, 5),
        (Java1_18, 12345, Village, -9, -52),
        (Java26_3, 12345, Village, -9, -52),
        (Java26_3, 12345, AbandonedCamp, 39, -28),
        (Java1_12_2, 0, Stronghold, -9, -99),
        (Java1_12_2, 0, Stronghold, -1224, 475),
        (Java1_12_2, 0, DesertPyramid, -149, -107),
        (Java1_12_2, 0, JunglePyramid, -501, -255),
        (Java1_12_2, 0, OceanMonument, 86, -12),
        (Java1_12_2, -1, Stronghold, -14, 94),
        (Java1_12_2, -1, Stronghold, -1314, 639),
        (Java1_12_2, -1, DesertPyramid, -128, 19),
        (Java1_12_2, -1, JunglePyramid, -246, -201),
        (Java1_12_2, -1, OceanMonument, -45, -22),
        (Java1_18, 0, Stronghold, -13, -106),
        (Java1_18, 0, Stronghold, -1331, 560),
        (Java1_18, 0, DesertPyramid, 0, -188),
        (Java1_18, 0, JunglePyramid, 8, 69),
        (Java1_18, 0, OceanMonument, -71, -15),
        (Java1_18, -1, Stronghold, -11, 88),
        (Java1_18, -1, Stronghold, -1376, 536),
        (Java1_18, -1, DesertPyramid, -54, -31),
        (Java1_18, -1, JunglePyramid, -53, -81),
        (Java1_18, -1, OceanMonument, 9, -27),
        (Java26_3, 0, Stronghold, -13, -106),
        (Java26_3, 0, Stronghold, -1172, 493),
        (Java26_3, 0, DesertPyramid, 0, -188),
        (Java26_3, 0, JunglePyramid, 8, 69),
        (Java26_3, 0, OceanMonument, 86, -12),
        (Java26_3, -1, Stronghold, -11, 88),
        (Java26_3, -1, DesertPyramid, -54, -31),
        (Java26_3, -1, JunglePyramid, -53, -81),
        (Java26_3, -1, OceanMonument, -17, 47),
    ] {
        let bounds = ChunkBounds {
            min_x: x,
            max_x: x,
            min_z: z,
            max_z: z,
        };
        assert_eq!(
            structure_starts(v, seed, kind, bounds, &ctx).unwrap(),
            [StructureStart {
                chunk_x: x,
                chunk_z: z
            }],
            "{v:?}/{seed}"
        );
        assert!(
            structure_starts(
                v,
                seed,
                kind,
                ChunkBounds {
                    min_x: x + 1,
                    max_x: x + 1,
                    ..bounds
                },
                &ctx
            )
            .unwrap()
            .is_empty()
        );
    }
}

#[test]
fn legacy_scattered_features_keep_their_actual_subtype() {
    let ctx = TaskCtx::noop("temple-type");
    // In 1.12.2 both pyramids use the same scattered-feature placement.
    // This negative check would catch accidentally returning both icons.
    for (x, z, wrong) in [(-149, -107, JunglePyramid), (-501, -255, DesertPyramid)] {
        let bounds = ChunkBounds {
            min_x: x,
            max_x: x,
            min_z: z,
            max_z: z,
        };
        assert!(
            structure_starts(Java1_12_2, 0, wrong, bounds, &ctx)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn outer_stronghold_on_strip_boundary_is_reported_once() {
    let start = StructureStart {
        chunk_x: -1172,
        chunk_z: 493,
    };
    let bounds = ChunkBounds {
        min_x: -1172,
        max_x: -1172,
        min_z: 461,
        max_z: 493,
    };
    assert_eq!(
        structure_starts(
            Java26_3,
            0,
            Stronghold,
            bounds,
            &TaskCtx::noop("outer-ring")
        )
        .unwrap(),
        [start]
    );
}

#[test]
fn tile_roundtrip_and_repair() {
    let dir = tempfile::tempdir().unwrap();
    let tile = Tile {
        x: -1,
        z: 0,
        zoom: 0,
    };
    let render =
        || cached_tile_png(dir.path(), Java1_12_2, 0, tile, &TaskCtx::noop("tile")).unwrap();
    let png = render();
    let mut reader = png::Decoder::new(std::io::Cursor::new(&png))
        .read_info()
        .unwrap();
    assert_eq!((reader.info().width, reader.info().height), (256, 256));
    let mut pixels = vec![0; 256 * 256];
    reader.next_frame(&mut pixels).unwrap();
    for (x, z) in [(0, 0), (21, 190), (255, 255)] {
        assert_eq!(
            pixels[z * 256 + x],
            biome_at(Java1_12_2, 0, x as i32 - 256, z as i32).unwrap()
        );
    }
    let cache = dir.path().join("overworld-y64-v1/1.12.2/0/0/-1_0.png");
    assert_eq!(std::fs::read(&cache).unwrap(), png);
    let stamp = std::fs::metadata(&cache).unwrap().modified().unwrap();
    assert_eq!(render(), png);
    assert_eq!(
        std::fs::metadata(&cache).unwrap().modified().unwrap(),
        stamp
    );
    std::fs::write(&cache, b"incomplete").unwrap();
    assert_eq!(render(), png);
    assert_eq!(std::fs::read(cache).unwrap(), png);
}

#[test]
fn rejects_out_of_profile_and_honors_cancellation() {
    assert_eq!(parse_seed("😀").unwrap(), 1772899);
    assert_eq!(parse_seed("-1").unwrap(), -1);
    assert!(parse_seed(" ").is_err());
    assert!(biome_at(Java26_3, 0, 30_000_001, 0).is_err());
    let ctx = TaskCtx::noop("test");
    let bounds = ChunkBounds {
        min_x: 0,
        max_x: 0,
        min_z: 0,
        max_z: 0,
    };
    for version in [Java1_12_2, Java1_18] {
        assert!(structure_starts(version, 0, AbandonedCamp, bounds, &ctx).is_err());
    }
    ctx.cancel.cancel();
    assert!(structure_starts(Java26_3, 0, Village, bounds, &ctx).is_err());
}
