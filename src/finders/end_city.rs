use super::template::{Rotation, template_matches};
use crate::placement::StructureSet;
use crate::world::{Cuboid, Found, Observation, StructureObservation, World};

const START_OFFSET: i32 = 7;

const MIN_START_Y: i32 = 60;

const TOP: i32 = 8;

const BIOMES: [&str; 2] = ["end_highlands", "end_midlands"];

const START_HOUSE: [([i32; 3], &str); 28] = [
    ([1, 5, 8], "purpur_slab"),
    ([7, 1, 2], "purpur_slab"),
    ([9, 7, 7], "magenta_stained_glass"),
    ([3, 4, 2], "purpur_slab"),
    ([5, 6, 8], "purpur_slab"),
    ([6, 3, 1], "end_stone_bricks"),
    ([7, 8, 8], "purpur_slab"),
    ([3, 2, 1], "end_stone_bricks"),
    ([3, 5, 8], "purpur_slab"),
    ([7, 1, 4], "purpur_slab"),
    ([5, 2, 2], "purpur_slab"),
    ([4, 3, 2], "purpur_block"),
    ([11, 8, -2], "purpur_stairs"),
    ([-2, 8, -2], "purpur_stairs"),
    ([5, 8, -2], "purpur_stairs"),
    ([-2, 8, 4], "purpur_stairs"),
    ([5, 0, 9], "purpur_block"),
    ([-2, 8, 11], "purpur_stairs"),
    ([6, 8, 3], "purpur_block"),
    ([1, 0, 6], "purpur_block"),
    ([11, 8, 3], "purpur_stairs"),
    ([11, 8, 11], "purpur_stairs"),
    ([-1, 4, 0], "purpur_stairs"),
    ([1, 8, 1], "purpur_block"),
    ([8, 3, 8], "purpur_pillar"),
    ([10, 4, -1], "purpur_stairs"),
    ([10, 4, 4], "purpur_stairs"),
    ([2, 8, 5], "purpur_block"),
];

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    let (sx, sz) = ((chunk_x << 4) + START_OFFSET, (chunk_z << 4) + START_OFFSET);
    let biome = world.biome(sx, 64, sz);
    if !biome.is_empty() && !BIOMES.contains(&biome) {
        return;
    }
    let low = MIN_START_Y.max(world.min_y());
    let high = world.max_y() - 1 - TOP;
    let (first, first_block) = START_HOUSE[0];
    for rotation in Rotation::ALL {
        let f = rotation.transform(first, [0, 0]);
        for y0 in low..=high {
            if world.block(sx + f[0], y0 + f[1], sz + f[2]) != first_block {
                continue;
            }
            let origin = [sx, y0, sz];
            if !template_matches(world, origin, rotation, [0, 0], &START_HOUSE[1..]) {
                continue;
            }
            out.push(Found {
                observation: Observation::Structure(StructureObservation {
                    set: StructureSet::EndCities,
                    chunk_x,
                    chunk_z,
                }),
                boxes: vec![
                    rotation.bounds(origin, [-3, 0, -3], [12, 13, 12], [0, 0]),
                    Cuboid::block(chunk_x << 4, y0, chunk_z << 4),
                ],
                color: 0,
            });
            return;
        }
    }
}
