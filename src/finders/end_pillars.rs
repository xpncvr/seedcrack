use crate::pillar::{BASE_HEIGHT, CENTERS, HEIGHT_STEP, PillarObservation};
use crate::world::{Cuboid, Found, Observation, World};

const MAX_HEIGHT: i32 = BASE_HEIGHT + HEIGHT_STEP * 9;

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    for (i, &(px, pz)) in CENTERS.iter().enumerate() {
        if px >> 4 != chunk_x || pz >> 4 != chunk_z {
            continue;
        }
        let Some(h) = spike_height(world, px, pz) else { continue };
        let mut heights = [None; 10];
        heights[i] = Some(h);
        out.push(Found {
            observation: Observation::Pillars(PillarObservation { heights }),
            boxes: vec![Cuboid::block(px, h, pz)],
            color: 0,
        });
    }
}

fn spike_height(world: &dyn World, x: i32, z: i32) -> Option<i32> {
    let lo = BASE_HEIGHT.max(world.min_y() + 3);
    let hi = MAX_HEIGHT.min(world.max_y() - 1);
    let mut found = None;
    for y in lo..=hi {
        if world.block(x, y, z) == "bedrock" {
            if found.is_some() {
                return None;
            }
            found = Some(y);
        }
    }
    let h = found?;
    if (h - BASE_HEIGHT) % HEIGHT_STEP != 0 {
        return None;
    }
    if (1..=3).all(|d| world.block(x, h - d, z) == "obsidian") { Some(h) } else { None }
}
