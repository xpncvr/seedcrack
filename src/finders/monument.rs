use crate::placement::StructureSet;
use crate::world::{Cuboid, Found, Observation, StructureObservation, World};

const BOTTOM: i32 = 39;
const GRAY: &str = "prismarine";
const LIGHT: &str = "prismarine_bricks";
const LAMP: &str = "sea_lantern";

fn spire() -> Vec<([i32; 3], &'static str)> {
    let mut b: Vec<([i32; 3], &'static str)> = Vec::new();
    for (x, z) in [(27, 27), (27, 30), (30, 30), (30, 27)] {
        b.push(([x, 20, z], LAMP));
    }
    for (x, z) in [(26, 26), (26, 31), (31, 31), (31, 26)] {
        b.push(([x, 20, z], LIGHT));
    }
    for (x, z) in [(27, 27), (27, 30), (30, 30), (30, 27)] {
        b.push(([x, 21, z], LIGHT));
    }
    for x in [28, 29] {
        b.push(([x, 21, 27], GRAY));
        b.push(([x, 21, 30], GRAY));
        b.push(([27, 21, x], GRAY));
        b.push(([30, 21, x], GRAY));
    }
    for x in [25, 32] {
        for z in [25, 32] {
            for y in 17..=19 {
                b.push(([x, y, z], LIGHT));
            }
        }
    }
    for x in 24..=33 {
        for z in 24..=33 {
            let ring = x == 24 || x == 33 || z == 24 || z == 33;
            b.push(([x, 16, z], if ring { LIGHT } else { GRAY }));
        }
    }
    b
}

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    let (x0, z0) = ((chunk_x << 4) - 29, (chunk_z << 4) - 29);
    if world.min_y() > BOTTOM || world.max_y() <= BOTTOM + 22 {
        return;
    }
    if world.block(x0 + 27, BOTTOM + 20, z0 + 27) != LAMP {
        return;
    }
    let ok = spire().iter().all(|&([x, y, z], want)| world.block(x0 + x, BOTTOM + y, z0 + z) == want);
    if !ok {
        return;
    }
    out.push(Found {
        observation: Observation::Structure(StructureObservation { set: StructureSet::OceanMonuments, chunk_x, chunk_z }),
        boxes: vec![
            Cuboid::sized([x0, BOTTOM, z0], [58, 23, 58]),
            Cuboid::sized([x0 + 24, BOTTOM + 16, z0 + 24], [10, 6, 10]),
            Cuboid::sized([chunk_x << 4, BOTTOM + 16, chunk_z << 4], [16, 1, 16]),
        ],
        color: 0,
    });
}
