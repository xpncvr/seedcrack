use crate::placement::StructureSet;
use crate::world::{Cuboid, Found, Observation, StructureObservation, World};

const HOLDERS: [&str; 10] = [
    "sandstone",
    "stone",
    "andesite",
    "granite",
    "diorite",
    "coal_ore",
    "iron_ore",
    "gold_ore",
    "copper_ore",
    "gravel",
];

const PIECE_Y: i32 = 90;

const BIOME_SAMPLE_YS: [i32; 4] = [60, 63, 66, 69];

fn is_beach(biome: &str) -> bool {
    matches!(biome, "beach" | "snowy_beach")
}

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    let x = (chunk_x << 4) + 9;
    let z = (chunk_z << 4) + 9;

    let mut known = true;
    let mut beach = false;
    for y in BIOME_SAMPLE_YS {
        let b = world.biome(x, y, z);
        if b.is_empty() {
            known = false;
        } else if is_beach(b) {
            beach = true;
        }
    }
    if known && !beach {
        return;
    }

    let top = (world.max_y() - 1).min(PIECE_Y);
    let mut hit = None;
    let mut count = 0;
    for y in (world.min_y() + 1)..=top {
        if world.block(x, y, z) == "chest" && HOLDERS.contains(&world.block(x, y - 1, z)) && walked_over(world, x, y, z, top) {
            count += 1;
            hit = Some(y);
        }
    }
    let (1, Some(y)) = (count, hit) else { return };

    out.push(Found {
        observation: Observation::Structure(StructureObservation {
            set: StructureSet::BuriedTreasures,
            chunk_x,
            chunk_z,
        }),
        boxes: vec![Cuboid::block(x, y, z)],
        color: 0,
    });
}

fn walked_over(world: &dyn World, x: i32, y: i32, z: i32, top: i32) -> bool {
    for above in y + 1..=top {
        let b = world.block(x, above, z);
        if b.is_empty() || HOLDERS.contains(&b) {
            return false;
        }
        if matches!(b, "air" | "cave_air" | "water" | "lava") {
            return true;
        }
    }
    false
}
