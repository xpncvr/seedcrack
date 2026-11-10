use std::sync::OnceLock;

use super::piece::Facing;
use super::scattered::{Builder, Layout, UNSURE, biome_allows, find_at_corner};
use crate::placement::StructureSet;
use crate::world::{Found, World};

const W: i32 = 21;
const D: i32 = 21;

const SS: &str = "sandstone";
const CUT: &str = "cut_sandstone";
const CHIS: &str = "chiseled_sandstone";
const ORANGE: &str = "orange_terracotta";
const BLUE: &str = "blue_terracotta";
const STAIRS: &str = "sandstone_stairs";
const SLAB: &str = "sandstone_slab";
const AIR: &str = "air";

const FACADE: [[&str; 3]; 7] = [
    [CUT, ORANGE, CUT],
    [CUT, ORANGE, CUT],
    [ORANGE, CHIS, ORANGE],
    [CUT, ORANGE, CUT],
    [ORANGE, CHIS, ORANGE],
    [ORANGE, ORANGE, ORANGE],
    [CUT, CUT, CUT],
];

fn build(facing: Facing) -> Layout {
    let mut p = Builder::new(facing, W, 15, D, 14);
    p.fill(0, -4, 0, W - 1, 0, D - 1, SS);
    for x in 1..=9 {
        p.fill(x, x, x, W - 1 - x, x, D - 1 - x, SS);
        p.fill(x + 1, x, x + 1, W - 2 - x, x, D - 2 - x, AIR);
    }
    p.generate_box(0, 0, 0, 4, 9, 4, SS, AIR, false);
    p.fill(1, 10, 1, 3, 10, 3, SS);
    p.place(STAIRS, 2, 10, 0);
    p.place(STAIRS, 2, 10, 4);
    p.place(STAIRS, 0, 10, 2);
    p.place(STAIRS, 4, 10, 2);
    p.generate_box(W - 5, 0, 0, W - 1, 9, 4, SS, AIR, false);
    p.fill(W - 4, 10, 1, W - 2, 10, 3, SS);
    p.place(STAIRS, W - 3, 10, 0);
    p.place(STAIRS, W - 3, 10, 4);
    p.place(STAIRS, W - 5, 10, 2);
    p.place(STAIRS, W - 1, 10, 2);
    p.generate_box(8, 0, 0, 12, 4, 4, SS, AIR, false);
    p.fill(9, 1, 0, 11, 3, 4, AIR);
    for (x, y) in [(9, 1), (9, 2), (9, 3), (10, 3), (11, 3), (11, 2), (11, 1)] {
        p.place(CUT, x, y, 1);
    }
    p.generate_box(4, 1, 1, 8, 3, 3, SS, AIR, false);
    p.fill(4, 1, 2, 8, 2, 2, AIR);
    p.generate_box(12, 1, 1, 16, 3, 3, SS, AIR, false);
    p.fill(12, 1, 2, 16, 2, 2, AIR);
    p.fill(5, 4, 5, W - 6, 4, D - 6, SS);
    p.fill(9, 4, 9, 11, 4, 11, AIR);
    p.fill(8, 1, 8, 8, 3, 8, CUT);
    p.fill(12, 1, 8, 12, 3, 8, CUT);
    p.fill(8, 1, 12, 8, 3, 12, CUT);
    p.fill(12, 1, 12, 12, 3, 12, CUT);
    p.fill(1, 1, 5, 4, 4, 11, SS);
    p.fill(W - 5, 1, 5, W - 2, 4, 11, SS);
    p.fill(6, 7, 9, 6, 7, 11, SS);
    p.fill(W - 7, 7, 9, W - 7, 7, 11, SS);
    p.fill(5, 5, 9, 5, 7, 11, CUT);
    p.fill(W - 6, 5, 9, W - 6, 7, 11, CUT);
    p.place(AIR, 5, 5, 10);
    p.place(AIR, 5, 6, 10);
    p.place(AIR, 6, 6, 10);
    p.place(AIR, W - 6, 5, 10);
    p.place(AIR, W - 6, 6, 10);
    p.place(AIR, W - 7, 6, 10);
    p.fill(2, 4, 4, 2, 6, 4, AIR);
    p.fill(W - 3, 4, 4, W - 3, 6, 4, AIR);
    p.place(STAIRS, 2, 4, 5);
    p.place(STAIRS, 2, 3, 4);
    p.place(STAIRS, W - 3, 4, 5);
    p.place(STAIRS, W - 3, 3, 4);
    p.fill(1, 1, 3, 2, 2, 3, SS);
    p.fill(W - 3, 1, 3, W - 2, 2, 3, SS);
    p.place(SS, 1, 1, 2);
    p.place(SS, W - 2, 1, 2);
    p.place(SLAB, 1, 2, 2);
    p.place(SLAB, W - 2, 2, 2);
    p.place(STAIRS, 2, 1, 2);
    p.place(STAIRS, W - 3, 1, 2);
    p.fill(4, 3, 5, 4, 3, 17, SS);
    p.fill(W - 5, 3, 5, W - 5, 3, 17, SS);
    p.fill(3, 1, 5, 4, 2, 16, AIR);
    p.fill(W - 6, 1, 5, W - 5, 2, 16, AIR);
    for z in (5..=17).step_by(2) {
        p.place(CUT, 4, 1, z);
        p.place(CHIS, 4, 2, z);
        p.place(CUT, W - 5, 1, z);
        p.place(CHIS, W - 5, 2, z);
    }
    for (x, z) in [(10, 7), (10, 8), (9, 9), (11, 9), (8, 10), (12, 10), (7, 10), (13, 10), (9, 11), (11, 11), (10, 12), (10, 13)] {
        p.place(ORANGE, x, 0, z);
    }
    p.place(BLUE, 10, 0, 10);
    for x in [0, W - 1] {
        for (row, cells) in FACADE.iter().enumerate() {
            for (i, &b) in cells.iter().enumerate() {
                p.place(b, x, row as i32 + 2, i as i32 + 1);
            }
        }
    }
    for x in [2, W - 3] {
        for (row, cells) in FACADE.iter().enumerate() {
            for (i, &b) in cells.iter().enumerate() {
                p.place(b, x - 1 + i as i32, row as i32 + 2, 0);
            }
        }
    }
    p.fill(8, 4, 0, 12, 6, 0, CUT);
    p.place(AIR, 8, 6, 0);
    p.place(AIR, 12, 6, 0);
    p.place(ORANGE, 9, 5, 0);
    p.place(CHIS, 10, 5, 0);
    p.place(ORANGE, 11, 5, 0);

    p.fill(8, -14, 8, 12, -11, 12, CUT);
    p.fill(8, -10, 8, 12, -10, 12, CHIS);
    p.fill(8, -9, 8, 12, -9, 12, CUT);
    p.fill(8, -8, 8, 12, -1, 12, SS);
    p.fill(9, -11, 9, 11, -1, 11, AIR);
    p.place(UNSURE, 10, -11, 10);
    p.fill(9, -13, 9, 11, -13, 11, UNSURE);
    for (x, z, dx, dz) in [(8, 10, -1, 0), (12, 10, 1, 0), (10, 8, 0, -1), (10, 12, 0, 1)] {
        p.place(AIR, x, -11, z);
        p.place(AIR, x, -10, z);
        p.place(CHIS, x + dx, -10, z + dz);
        p.place(CUT, x + dx, -11, z + dz);
    }
    for (x, z) in [(10, 8), (12, 10), (10, 12), (8, 10)] {
        p.place(UNSURE, x, -11, z);
    }

    let (cx, cy, cz) = (16, -4, 13);
    let mut sand: Vec<(i32, i32, i32)> = Vec::new();
    p.place(STAIRS, 13, -1, 17);
    p.place(STAIRS, 14, -2, 17);
    p.place(STAIRS, 15, -3, 17);
    for x in cx - 4..=cx {
        p.place(UNSURE, x, cy + 4, cz + 4);
    }
    p.place(UNSURE, cx - 2, cy + 3, cz + 4);
    p.place(UNSURE, cx - 1, cy + 3, cz + 4);
    p.place(UNSURE, cx, cy + 3, cz + 4);
    p.place(UNSURE, cx - 1, cy + 2, cz + 4);
    p.place(SS, cx, cy + 2, cz + 4);
    p.place(UNSURE, cx, cy + 1, cz + 4);
    for (y, block) in [(cy + 1, CUT), (cy + 2, CHIS), (-1, CUT)] {
        p.generate_box(cx - 3, y, cz - 3, cx - 3, y, cz + 2, block, block, true);
        p.generate_box(cx + 3, y, cz - 3, cx + 3, y, cz + 2, block, block, true);
        p.generate_box(cx - 3, y, cz - 3, cx + 3, y, cz - 2, block, block, true);
        p.generate_box(cx - 3, y, cz + 3, cx + 3, y, cz + 3, block, block, true);
    }
    for y in cy + 1..=cy + 3 {
        for x in cx - 2..=cx + 2 {
            for z in cz - 2..=cz + 2 {
                sand.push((x, y, z));
            }
        }
    }
    p.fill(cx - 2, cy + 4, cz - 2, cx + 2, cy + 4, cz + 2, UNSURE);
    p.place(BLUE, cx, cy, cz);
    for (dx, dz) in [(1, -1), (1, 1), (-1, -1), (-1, 1), (2, 0), (-2, 0), (0, 2), (0, -2)] {
        p.place(ORANGE, cx + dx, cy, cz + dz);
    }
    p.place(ORANGE, cx + 3, cy, cz);
    sand.push((cx + 3, cy + 1, cz));
    sand.push((cx + 3, cy + 2, cz));
    p.place(CUT, cx + 4, cy + 1, cz);
    p.place(CHIS, cx + 4, cy + 2, cz);
    p.place(ORANGE, cx - 3, cy, cz);
    sand.push((cx - 3, cy + 1, cz));
    sand.push((cx - 3, cy + 2, cz));
    p.place(CUT, cx - 4, cy + 1, cz);
    p.place(CHIS, cx - 4, cy + 2, cz);
    p.place(ORANGE, cx, cy, cz + 3);
    sand.push((cx, cy + 1, cz + 3));
    sand.push((cx, cy + 2, cz + 3));
    p.place(ORANGE, cx, cy, cz - 3);
    sand.push((cx, cy + 1, cz - 3));
    sand.push((cx, cy + 2, cz - 3));
    p.place(CUT, cx, cy + 1, cz - 4);
    p.place(CHIS, cx, -2, cz - 4);
    for (x, y, z) in sand {
        p.place(UNSURE, x, y, z);
    }

    p.build(&[BLUE, ORANGE, CHIS, CUT, STAIRS, SLAB, SS])
}

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    if !biome_allows(world, chunk_x, chunk_z, &["desert"]) {
        return;
    }
    static LAYOUTS: OnceLock<Vec<Layout>> = OnceLock::new();
    let layouts = LAYOUTS.get_or_init(|| Facing::HORIZONTAL.iter().map(|&f| build(f)).collect());
    find_at_corner(world, chunk_x, chunk_z, layouts, StructureSet::DesertPyramids, out);
}
