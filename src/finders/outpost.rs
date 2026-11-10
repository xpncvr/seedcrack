use super::template::{Rotation, template_matches};
use crate::placement::StructureSet;
use crate::world::{Cuboid, Found, Observation, StructureObservation, World};

const TOWER_SIZE: [i32; 3] = [15, 21, 15];

const PLATE_SIZE: [i32; 3] = [16, 30, 16];

const TOWER_SHIFTS: [i32; 2] = [0, 1];

const BIOMES: [&str; 12] = [
    "desert",
    "plains",
    "savanna",
    "snowy_plains",
    "taiga",
    "meadow",
    "frozen_peaks",
    "jagged_peaks",
    "stony_peaks",
    "snowy_slopes",
    "cherry_grove",
    "grove",
];

const TOWER: [([i32; 3], &str); 24] = [
    ([9, 14, 10], "chest"),
    ([4, 0, 1], "dark_oak_log"),
    ([10, 0, 13], "dark_oak_log"),
    ([2, 17, 2], "dark_oak_planks"),
    ([3, 6, 9], "dark_oak_planks"),
    ([13, 2, 4], "dark_oak_log"),
    ([11, 13, 1], "dark_oak_log"),
    ([2, 19, 10], "dark_oak_planks"),
    ([11, 20, 5], "dark_oak_planks"),
    ([11, 7, 9], "dark_oak_planks"),
    ([4, 10, 3], "dark_oak_log"),
    ([3, 13, 13], "dark_oak_log"),
    ([7, 0, 7], "birch_planks"),
    ([1, 0, 10], "dark_oak_log"),
    ([8, 20, 11], "dark_oak_planks"),
    ([8, 5, 3], "birch_planks"),
    ([1, 13, 7], "dark_oak_log"),
    ([13, 13, 6], "dark_oak_log"),
    ([5, 20, 5], "dark_oak_planks"),
    ([3, 5, 4], "dark_oak_log"),
    ([8, 13, 5], "birch_planks"),
    ([7, 4, 11], "dark_oak_planks"),
    ([7, 9, 11], "dark_oak_log"),
    ([11, 9, 4], "dark_oak_log"),
];

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    let (x0, z0) = (chunk_x << 4, chunk_z << 4);
    let (min_y, max_y) = (world.min_y(), world.max_y());
    let (chest, chest_block) = TOWER[0];
    for rotation in Rotation::ALL {
        let c = rotation.transform(chest, [0, 0]);
        for shift in TOWER_SHIFTS {
            let s = rotation.transform([shift, 0, 0], [0, 0]);
            for y0 in min_y..=max_y - TOWER_SIZE[1] {
                if world.block(x0 + s[0] + c[0], y0 + c[1], z0 + s[2] + c[2]) != chest_block {
                    continue;
                }
                let tower = [x0 + s[0], y0, z0 + s[2]];
                if !template_matches(world, tower, rotation, [0, 0], &TOWER[1..]) {
                    continue;
                }
                let plate = rotation.template_box([x0, y0, z0], PLATE_SIZE, [0, 0]);
                let mid_x = (plate.min[0] + plate.max[0] - 1) / 2;
                let mid_z = (plate.min[2] + plate.max[2] - 1) / 2;
                let biome = world.biome(mid_x, y0 + 1, mid_z);
                if !biome.is_empty() && !BIOMES.contains(&biome) {
                    return;
                }
                out.push(Found {
                    observation: Observation::Structure(StructureObservation {
                        set: StructureSet::PillagerOutposts,
                        chunk_x,
                        chunk_z,
                    }),
                    boxes: vec![rotation.template_box(tower, TOWER_SIZE, [0, 0]), Cuboid::block(x0, y0, z0)],
                    color: 0,
                });
                return;
            }
        }
    }
}
