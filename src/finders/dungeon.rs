use super::piece::is_air;
use crate::decorator::{DecoratorKind, DecoratorObservation};
use crate::world::{Cuboid, Found, Observation, World};

const INTERIOR_SLACK: u32 = 3;

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    let top = world.max_y() - 1;
    let deep_lo = world.min_y() + 6;
    let kinds = [(DecoratorKind::Dungeon, 0, top), (DecoratorKind::DeepDungeon, deep_lo, -1)];
    for (kind, lo, hi) in kinds {
        if lo > hi {
            continue;
        }
        if let Some((x, y, z, xr, zr)) = unique_room(world, chunk_x, chunk_z, lo, hi) {
            out.push(Found {
                observation: Observation::Decorator(DecoratorObservation {
                    kind,
                    x,
                    y,
                    z,
                    height_above_surface: None,
                }),
                boxes: vec![Cuboid::new([x - xr - 1, y - 1, z - zr - 1], [x + xr + 2, y + 5, z + zr + 2])],
                color: 0,
            });
        }
    }
}

fn unique_room(
    world: &dyn World,
    chunk_x: i32,
    chunk_z: i32,
    lo: i32,
    hi: i32,
) -> Option<(i32, i32, i32, i32, i32)> {
    let (bx, bz) = (chunk_x << 4, chunk_z << 4);
    let mut hit = None;
    let mut spawners = 0;
    for y in lo..=hi {
        for x in bx..bx + 16 {
            for z in bz..bz + 16 {
                if world.block(x, y, z) != "spawner" {
                    continue;
                }
                spawners += 1;
                if spawners > 1 {
                    return None;
                }
                hit = room(world, x, y, z).map(|(xr, zr)| (x, y, z, xr, zr));
            }
        }
    }
    hit
}

fn room(world: &dyn World, x: i32, y: i32, z: i32) -> Option<(i32, i32)> {
    for xr in 2..=3 {
        for zr in 2..=3 {
            if room_with(world, x, y, z, xr, zr) {
                return Some((xr, zr));
            }
        }
    }
    None
}

fn room_with(world: &dyn World, x: i32, y: i32, z: i32, xr: i32, zr: i32) -> bool {
    let (wx, wz) = (xr + 1, zr + 1);

    let mut area = 0;
    let mut cobble = 0;
    let mut mossy = 0;
    for dx in -wx..=wx {
        for dz in -wz..=wz {
            area += 1;
            match world.block(x + dx, y - 1, z + dz) {
                "cobblestone" => cobble += 1,
                "mossy_cobblestone" => {
                    cobble += 1;
                    mossy += 1;
                }
                b if is_air(b) => {}
                _ => return false,
            }
        }
    }
    if mossy == 0 || cobble * 3 < area * 2 {
        return false;
    }

    let mut wall_cells = 0;
    let mut wall_cobble = 0;
    let mut other = 0;
    for dy in 0..=3 {
        for dx in -wx..=wx {
            for dz in -wz..=wz {
                if dx == 0 && dy == 0 && dz == 0 {
                    continue;
                }
                let b = world.block(x + dx, y + dy, z + dz);
                if dx.abs() == wx || dz.abs() == wz {
                    wall_cells += 1;
                    if b == "cobblestone" {
                        wall_cobble += 1;
                    }
                } else if !(is_air(b) || matches!(b, "chest" | "water" | "lava")) {
                    other += 1;
                    if other > INTERIOR_SLACK {
                        return false;
                    }
                }
            }
        }
    }
    wall_cobble * 2 >= wall_cells
}
