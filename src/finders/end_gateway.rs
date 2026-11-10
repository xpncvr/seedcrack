use super::piece::is_air;
use crate::decorator::{DecoratorKind, DecoratorObservation};
use crate::world::{Cuboid, Found, Observation, World};

const MAIN_ISLAND_RADIUS: i32 = 128;

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    let (bx, bz) = (chunk_x << 4, chunk_z << 4);
    let (min_y, max_y) = (world.min_y(), world.max_y());
    for x in bx..bx + 16 {
        for z in bz..bz + 16 {
            if x * x + z * z <= MAIN_ISLAND_RADIUS * MAIN_ISLAND_RADIUS {
                continue;
            }
            for y in (min_y + 3..max_y - 1).rev() {
                if world.block(x, y, z) != "end_gateway" {
                    continue;
                }
                if let Some(found) = check(world, x, y, z) {
                    out.push(found);
                }
            }
        }
    }
}

fn check(world: &dyn World, x: i32, y: i32, z: i32) -> Option<Found> {
    let biome = world.biome(x, y, z);
    if !biome.is_empty() && biome != "end_highlands" {
        return None;
    }
    if world.block(x, y - 1, z) != "bedrock" || world.block(x, y + 1, z) != "bedrock" {
        return None;
    }

    let mut sy = y - 3;
    while sy >= world.min_y() && is_air(world.block(x, sy, z)) {
        sy -= 1;
    }
    if sy < world.min_y() || world.block(x, sy, z) != "end_stone" {
        return None;
    }
    let h = y - (sy + 1);
    if !(3..=9).contains(&h) {
        return None;
    }
    let height = Some(h);

    Some(Found {
        observation: Observation::Decorator(DecoratorObservation {
            kind: DecoratorKind::EndGateway,
            x,
            y,
            z,
            height_above_surface: height,
        }),
        boxes: vec![Cuboid::new([x - 1, y - 2, z - 1], [x + 2, y + 3, z + 2])],
        color: 0,
    })
}
