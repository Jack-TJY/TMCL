use super::*;
use serde_json::json;

fn args(query: serde_json::Value) -> SeedMapArgs {
    serde_json::from_value(json!({"version":"26.3","seed":"0","query":query})).unwrap()
}

#[test]
fn refuses_empty_oversized_and_unsupported_requests() {
    let tile = json!({"x":0,"z":0,"zoom":0});
    for tiles in [vec![], vec![tile.clone(); 17]] {
        let a = args(json!({"type":"tiles","tiles":tiles}));
        assert_eq!(a.inputs().unwrap_err().code, ErrorCode::SeedInvalidInput);
    }
    let mut a = args(json!({"type":"tiles","tiles":[tile]}));
    a.seed = "x".repeat(1025);
    assert_eq!(a.inputs().unwrap_err().code, ErrorCode::SeedInvalidInput);
    a.seed = "9223372036854775807".into();
    assert_eq!(a.inputs().unwrap().1, i64::MAX);
    a.version = "unknown".into();
    assert_eq!(
        a.inputs().unwrap_err().code,
        ErrorCode::SeedUnsupportedVersion
    );
}

#[test]
fn returns_real_structure_chunk_and_refuses_unsupported_kind() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = TaskCtx::noop("seed_map");
    let q = json!({"type":"structures","kind":"abandoned_camp","bounds":{"min_x":11,"max_x":11,"min_z":-11,"max_z":-11}});
    let mut hits = vec![];
    args(q.clone())
        .query
        .run(
            dir.path(),
            GameVersion::Java26_3,
            tmcl_seed::Dimension::Overworld,
            0,
            &ctx,
            |msg| {
                hits.push(serde_json::to_value(msg).unwrap());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        hits,
        [
            json!({"type":"structures","kind":"abandoned_camp","starts":[{"chunk_x":11,"chunk_z":-11}]})
        ]
    );
    assert_eq!(
        args(q)
            .query
            .run(
                dir.path(),
                GameVersion::Java1_18,
                tmcl_seed::Dimension::Overworld,
                0,
                &ctx,
                |_| panic!()
            )
            .unwrap_err()
            .code,
        ErrorCode::SeedInvalidInput
    );
}

#[test]
fn streams_png_and_cancellation_stops_the_rest_of_batch() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = TaskCtx::noop("seed_map");
    let a = args(json!({"type":"tiles","tiles":[{"x":-1,"z":0,"zoom":0},{"x":0,"z":0,"zoom":0}]}));
    let mut count = 0;
    let err = a
        .query
        .run(
            dir.path(),
            GameVersion::Java1_12_2,
            tmcl_seed::Dimension::Overworld,
            0,
            &ctx,
            |result| {
                let SeedMapResult::Tile { tile, png } = result else {
                    panic!()
                };
                assert_eq!(tile.x, -1);
                assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
                count += 1;
                ctx.cancel.cancel();
                Ok(())
            },
        )
        .unwrap_err();
    assert!(err.is_cancelled());
    assert_eq!(count, 1);
}
fn level_dat(seed: i64, name: &str) -> Vec<u8> {
    use std::io::Write;
    fn tag(out: &mut Vec<u8>, kind: u8, name: &str) {
        out.push(kind);
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
    }
    let mut nbt = Vec::new();
    tag(&mut nbt, 10, "");
    tag(&mut nbt, 10, "Data");
    tag(&mut nbt, 4, "RandomSeed");
    nbt.extend_from_slice(&seed.to_be_bytes());
    tag(&mut nbt, 8, "LevelName");
    nbt.extend_from_slice(&(name.len() as u16).to_be_bytes());
    nbt.extend_from_slice(name.as_bytes());
    nbt.push(0);
    nbt.push(0);
    let mut out = Vec::new();
    let mut encoder = flate2::write::GzEncoder::new(&mut out, flate2::Compression::fast());
    encoder.write_all(&nbt).unwrap();
    encoder.finish().unwrap();
    out
}

#[test]
fn lists_saved_worlds_and_skips_unreadable_ones() {
    let dir = tempfile::tempdir().unwrap();
    assert!(list_saves(dir.path()).is_empty());
    let saves = dir.path().join("saves");
    std::fs::create_dir_all(saves.join("Alpha")).unwrap();
    std::fs::write(saves.join("Alpha/level.dat"), level_dat(-1, "Alpha")).unwrap();
    std::fs::create_dir_all(saves.join("Broken")).unwrap();
    std::fs::write(saves.join("Broken/level.dat"), b"not gzip").unwrap();
    let worlds = list_saves(dir.path());
    assert_eq!(worlds.len(), 1);
    assert_eq!(worlds[0].dir, "Alpha");
    assert_eq!(worlds[0].name, "Alpha");
    assert_eq!(worlds[0].seed, "-1");
}
