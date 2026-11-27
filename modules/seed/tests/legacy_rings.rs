use std::collections::BTreeMap;

use tmcl_common::TaskCtx;
use tmcl_seed::{ChunkBounds, GameVersion, StructureKind, structure_starts};

#[test]
fn all_legacy_rings_match_vanilla_without_duplicates() {
    let ctx = TaskCtx::noop("rings");
    // Use the independent vanilla oracle's version -> seed -> ring order JSON.
    let fixture: BTreeMap<String, BTreeMap<i64, Vec<String>>> =
        serde_json::from_str(include_str!("../validation/legacy/rings.json")).unwrap();
    assert_eq!(fixture.len(), 2);
    for name in ["1.12.2", "1.18"] {
        let version: GameVersion = name.parse().unwrap();
        let seeds = &fixture[name];
        assert_eq!(seeds.len(), 2);
        for seed in [0, -1] {
            let mut expected: Vec<(i32, i32)> = seeds[&seed]
                .iter()
                .map(|pair| {
                    let (x, z) = pair.split_once(',').unwrap();
                    (x.parse().unwrap(), z.parse().unwrap())
                })
                .collect();
            assert_eq!(expected.len(), 128);
            let mut actual = Vec::new();
            for z in (-1536..1536).step_by(256) {
                for x in (-1536..1536).step_by(256) {
                    let bounds = ChunkBounds {
                        min_x: x,
                        max_x: x + 255,
                        min_z: z,
                        max_z: z + 255,
                    };
                    actual.extend(
                        structure_starts(version, seed, StructureKind::Stronghold, bounds, &ctx)
                            .unwrap()
                            .iter()
                            .map(|p| (p.chunk_x, p.chunk_z)),
                    );
                }
            }
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(actual, expected, "{version:?} seed {seed}");
        }
    }
}
