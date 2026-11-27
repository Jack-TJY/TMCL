//! Guard the independent vanilla denominator. No Java, jars, or network in CI.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

type Members = BTreeMap<String, Vec<String>>;

#[test]
fn vanilla_targets_are_not_tags_or_configured_variants() {
    for (version, fixture, per_dimension, unique, tag_count) in [
        (
            "1.12.2",
            include_str!("../validation/catalog/1.12.2.json"),
            [6, 1, 1],
            8,
            0,
        ),
        (
            "1.18",
            include_str!("../validation/catalog/1.18.json"),
            [14, 4, 1],
            18,
            0,
        ),
        (
            "26.3",
            include_str!("../validation/catalog/26.3.json"),
            [47, 4, 1],
            52,
            33,
        ),
    ] {
        let catalog: Value = serde_json::from_str(fixture).unwrap();
        assert_eq!(catalog["version"], version);
        let dims: BTreeMap<String, Members> =
            serde_json::from_value(catalog["dimensions"].clone()).unwrap();
        assert_eq!(dims.len(), 3);
        let mut ids = BTreeSet::new();
        for (dim, expected) in [
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end",
        ]
        .into_iter()
        .zip(per_dimension)
        {
            assert_eq!(dims[dim].len(), expected, "{version}/{dim}");
            for (id, members) in &dims[dim] {
                assert!(!id.starts_with('#') && !members.is_empty());
                assert!(members.windows(2).all(|w| w[0] < w[1]));
                ids.insert(id.clone());
            }
        }
        assert_eq!(ids.len(), unique);
        let tags: Members = serde_json::from_value(catalog["tags"].clone()).unwrap();
        assert_eq!(tags.len(), tag_count);
        for (tag, members) in &tags {
            assert!(tag.starts_with('#') && members.iter().all(|id| ids.contains(id)));
            assert!(members.windows(2).all(|w| w[0] < w[1]));
        }
        match version {
            "1.12.2" => {
                assert_eq!(dims["minecraft:overworld"]["Temple"].len(), 4);
                assert!(!ids.contains("desert_pyramid"));
            }
            "1.18" => {
                assert_eq!(dims["minecraft:the_end"]["endcity"], ["minecraft:end_city"]);
                assert_eq!(
                    dims["minecraft:the_nether"]["ruined_portal"],
                    ["minecraft:ruined_portal_nether"]
                );
                assert_eq!(dims["minecraft:overworld"]["ruined_portal"].len(), 6);
            }
            _ => {
                assert_eq!(tags["#minecraft:village"].len(), 5);
                assert_eq!(tags["#minecraft:abandoned_camp"].len(), 18);
                assert!(dims["minecraft:the_nether"].contains_key("minecraft:fortress"));
                assert!(
                    !dims["minecraft:overworld"].contains_key("minecraft:ruined_portal_nether")
                );
            }
        }
    }
}
