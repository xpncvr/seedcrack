use super::scattered::biome_allows;
use super::piece::is_air;
use crate::decorator::{DecoratorKind, DecoratorObservation};
use crate::world::{Cuboid, Found, Observation, World};

const SS: &str = "sandstone";
const SLAB: &str = "sandstone_slab";
const WATER: &str = "water";

const SAMPLES: [i32; 6] = [1, 4, 7, 10, 13, 16];

fn matches(world: &dyn World, x: i32, y: i32, z: i32) -> bool {
    let at = |dx: i32, dy: i32, dz: i32, want: &str| world.block(x + dx, y + dy, z + dz) == want;
    if !at(0, 0, 0, WATER) || !at(0, 4, 0, SS) {
        return false;
    }
    for dx in -2..=2i32 {
        for dz in -2..=2i32 {
            let plus = dx.abs() + dz.abs() <= 1;
            let rim = dx.abs() == 2 || dz.abs() == 2;
            let pillar = dx.abs() == 1 && dz.abs() == 1;
            if !plus && !(at(dx, -2, dz, SS) && at(dx, -1, dz, SS) && at(dx, 0, dz, SS)) {
                return false;
            }
            if plus && !at(dx, 0, dz, WATER) {
                return false;
            }
            if rim && !at(dx, 1, dz, if dx == 0 || dz == 0 { SLAB } else { SS }) {
                return false;
            }
            if pillar && !(at(dx, 1, dz, SS) && at(dx, 2, dz, SS) && at(dx, 3, dz, SS)) {
                return false;
            }
            if !rim && (dx, dz) != (0, 0) && !at(dx, 4, dz, SLAB) {
                return false;
            }
        }
    }
    true
}

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    if !biome_allows(world, chunk_x, chunk_z, &["desert"]) {
        return;
    }
    let (x0, z0) = (chunk_x << 4, chunk_z << 4);
    let mut tried: Vec<[i32; 3]> = Vec::new();
    let mut try_at = |x: i32, y: i32, z: i32, out: &mut Vec<Found>| {
        if x >> 4 != chunk_x || z >> 4 != chunk_z || tried.contains(&[x, y, z]) {
            return;
        }
        tried.push([x, y, z]);
        if !matches(world, x, y, z) {
            return;
        }
        out.push(Found {
            observation: Observation::Decorator(DecoratorObservation {
                kind: DecoratorKind::DesertWell,
                x,
                y,
                z,
                height_above_surface: None,
            }),
            boxes: vec![Cuboid::sized([x - 2, y - 2, z - 2], [5, 7, 5]), Cuboid::block(x, y, z)],
            color: 0,
        });
    };
    for sx in SAMPLES {
        for sz in SAMPLES {
            let (x, z) = (x0 + sx, z0 + sz);
            for y in world.min_y()..world.max_y() {
                let block = world.block(x, y, z);
                if block == SLAB {
                    for dx in -1..=1 {
                        for dz in -1..=1 {
                            if (dx, dz) != (0, 0) {
                                try_at(x - dx, y - 4, z - dz, &mut *out);
                            }
                        }
                    }
                    for (dx, dz) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
                        try_at(x - dx, y - 1, z - dz, &mut *out);
                    }
                } else if block == SS && is_air(world.block(x, y + 1, z)) && world.block(x + 1, y, z) == SLAB {
                    try_at(x, y - 4, z, &mut *out);
                }
            }
        }
    }
}
