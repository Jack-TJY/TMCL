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
        .run(dir.path(), GameVersion::Java26_3, 0, &ctx, |msg| {
            hits.push(serde_json::to_value(msg).unwrap());
            Ok(())
        })
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
            .run(dir.path(), GameVersion::Java1_18, 0, &ctx, |_| panic!())
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
        .run(dir.path(), GameVersion::Java1_12_2, 0, &ctx, |result| {
            let SeedMapResult::Tile { tile, png } = result else {
                panic!()
            };
            assert_eq!(tile.x, -1);
            assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
            count += 1;
            ctx.cancel.cancel();
            Ok(())
        })
        .unwrap_err();
    assert!(err.is_cancelled());
    assert_eq!(count, 1);
}
