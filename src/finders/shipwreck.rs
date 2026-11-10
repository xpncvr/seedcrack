use super::chunk_biome;
use super::template::{Rotation, template_matches};
use crate::placement::StructureSet;
use crate::world::{Cuboid, Found, Observation, StructureObservation, World};

const PIVOT: [i32; 2] = [4, 15];

const SEA_LEVEL: i32 = 63;

const BIOMES: [&str; 11] = [
    "deep_frozen_ocean",
    "deep_cold_ocean",
    "deep_ocean",
    "deep_lukewarm_ocean",
    "frozen_ocean",
    "ocean",
    "cold_ocean",
    "lukewarm_ocean",
    "warm_ocean",
    "beach",
    "snowy_beach",
];

struct Template {
    size: [i32; 3],
    palettes: &'static [&'static [&'static str]],
    column: &'static [(i32, u8)],
    blocks: &'static [([i32; 3], u8)],
}

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    let biome = chunk_biome(world, chunk_x, SEA_LEVEL, chunk_z);
    if !biome.is_empty() && !BIOMES.contains(&biome) {
        return;
    }
    let (x0, z0) = (chunk_x << 4, chunk_z << 4);
    let (min_y, max_y) = (world.min_y(), world.max_y());
    let column: Vec<&str> = (min_y..max_y).map(|y| world.block(x0 + PIVOT[0], y, z0 + PIVOT[1])).collect();
    for t in &TEMPLATES {
        for palette in t.palettes {
            for y0 in min_y..=max_y - t.size[1] {
                let on_column = t
                    .column
                    .iter()
                    .all(|&(y, i)| column[(y0 + y - min_y) as usize] == palette[i as usize]);
                if !on_column {
                    continue;
                }
                let origin = [x0, y0, z0];
                for rotation in Rotation::ALL {
                    let whole = t.blocks.iter().all(|&(p, i)| {
                        let q = rotation.transform(p, PIVOT);
                        world.block(x0 + q[0], y0 + q[1], z0 + q[2]) == palette[i as usize]
                    });
                    if whole {
                        out.push(Found {
                            observation: Observation::Structure(StructureObservation {
                                set: StructureSet::Shipwrecks,
                                chunk_x,
                                chunk_z,
                            }),
                            boxes: vec![rotation.template_box(origin, t.size, PIVOT), Cuboid::block(x0, y0, z0)],
                            color: 0,
                        });
                        return;
                    }
                }
            }
        }
    }
}

