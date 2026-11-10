use super::scattered::biome_allows;
use crate::placement::StructureSet;
use crate::world::{Cuboid, Found, Observation, StructureObservation, World};

const SNOW: &str = "snow_block";
const WHITE: &str = "white_carpet";
const GRAY: &str = "light_gray_carpet";

const BLOCKS: &[([i32; 3], &str)] = &[
    ([1, 1, 5], "crafting_table"),
    ([1, 1, 3], "furnace"),
    ([2, 1, 3], WHITE),
    ([3, 1, 3], WHITE),
    ([4, 1, 3], WHITE),
    ([2, 1, 4], WHITE),
    ([3, 1, 4], WHITE),
    ([4, 1, 4], WHITE),
    ([2, 1, 5], WHITE),
    ([3, 1, 5], WHITE),
    ([4, 1, 5], WHITE),
    ([2, 1, 6], GRAY),
    ([3, 1, 6], GRAY),
    ([4, 1, 6], GRAY),
    ([2, 1, 0], SNOW),
    ([4, 1, 0], SNOW),
    ([2, 1, 1], SNOW),
    ([4, 1, 1], SNOW),
    ([1, 1, 2], SNOW),
    ([5, 1, 2], SNOW),
    ([0, 1, 3], SNOW),
    ([6, 1, 3], SNOW),
    ([0, 1, 5], SNOW),
    ([6, 1, 5], SNOW),
    ([1, 1, 6], SNOW),
    ([5, 1, 6], SNOW),
    ([2, 1, 7], SNOW),
    ([3, 1, 7], SNOW),
    ([4, 1, 7], SNOW),
    ([2, 2, 0], SNOW),
    ([4, 2, 0], SNOW),
    ([2, 2, 1], SNOW),
    ([4, 2, 1], SNOW),
    ([1, 2, 2], SNOW),
    ([5, 2, 2], SNOW),
    ([0, 2, 3], SNOW),
    ([6, 2, 3], SNOW),
    ([0, 2, 4], SNOW),
    ([6, 2, 4], SNOW),
    ([0, 2, 5], SNOW),
    ([6, 2, 5], SNOW),
    ([1, 2, 6], SNOW),
    ([5, 2, 6], SNOW),
    ([2, 2, 7], SNOW),
    ([3, 2, 7], SNOW),
    ([4, 2, 7], SNOW),
    ([2, 4, 3], SNOW),
    ([3, 4, 3], SNOW),
    ([4, 4, 3], SNOW),
    ([2, 4, 4], SNOW),
    ([3, 4, 4], SNOW),
    ([4, 4, 4], SNOW),
    ([2, 4, 5], SNOW),
    ([3, 4, 5], SNOW),
    ([4, 4, 5], SNOW),
];

const SIZE: [i32; 3] = [7, 5, 8];

fn rotate(rotation: usize, x: i32, z: i32) -> (i32, i32) {
    const PX: i32 = 3;
    const PZ: i32 = 5;
    match rotation {
        0 => (x, z),
        1 => (PX + PZ - z, PZ - PX + x),
        2 => (PX + PX - x, PZ + PZ - z),
        _ => (PX - PZ + z, PX + PZ - x),
    }
}

fn matches(world: &dyn World, rotation: usize, x0: i32, y0: i32, z0: i32) -> bool {
    BLOCKS.iter().all(|&([x, y, z], want)| {
        let (dx, dz) = rotate(rotation, x, z);
        world.block(x0 + dx, y0 + y, z0 + dz) == want
    })
}

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    if !biome_allows(world, chunk_x, chunk_z, &["snowy_taiga", "snowy_plains", "snowy_slopes"]) {
        return;
    }
    let (x0, z0) = (chunk_x << 4, chunk_z << 4);
    for rotation in 0..4 {
        let Some(y0) = (world.min_y()..world.max_y() - SIZE[1]).find(|&y| matches(world, rotation, x0, y, z0)) else {
            continue;
        };
        let (ax, az) = rotate(rotation, 0, 0);
        let (bx, bz) = rotate(rotation, SIZE[0] - 1, SIZE[2] - 1);
        let min = [x0 + ax.min(bx), y0, z0 + az.min(bz)];
        let max = [x0 + ax.max(bx) + 1, y0 + SIZE[1], z0 + az.max(bz) + 1];
        out.push(Found {
            observation: Observation::Structure(StructureObservation { set: StructureSet::Igloos, chunk_x, chunk_z }),
            boxes: vec![Cuboid::new(min, max), Cuboid::sized([x0, y0, z0], [16, 1, 16])],
            color: 0,
        });
        return;
    }
}