const TEMPLATES: [Template; 20] = [
    Template {
        size: [9, 9, 16],
        palettes: &[
            &["oak_planks", "spruce_fence", "spruce_planks", "oak_stairs", "spruce_slab", "oak_fence"],
            &["jungle_planks", "spruce_fence", "spruce_planks", "jungle_stairs", "spruce_slab", "jungle_fence"],
            &["dark_oak_planks", "jungle_fence", "jungle_planks", "dark_oak_stairs", "jungle_slab", "dark_oak_fence"],
            &["dark_oak_planks", "spruce_fence", "spruce_planks", "dark_oak_stairs", "spruce_slab", "dark_oak_fence"],
            &["spruce_planks", "oak_fence", "oak_planks", "spruce_stairs", "oak_slab", "spruce_fence"],
            &["spruce_planks", "jungle_fence", "jungle_planks", "spruce_stairs", "jungle_slab", "spruce_fence"],
            &["spruce_planks", "dark_oak_fence", "dark_oak_planks", "spruce_stairs", "dark_oak_slab", "spruce_fence"],
            &["oak_planks", "birch_fence", "birch_planks", "oak_stairs", "birch_slab", "oak_fence"],
        ],
        column: &[(7, 0), (8, 1)],
        blocks: &[
            ([4, 0, 0], 2), ([0, 6, 6], 3), ([8, 4, 8], 3), ([4, 0, 13], 0), ([4, 0, 6], 2),
            ([0, 8, 11], 3), ([1, 3, 10], 3), ([5, 8, 9], 0), ([7, 4, 3], 2), ([2, 4, 2], 2),
            ([5, 4, 11], 3), ([4, 4, 7], 4), ([1, 2, 5], 3), ([7, 7, 12], 0), ([1, 5, 13], 3),
            ([5, 1, 3], 0), ([5, 1, 9], 3), ([4, 4, 14], 3), ([8, 7, 9], 0), ([1, 6, 9], 5),
            ([2, 1, 2], 3), ([2, 1, 8], 3),
        ],
    },
    Template {
        size: [9, 9, 16],
        palettes: &[
            &["oak_planks", "spruce_fence", "spruce_planks", "oak_stairs", "spruce_slab", "oak_fence"],
            &["jungle_planks", "spruce_fence", "spruce_planks", "jungle_stairs", "spruce_slab", "jungle_fence"],
            &["dark_oak_planks", "jungle_fence", "jungle_planks", "dark_oak_stairs", "jungle_slab", "dark_oak_fence"],
            &["dark_oak_planks", "spruce_fence", "spruce_planks", "dark_oak_stairs", "spruce_slab", "dark_oak_fence"],
            &["spruce_planks", "oak_fence", "oak_planks", "spruce_stairs", "oak_slab", "spruce_fence"],
            &["spruce_planks", "jungle_fence", "jungle_planks", "spruce_stairs", "jungle_slab", "spruce_fence"],
            &["spruce_planks", "dark_oak_fence", "dark_oak_planks", "spruce_stairs", "dark_oak_slab", "spruce_fence"],
            &["oak_planks", "birch_fence", "birch_planks", "oak_stairs", "birch_slab", "oak_fence"],
        ],
        column: &[(7, 0), (8, 1)],
        blocks: &[
            ([4, 0, 0], 2), ([0, 6, 6], 3), ([8, 4, 8], 3), ([4, 0, 13], 0), ([4, 0, 6], 2),
            ([0, 8, 11], 3), ([1, 3, 10], 3), ([7, 4, 3], 2), ([7, 8, 11], 1), ([1, 2, 3], 3),
            ([4, 6, 8], 4), ([7, 3, 12], 3), ([3, 4, 13], 0), ([5, 2, 9], 0), ([4, 4, 5], 2),
            ([5, 1, 3], 0), ([1, 2, 7], 3), ([3, 7, 11], 4), ([1, 6, 9], 5), ([6, 6, 13], 5),
            ([7, 2, 6], 3), ([8, 7, 8], 3),
        ],
    },
    Template {
        size: [9, 9, 24],
        palettes: &[
            &["spruce_planks", "oak_planks", "spruce_stairs", "oak_stairs", "spruce_fence", "oak_trapdoor"],
            &["spruce_planks", "jungle_planks", "spruce_stairs", "jungle_stairs", "spruce_fence", "jungle_trapdoor"],
            &["jungle_planks", "dark_oak_planks", "jungle_stairs", "dark_oak_stairs", "jungle_fence", "dark_oak_trapdoor"],
            &["spruce_planks", "dark_oak_planks", "spruce_stairs", "dark_oak_stairs", "spruce_fence", "dark_oak_trapdoor"],
            &["oak_planks", "spruce_planks", "oak_stairs", "spruce_stairs", "oak_fence", "spruce_trapdoor"],
            &["jungle_planks", "spruce_planks", "jungle_stairs", "spruce_stairs", "jungle_fence", "spruce_trapdoor"],
            &["dark_oak_planks", "spruce_planks", "dark_oak_stairs", "spruce_stairs", "dark_oak_fence", "spruce_trapdoor"],
            &["birch_planks", "oak_planks", "birch_stairs", "oak_stairs", "birch_fence", "oak_trapdoor"],
        ],
        column: &[(0, 0), (1, 1)],
        blocks: &[
            ([4, 8, 0], 2), ([0, 6, 8], 1), ([8, 5, 9], 1), ([5, 1, 23], 3), ([4, 0, 8], 2),
            ([8, 5, 17], 4), ([5, 4, 4], 3), ([0, 5, 15], 4), ([1, 2, 11], 3), ([2, 2, 19], 1),
            ([5, 4, 12], 5), ([6, 1, 19], 3), ([4, 4, 8], 2), ([2, 6, 5], 1), ([5, 4, 16], 0),
            ([6, 1, 11], 3), ([7, 2, 14], 3), ([7, 6, 6], 1), ([8, 5, 13], 4), ([0, 5, 11], 4),
            ([1, 2, 15], 3), ([1, 3, 8], 3),
        ],
    },
    Template {
        size: [9, 9, 24],
        palettes: &[
            &["spruce_planks", "oak_planks", "oak_stairs", "spruce_fence", "oak_trapdoor", "spruce_stairs"],
            &["spruce_planks", "jungle_planks", "jungle_stairs", "spruce_fence", "jungle_trapdoor", "spruce_stairs"],
            &["jungle_planks", "dark_oak_planks", "dark_oak_stairs", "jungle_fence", "dark_oak_trapdoor", "jungle_stairs"],
            &["spruce_planks", "dark_oak_planks", "dark_oak_stairs", "spruce_fence", "dark_oak_trapdoor", "spruce_stairs"],
            &["oak_planks", "spruce_planks", "spruce_stairs", "oak_fence", "spruce_trapdoor", "oak_stairs"],
            &["jungle_planks", "spruce_planks", "spruce_stairs", "jungle_fence", "spruce_trapdoor", "jungle_stairs"],
            &["dark_oak_planks", "spruce_planks", "spruce_stairs", "dark_oak_fence", "spruce_trapdoor", "dark_oak_stairs"],
            &["birch_planks", "oak_planks", "oak_stairs", "birch_fence", "oak_trapdoor", "birch_stairs"],
        ],
        column: &[(0, 0), (1, 1)],
        blocks: &[
            ([3, 4, 4], 2), ([5, 1, 23], 2), ([8, 5, 10], 1), ([0, 5, 11], 3), ([4, 0, 9], 0),
            ([8, 5, 17], 3), ([0, 4, 16], 2), ([2, 1, 19], 2), ([4, 4, 12], 4), ([6, 2, 19], 1),
            ([6, 3, 7], 2), ([1, 2, 13], 2), ([1, 3, 8], 2), ([7, 2, 13], 2), ([5, 4, 16], 0),
            ([7, 6, 7], 1), ([4, 0, 12], 0), ([4, 1, 6], 5), ([6, 2, 10], 1), ([8, 5, 13], 3),
            ([1, 5, 6], 2), ([7, 2, 16], 2),
        ],
    },
    Template {
        size: [9, 9, 28],
        palettes: &[
            &["spruce_planks", "oak_planks", "oak_trapdoor", "spruce_stairs", "spruce_fence", "oak_stairs", "oak_slab", "oak_log"],
            &["spruce_planks", "jungle_planks", "jungle_trapdoor", "spruce_stairs", "spruce_fence", "jungle_stairs", "jungle_slab", "jungle_log"],
            &["jungle_planks", "dark_oak_planks", "dark_oak_trapdoor", "jungle_stairs", "jungle_fence", "dark_oak_stairs", "dark_oak_slab", "dark_oak_log"],
            &["spruce_planks", "dark_oak_planks", "dark_oak_trapdoor", "spruce_stairs", "spruce_fence", "dark_oak_stairs", "dark_oak_slab", "dark_oak_log"],
            &["oak_planks", "spruce_planks", "spruce_trapdoor", "oak_stairs", "oak_fence", "spruce_stairs", "spruce_slab", "spruce_log"],
            &["jungle_planks", "spruce_planks", "spruce_trapdoor", "jungle_stairs", "jungle_fence", "spruce_stairs", "spruce_slab", "spruce_log"],
            &["dark_oak_planks", "spruce_planks", "spruce_trapdoor", "dark_oak_stairs", "dark_oak_fence", "spruce_stairs", "spruce_slab", "spruce_log"],
            &["birch_planks", "oak_planks", "oak_trapdoor", "birch_stairs", "birch_fence", "oak_stairs", "oak_slab", "oak_log"],
        ],
        column: &[(0, 0), (1, 1), (4, 2)],
        blocks: &[
            ([4, 8, 0], 3), ([3, 8, 27], 4), ([4, 1, 6], 3), ([4, 0, 25], 1), ([8, 7, 21], 1),
            ([0, 5, 21], 5), ([0, 6, 9], 5), ([8, 6, 9], 5), ([6, 2, 20], 1), ([6, 5, 25], 1),
            ([1, 2, 11], 5), ([2, 5, 5], 1), ([6, 2, 10], 1), ([4, 7, 19], 6), ([8, 5, 17], 4),
            ([1, 2, 18], 5), ([0, 5, 15], 4), ([1, 5, 25], 5), ([6, 6, 5], 1), ([8, 4, 13], 5),
            ([2, 2, 22], 5), ([4, 5, 11], 7),
        ],
    },
    Template {
        size: [9, 9, 28],
        palettes: &[
            &["spruce_planks", "oak_planks", "oak_trapdoor", "spruce_stairs", "spruce_fence", "oak_stairs", "oak_slab", "oak_log"],
            &["spruce_planks", "jungle_planks", "jungle_trapdoor", "spruce_stairs", "spruce_fence", "jungle_stairs", "jungle_slab", "jungle_log"],
            &["jungle_planks", "dark_oak_planks", "dark_oak_trapdoor", "jungle_stairs", "jungle_fence", "dark_oak_stairs", "dark_oak_slab", "dark_oak_log"],
            &["spruce_planks", "dark_oak_planks", "dark_oak_trapdoor", "spruce_stairs", "spruce_fence", "dark_oak_stairs", "dark_oak_slab", "dark_oak_log"],
            &["oak_planks", "spruce_planks", "spruce_trapdoor", "oak_stairs", "oak_fence", "spruce_stairs", "spruce_slab", "spruce_log"],
            &["jungle_planks", "spruce_planks", "spruce_trapdoor", "jungle_stairs", "jungle_fence", "spruce_stairs", "spruce_slab", "spruce_log"],
            &["dark_oak_planks", "spruce_planks", "spruce_trapdoor", "dark_oak_stairs", "dark_oak_fence", "spruce_stairs", "spruce_slab", "spruce_log"],
            &["birch_planks", "oak_planks", "oak_trapdoor", "birch_stairs", "birch_fence", "oak_stairs", "oak_slab", "oak_log"],
        ],
        column: &[(0, 0), (1, 1), (4, 2)],
        blocks: &[
            ([4, 8, 0], 3), ([3, 8, 27], 4), ([4, 1, 6], 3), ([4, 0, 25], 1), ([8, 7, 21], 1),
            ([0, 5, 21], 5), ([0, 6, 9], 5), ([8, 6, 9], 5), ([6, 2, 20], 1), ([6, 5, 25], 1),
            ([1, 2, 11], 5), ([2, 5, 5], 1), ([6, 2, 10], 1), ([4, 7, 19], 6), ([8, 5, 17], 4),
            ([1, 2, 18], 5), ([0, 5, 15], 4), ([1, 5, 25], 5), ([6, 6, 5], 1), ([2, 2, 22], 5),
            ([4, 5, 11], 7), ([0, 8, 23], 5),
        ],
    },
    Template {
        size: [9, 9, 17],
        palettes: &[
            &["oak_stairs", "oak_planks", "spruce_planks", "spruce_fence", "oak_log"],
            &["jungle_stairs", "jungle_planks", "spruce_planks", "spruce_fence", "jungle_log"],
            &["dark_oak_stairs", "dark_oak_planks", "jungle_planks", "jungle_fence", "dark_oak_log"],
            &["dark_oak_stairs", "dark_oak_planks", "spruce_planks", "spruce_fence", "dark_oak_log"],
            &["spruce_stairs", "spruce_planks", "oak_planks", "oak_fence", "spruce_log"],
            &["spruce_stairs", "spruce_planks", "jungle_planks", "jungle_fence", "spruce_log"],
            &["spruce_stairs", "spruce_planks", "dark_oak_planks", "dark_oak_fence", "spruce_log"],
            &["oak_stairs", "oak_planks", "birch_planks", "birch_fence", "oak_log"],
        ],
        column: &[(4, 0)],
        blocks: &[
            ([4, 0, 0], 0), ([6, 7, 6], 0), ([2, 0, 8], 1), ([0, 8, 10], 0), ([7, 3, 10], 0),
            ([8, 4, 1], 2), ([0, 1, 13], 3), ([6, 1, 5], 0), ([5, 7, 11], 0), ([0, 6, 15], 1),
            ([3, 4, 5], 4), ([1, 4, 10], 2), ([4, 1, 12], 1), ([2, 8, 7], 0), ([8, 4, 5], 2),
            ([1, 3, 16], 0), ([3, 7, 14], 0), ([4, 3, 8], 2), ([0, 1, 10], 1), ([1, 4, 13], 2),
            ([4, 0, 3], 0), ([4, 6, 8], 2),
        ],
    },
    Template {
        size: [9, 9, 17],
        palettes: &[
            &["oak_stairs", "oak_planks", "spruce_fence", "oak_log", "spruce_planks"],
            &["jungle_stairs", "jungle_planks", "spruce_fence", "jungle_log", "spruce_planks"],
            &["dark_oak_stairs", "dark_oak_planks", "jungle_fence", "dark_oak_log", "jungle_planks"],
            &["dark_oak_stairs", "dark_oak_planks", "spruce_fence", "dark_oak_log", "spruce_planks"],
            &["spruce_stairs", "spruce_planks", "oak_fence", "spruce_log", "oak_planks"],
            &["spruce_stairs", "spruce_planks", "jungle_fence", "spruce_log", "jungle_planks"],
            &["spruce_stairs", "spruce_planks", "dark_oak_fence", "spruce_log", "dark_oak_planks"],
            &["oak_stairs", "oak_planks", "birch_fence", "oak_log", "birch_planks"],
        ],
        column: &[(4, 0)],
        blocks: &[
            ([4, 0, 0], 0), ([6, 7, 6], 0), ([2, 0, 8], 1), ([0, 8, 11], 0), ([7, 3, 10], 0),
            ([0, 1, 13], 2), ([7, 2, 4], 0), ([5, 7, 11], 0), ([0, 6, 15], 1), ([3, 4, 5], 3),
            ([1, 4, 10], 4), ([4, 1, 12], 1), ([4, 0, 4], 0), ([7, 2, 0], 0), ([3, 6, 8], 1),
            ([1, 3, 16], 0), ([3, 7, 14], 0), ([6, 1, 7], 0), ([8, 4, 7], 4), ([0, 1, 10], 1),
            ([1, 4, 13], 4), ([3, 3, 8], 1),
        ],
    },
    Template {
        size: [9, 9, 24],
        palettes: &[
            &["oak_stairs", "spruce_stairs", "oak_planks", "spruce_planks", "oak_log", "spruce_fence"],
            &["jungle_stairs", "spruce_stairs", "jungle_planks", "spruce_planks", "jungle_log", "spruce_fence"],
            &["dark_oak_stairs", "jungle_stairs", "dark_oak_planks", "jungle_planks", "dark_oak_log", "jungle_fence"],
            &["dark_oak_stairs", "spruce_stairs", "dark_oak_planks", "spruce_planks", "dark_oak_log", "spruce_fence"],
            &["spruce_stairs", "oak_stairs", "spruce_planks", "oak_planks", "spruce_log", "oak_fence"],
            &["spruce_stairs", "jungle_stairs", "spruce_planks", "jungle_planks", "spruce_log", "jungle_fence"],
            &["spruce_stairs", "dark_oak_stairs", "spruce_planks", "dark_oak_planks", "spruce_log", "dark_oak_fence"],
            &["oak_stairs", "birch_stairs", "oak_planks", "birch_planks", "oak_log", "birch_fence"],
        ],
        column: &[(0, 0)],
        blocks: &[
            ([0, 4, 0], 1), ([2, 8, 9], 0), ([2, 0, 23], 2), ([4, 1, 7], 0), ([8, 4, 20], 3),
            ([7, 6, 13], 0), ([3, 5, 4], 2), ([7, 5, 8], 0), ([3, 4, 11], 4), ([5, 1, 19], 2),
            ([6, 1, 11], 0), ([3, 8, 13], 5), ([7, 3, 16], 2), ([3, 0, 10], 2), ([4, 6, 7], 2),
            ([5, 7, 10], 2), ([6, 2, 22], 0), ([6, 4, 5], 1), ([1, 3, 3], 5), ([6, 4, 11], 4),
            ([7, 6, 16], 0), ([4, 2, 13], 3),
        ],
    },
    Template {
        size: [9, 9, 24],
        palettes: &[
            &["oak_stairs", "spruce_stairs", "oak_planks", "spruce_planks", "oak_log", "spruce_fence"],
            &["jungle_stairs", "spruce_stairs", "jungle_planks", "spruce_planks", "jungle_log", "spruce_fence"],
            &["dark_oak_stairs", "jungle_stairs", "dark_oak_planks", "jungle_planks", "dark_oak_log", "jungle_fence"],
            &["dark_oak_stairs", "spruce_stairs", "dark_oak_planks", "spruce_planks", "dark_oak_log", "spruce_fence"],
            &["spruce_stairs", "oak_stairs", "spruce_planks", "oak_planks", "spruce_log", "oak_fence"],
            &["spruce_stairs", "jungle_stairs", "spruce_planks", "jungle_planks", "spruce_log", "jungle_fence"],
            &["spruce_stairs", "dark_oak_stairs", "spruce_planks", "dark_oak_planks", "spruce_log", "dark_oak_fence"],
            &["oak_stairs", "birch_stairs", "oak_planks", "birch_planks", "oak_log", "birch_fence"],
        ],
        column: &[(0, 0)],
        blocks: &[
            ([0, 4, 0], 1), ([5, 7, 8], 0), ([2, 0, 23], 2), ([2, 0, 8], 2), ([7, 3, 20], 2),
            ([8, 4, 13], 3), ([4, 3, 4], 0), ([3, 4, 11], 4), ([7, 2, 9], 0), ([3, 0, 19], 2),
            ([2, 6, 5], 2), ([4, 0, 11], 0), ([7, 4, 6], 1), ([4, 3, 8], 3), ([5, 1, 22], 0),
            ([1, 3, 3], 5), ([7, 5, 10], 2), ([5, 2, 13], 2), ([6, 1, 17], 0), ([2, 2, 5], 2),
            ([2, 5, 2], 5), ([3, 4, 6], 3),
        ],
    },
    Template {
        size: [9, 9, 28],
        palettes: &[
            &["oak_stairs", "spruce_planks", "spruce_stairs", "oak_trapdoor", "spruce_fence", "oak_planks", "oak_log"],
            &["jungle_stairs", "spruce_planks", "spruce_stairs", "jungle_trapdoor", "spruce_fence", "jungle_planks", "jungle_log"],
            &["dark_oak_stairs", "jungle_planks", "jungle_stairs", "dark_oak_trapdoor", "jungle_fence", "dark_oak_planks", "dark_oak_log"],
            &["dark_oak_stairs", "spruce_planks", "spruce_stairs", "dark_oak_trapdoor", "spruce_fence", "dark_oak_planks", "dark_oak_log"],
            &["spruce_stairs", "oak_planks", "oak_stairs", "spruce_trapdoor", "oak_fence", "spruce_planks", "spruce_log"],
            &["spruce_stairs", "jungle_planks", "jungle_stairs", "spruce_trapdoor", "jungle_fence", "spruce_planks", "spruce_log"],
            &["spruce_stairs", "dark_oak_planks", "dark_oak_stairs", "spruce_trapdoor", "dark_oak_fence", "spruce_planks", "spruce_log"],
            &["oak_stairs", "birch_planks", "birch_stairs", "oak_trapdoor", "birch_fence", "oak_planks", "oak_log"],
        ],
        column: &[(0, 0), (1, 1), (2, 2), (3, 3), (4, 3), (5, 3), (6, 2), (7, 1), (8, 0)],
        blocks: &[
            ([0, 4, 0], 2), ([0, 3, 27], 4), ([7, 4, 6], 2), ([7, 5, 23], 0), ([0, 7, 21], 5),
            ([0, 0, 21], 0), ([2, 0, 8], 5), ([2, 8, 8], 5), ([4, 1, 24], 5), ([8, 4, 11], 1),
            ([7, 2, 19], 0), ([3, 7, 25], 0), ([2, 3, 4], 5), ([6, 7, 19], 0), ([3, 3, 19], 5),
            ([3, 4, 11], 6), ([6, 1, 9], 0), ([6, 7, 9], 0), ([8, 4, 15], 1), ([1, 3, 23], 1),
            ([4, 6, 5], 0), ([2, 8, 18], 0),
        ],
    },
    Template {
        size: [9, 9, 28],
        palettes: &[
            &["oak_stairs", "spruce_planks", "spruce_stairs", "oak_trapdoor", "spruce_fence", "oak_planks", "oak_log"],
            &["jungle_stairs", "spruce_planks", "spruce_stairs", "jungle_trapdoor", "spruce_fence", "jungle_planks", "jungle_log"],
            &["dark_oak_stairs", "jungle_planks", "jungle_stairs", "dark_oak_trapdoor", "jungle_fence", "dark_oak_planks", "dark_oak_log"],
            &["dark_oak_stairs", "spruce_planks", "spruce_stairs", "dark_oak_trapdoor", "spruce_fence", "dark_oak_planks", "dark_oak_log"],
            &["spruce_stairs", "oak_planks", "oak_stairs", "spruce_trapdoor", "oak_fence", "spruce_planks", "spruce_log"],
            &["spruce_stairs", "jungle_planks", "jungle_stairs", "spruce_trapdoor", "jungle_fence", "spruce_planks", "spruce_log"],
            &["spruce_stairs", "dark_oak_planks", "dark_oak_stairs", "spruce_trapdoor", "dark_oak_fence", "spruce_planks", "spruce_log"],
            &["oak_stairs", "birch_planks", "birch_stairs", "oak_trapdoor", "birch_fence", "oak_planks", "oak_log"],
        ],
        column: &[(0, 0), (1, 1), (2, 2), (3, 3), (4, 3)],
        blocks: &[
            ([0, 4, 0], 2), ([0, 5, 27], 4), ([7, 4, 6], 2), ([7, 3, 23], 0), ([0, 0, 21], 0),
            ([1, 8, 20], 0), ([2, 8, 9], 0), ([2, 0, 8], 5), ([7, 6, 11], 0), ([6, 7, 19], 0),
            ([4, 7, 24], 5), ([3, 1, 25], 0), ([2, 3, 4], 5), ([6, 1, 10], 0), ([6, 1, 19], 0),
            ([1, 4, 22], 1), ([3, 4, 11], 6), ([8, 4, 16], 1), ([3, 5, 7], 2), ([0, 7, 24], 4),
            ([2, 0, 18], 0), ([3, 5, 19], 5),
        ],
    },
    Template {
        size: [9, 9, 16],
        palettes: &[
            &["spruce_fence", "oak_planks", "oak_stairs", "oak_log", "spruce_slab", "spruce_stairs", "spruce_planks"],
            &["spruce_fence", "jungle_planks", "jungle_stairs", "jungle_log", "spruce_slab", "spruce_stairs", "spruce_planks"],
            &["jungle_fence", "dark_oak_planks", "dark_oak_stairs", "dark_oak_log", "jungle_slab", "jungle_stairs", "jungle_planks"],
            &["spruce_fence", "dark_oak_planks", "dark_oak_stairs", "dark_oak_log", "spruce_slab", "spruce_stairs", "spruce_planks"],
            &["oak_fence", "spruce_planks", "spruce_stairs", "spruce_log", "oak_slab", "oak_stairs", "oak_planks"],
            &["jungle_fence", "spruce_planks", "spruce_stairs", "spruce_log", "jungle_slab", "jungle_stairs", "jungle_planks"],
            &["dark_oak_fence", "spruce_planks", "spruce_stairs", "spruce_log", "dark_oak_slab", "dark_oak_stairs", "dark_oak_planks"],
            &["birch_fence", "oak_planks", "oak_stairs", "oak_log", "birch_slab", "birch_stairs", "birch_planks"],
        ],
        column: &[(0, 0), (1, 1)],
        blocks: &[
            ([6, 7, 0], 2), ([0, 2, 6], 2), ([8, 4, 8], 2), ([2, 6, 11], 2), ([3, 7, 5], 1),
            ([0, 0, 11], 2), ([5, 0, 9], 1), ([8, 4, 3], 2), ([4, 3, 4], 3), ([6, 5, 12], 2),
            ([1, 3, 13], 2), ([4, 5, 8], 1), ([7, 1, 12], 1), ([2, 2, 9], 4), ([2, 6, 2], 1),
            ([1, 5, 7], 1), ([4, 8, 9], 5), ([6, 6, 5], 1), ([8, 1, 9], 1), ([1, 4, 4], 6),
            ([3, 1, 12], 6), ([4, 4, 14], 2),
        ],
    },
    Template {
        size: [9, 9, 16],
        palettes: &[
            &["spruce_fence", "oak_planks", "oak_stairs", "oak_log", "spruce_slab", "spruce_planks"],
            &["spruce_fence", "jungle_planks", "jungle_stairs", "jungle_log", "spruce_slab", "spruce_planks"],
            &["jungle_fence", "dark_oak_planks", "dark_oak_stairs", "dark_oak_log", "jungle_slab", "jungle_planks"],
            &["spruce_fence", "dark_oak_planks", "dark_oak_stairs", "dark_oak_log", "spruce_slab", "spruce_planks"],
            &["oak_fence", "spruce_planks", "spruce_stairs", "spruce_log", "oak_slab", "oak_planks"],
            &["jungle_fence", "spruce_planks", "spruce_stairs", "spruce_log", "jungle_slab", "jungle_planks"],
            &["dark_oak_fence", "spruce_planks", "spruce_stairs", "spruce_log", "dark_oak_slab", "dark_oak_planks"],
            &["birch_fence", "oak_planks", "oak_stairs", "oak_log", "birch_slab", "birch_planks"],
        ],
        column: &[(0, 0), (1, 1)],
        blocks: &[
            ([6, 7, 0], 2), ([0, 2, 6], 2), ([8, 4, 8], 2), ([2, 6, 11], 2), ([3, 7, 5], 1),
            ([0, 0, 11], 2), ([5, 0, 9], 1), ([8, 4, 3], 2), ([4, 3, 4], 3), ([6, 5, 12], 2),
            ([1, 3, 13], 2), ([4, 5, 8], 1), ([7, 1, 12], 1), ([2, 2, 9], 4), ([2, 6, 2], 1),
            ([1, 5, 7], 1), ([6, 6, 5], 1), ([8, 1, 9], 1), ([1, 4, 4], 5), ([3, 1, 12], 5),
            ([4, 4, 14], 2), ([6, 2, 7], 1),
        ],
    },
    Template {
        size: [9, 9, 22],
        palettes: &[
            &["oak_trapdoor", "oak_planks", "spruce_planks", "spruce_stairs", "oak_stairs", "oak_log", "spruce_fence"],
            &["jungle_trapdoor", "jungle_planks", "spruce_planks", "spruce_stairs", "jungle_stairs", "jungle_log", "spruce_fence"],
            &["dark_oak_trapdoor", "dark_oak_planks", "jungle_planks", "jungle_stairs", "dark_oak_stairs", "dark_oak_log", "jungle_fence"],
            &["dark_oak_trapdoor", "dark_oak_planks", "spruce_planks", "spruce_stairs", "dark_oak_stairs", "dark_oak_log", "spruce_fence"],
            &["spruce_trapdoor", "spruce_planks", "oak_planks", "oak_stairs", "spruce_stairs", "spruce_log", "oak_fence"],
            &["spruce_trapdoor", "spruce_planks", "jungle_planks", "jungle_stairs", "spruce_stairs", "spruce_log", "jungle_fence"],
            &["spruce_trapdoor", "spruce_planks", "dark_oak_planks", "dark_oak_stairs", "spruce_stairs", "spruce_log", "dark_oak_fence"],
            &["oak_trapdoor", "oak_planks", "birch_planks", "birch_stairs", "oak_stairs", "oak_log", "birch_fence"],
        ],
        column: &[(4, 0), (7, 1), (8, 2)],
        blocks: &[
            ([4, 0, 0], 3), ([4, 7, 6], 3), ([0, 2, 9], 4), ([8, 2, 9], 4), ([4, 8, 21], 3),
            ([1, 6, 11], 4), ([2, 3, 5], 1), ([6, 6, 10], 1), ([7, 6, 18], 4), ([6, 2, 5], 1),
            ([8, 4, 13], 4), ([0, 4, 15], 4), ([4, 3, 11], 5), ([2, 7, 18], 4), ([2, 5, 8], 1),
            ([3, 1, 3], 6), ([4, 4, 3], 3), ([4, 4, 18], 3), ([4, 7, 12], 1), ([4, 8, 9], 2),
            ([5, 4, 7], 1), ([7, 6, 15], 4),
        ],
    },
    Template {
        size: [9, 9, 22],
        palettes: &[
            &["oak_trapdoor", "oak_planks", "spruce_planks", "spruce_stairs", "oak_stairs", "oak_log", "spruce_fence"],
            &["jungle_trapdoor", "jungle_planks", "spruce_planks", "spruce_stairs", "jungle_stairs", "jungle_log", "spruce_fence"],
            &["dark_oak_trapdoor", "dark_oak_planks", "jungle_planks", "jungle_stairs", "dark_oak_stairs", "dark_oak_log", "jungle_fence"],
            &["dark_oak_trapdoor", "dark_oak_planks", "spruce_planks", "spruce_stairs", "dark_oak_stairs", "dark_oak_log", "spruce_fence"],
            &["spruce_trapdoor", "spruce_planks", "oak_planks", "oak_stairs", "spruce_stairs", "spruce_log", "oak_fence"],
            &["spruce_trapdoor", "spruce_planks", "jungle_planks", "jungle_stairs", "spruce_stairs", "spruce_log", "jungle_fence"],
            &["spruce_trapdoor", "spruce_planks", "dark_oak_planks", "dark_oak_stairs", "spruce_stairs", "spruce_log", "dark_oak_fence"],
            &["oak_trapdoor", "oak_planks", "birch_planks", "birch_stairs", "oak_stairs", "oak_log", "birch_fence"],
        ],
        column: &[(4, 0), (7, 1), (8, 2)],
        blocks: &[
            ([4, 0, 0], 3), ([4, 7, 6], 3), ([0, 2, 9], 4), ([8, 2, 9], 4), ([4, 8, 21], 3),
            ([1, 6, 11], 4), ([2, 3, 5], 1), ([6, 6, 10], 1), ([7, 6, 18], 4), ([6, 2, 5], 1),
            ([8, 4, 13], 4), ([4, 3, 11], 5), ([2, 7, 18], 4), ([2, 5, 8], 1), ([0, 4, 13], 4),
            ([3, 1, 3], 6), ([4, 4, 3], 3), ([4, 4, 18], 3), ([4, 8, 9], 2), ([5, 4, 7], 1),
            ([7, 6, 15], 4), ([5, 2, 2], 6),
        ],
    },
    Template {
        size: [9, 9, 28],
        palettes: &[
            &["oak_trapdoor", "spruce_stairs", "spruce_fence", "oak_stairs", "oak_planks", "spruce_planks", "oak_log"],
            &["jungle_trapdoor", "spruce_stairs", "spruce_fence", "jungle_stairs", "jungle_planks", "spruce_planks", "jungle_log"],
            &["dark_oak_trapdoor", "jungle_stairs", "jungle_fence", "dark_oak_stairs", "dark_oak_planks", "jungle_planks", "dark_oak_log"],
            &["dark_oak_trapdoor", "spruce_stairs", "spruce_fence", "dark_oak_stairs", "dark_oak_planks", "spruce_planks", "dark_oak_log"],
            &["spruce_trapdoor", "oak_stairs", "oak_fence", "spruce_stairs", "spruce_planks", "oak_planks", "spruce_log"],
            &["spruce_trapdoor", "jungle_stairs", "jungle_fence", "spruce_stairs", "spruce_planks", "jungle_planks", "spruce_log"],
            &["spruce_trapdoor", "dark_oak_stairs", "dark_oak_fence", "spruce_stairs", "spruce_planks", "dark_oak_planks", "spruce_log"],
            &["oak_trapdoor", "birch_stairs", "birch_fence", "oak_stairs", "oak_planks", "birch_planks", "oak_log"],
        ],
        column: &[(4, 0)],
        blocks: &[
            ([4, 0, 0], 1), ([3, 0, 27], 2), ([4, 7, 6], 1), ([5, 7, 23], 3), ([8, 0, 21], 3),
            ([0, 1, 20], 3), ([0, 2, 9], 3), ([8, 2, 9], 3), ([2, 7, 11], 3), ([1, 6, 19], 3),
            ([1, 4, 24], 3), ([8, 4, 18], 3), ([7, 3, 25], 3), ([2, 3, 5], 4), ([7, 6, 11], 3),
            ([0, 3, 16], 2), ([4, 1, 22], 5), ([5, 7, 18], 4), ([4, 3, 11], 6), ([6, 2, 5], 4),
            ([8, 3, 13], 2), ([0, 4, 12], 3),
        ],
    },
    Template {
        size: [9, 9, 28],
        palettes: &[
            &["oak_trapdoor", "spruce_stairs", "spruce_fence", "oak_stairs", "oak_planks", "spruce_planks", "oak_log"],
            &["jungle_trapdoor", "spruce_stairs", "spruce_fence", "jungle_stairs", "jungle_planks", "spruce_planks", "jungle_log"],
            &["dark_oak_trapdoor", "jungle_stairs", "jungle_fence", "dark_oak_stairs", "dark_oak_planks", "jungle_planks", "dark_oak_log"],
            &["dark_oak_trapdoor", "spruce_stairs", "spruce_fence", "dark_oak_stairs", "dark_oak_planks", "spruce_planks", "dark_oak_log"],
            &["spruce_trapdoor", "oak_stairs", "oak_fence", "spruce_stairs", "spruce_planks", "oak_planks", "spruce_log"],
            &["spruce_trapdoor", "jungle_stairs", "jungle_fence", "spruce_stairs", "spruce_planks", "jungle_planks", "spruce_log"],
            &["spruce_trapdoor", "dark_oak_stairs", "dark_oak_fence", "spruce_stairs", "spruce_planks", "dark_oak_planks", "spruce_log"],
            &["oak_trapdoor", "birch_stairs", "birch_fence", "oak_stairs", "oak_planks", "birch_planks", "oak_log"],
        ],
        column: &[(4, 0)],
        blocks: &[
            ([4, 0, 0], 1), ([3, 0, 27], 2), ([4, 7, 6], 1), ([5, 7, 23], 3), ([8, 0, 21], 3),
            ([0, 1, 20], 3), ([0, 2, 9], 3), ([8, 4, 11], 3), ([2, 7, 11], 3), ([1, 6, 19], 3),
            ([1, 4, 24], 3), ([7, 3, 6], 3), ([8, 4, 18], 3), ([7, 3, 25], 3), ([2, 3, 5], 4),
            ([0, 3, 16], 2), ([4, 1, 22], 5), ([4, 3, 11], 6), ([5, 2, 3], 3), ([6, 7, 10], 3),
            ([0, 4, 12], 3), ([1, 0, 24], 2),
        ],
    },
    Template {
        size: [9, 21, 28],
        palettes: &[
            &["spruce_planks", "oak_trapdoor", "oak_log", "spruce_stairs", "spruce_fence", "oak_planks", "oak_stairs"],
            &["spruce_planks", "jungle_trapdoor", "jungle_log", "spruce_stairs", "spruce_fence", "jungle_planks", "jungle_stairs"],
            &["jungle_planks", "dark_oak_trapdoor", "dark_oak_log", "jungle_stairs", "jungle_fence", "dark_oak_planks", "dark_oak_stairs"],
            &["spruce_planks", "dark_oak_trapdoor", "dark_oak_log", "spruce_stairs", "spruce_fence", "dark_oak_planks", "dark_oak_stairs"],
            &["oak_planks", "spruce_trapdoor", "spruce_log", "oak_stairs", "oak_fence", "spruce_planks", "spruce_stairs"],
            &["jungle_planks", "spruce_trapdoor", "spruce_log", "jungle_stairs", "jungle_fence", "spruce_planks", "spruce_stairs"],
            &["dark_oak_planks", "spruce_trapdoor", "spruce_log", "dark_oak_stairs", "dark_oak_fence", "spruce_planks", "spruce_stairs"],
            &["birch_planks", "oak_trapdoor", "oak_log", "birch_stairs", "birch_fence", "oak_planks", "oak_stairs"],
        ],
        column: &[(0, 0), (1, 0), (4, 1), (14, 2)],
        blocks: &[
            ([4, 8, 0], 3), ([3, 8, 27], 4), ([4, 1, 6], 3), ([7, 14, 6], 2), ([4, 0, 25], 0),
            ([8, 7, 21], 5), ([0, 5, 21], 6), ([4, 20, 11], 2), ([0, 6, 9], 6), ([8, 6, 9], 6),
            ([3, 10, 6], 2), ([6, 2, 20], 5), ([4, 9, 11], 2), ([4, 9, 16], 2), ([6, 5, 25], 5),
            ([1, 2, 11], 6), ([2, 5, 5], 5), ([6, 2, 10], 5), ([8, 5, 17], 4), ([1, 2, 18], 6),
            ([4, 17, 7], 2), ([3, 8, 21], 5),
        ],
    },
    Template {
        size: [9, 21, 28],
        palettes: &[
            &["spruce_planks", "oak_trapdoor", "spruce_stairs", "spruce_fence", "oak_log", "oak_planks", "oak_stairs", "oak_slab"],
            &["spruce_planks", "jungle_trapdoor", "spruce_stairs", "spruce_fence", "jungle_log", "jungle_planks", "jungle_stairs", "jungle_slab"],
            &["jungle_planks", "dark_oak_trapdoor", "jungle_stairs", "jungle_fence", "dark_oak_log", "dark_oak_planks", "dark_oak_stairs", "dark_oak_slab"],
            &["spruce_planks", "dark_oak_trapdoor", "spruce_stairs", "spruce_fence", "dark_oak_log", "dark_oak_planks", "dark_oak_stairs", "dark_oak_slab"],
            &["oak_planks", "spruce_trapdoor", "oak_stairs", "oak_fence", "spruce_log", "spruce_planks", "spruce_stairs", "spruce_slab"],
            &["jungle_planks", "spruce_trapdoor", "jungle_stairs", "jungle_fence", "spruce_log", "spruce_planks", "spruce_stairs", "spruce_slab"],
            &["dark_oak_planks", "spruce_trapdoor", "dark_oak_stairs", "dark_oak_fence", "spruce_log", "spruce_planks", "spruce_stairs", "spruce_slab"],
            &["birch_planks", "oak_trapdoor", "birch_stairs", "birch_fence", "oak_log", "oak_planks", "oak_stairs", "oak_slab"],
        ],
        column: &[(0, 0), (1, 0), (4, 1)],
        blocks: &[
            ([4, 8, 0], 2), ([3, 8, 27], 3), ([4, 1, 6], 2), ([4, 12, 11], 4), ([4, 0, 25], 0),
            ([8, 7, 21], 5), ([0, 5, 21], 6), ([8, 6, 9], 6), ([1, 6, 8], 6), ([6, 2, 20], 5),
            ([1, 2, 11], 6), ([6, 5, 25], 5), ([3, 5, 4], 5), ([6, 2, 10], 5), ([4, 7, 19], 7),
            ([8, 5, 17], 3), ([1, 2, 18], 6), ([4, 7, 11], 4), ([4, 9, 7], 4), ([0, 5, 15], 3),
            ([1, 5, 25], 6), ([2, 2, 22], 6),
        ],
    },
];
