use std::sync::OnceLock;

use super::scattered::{Builder, Layout, UNSURE, biome_allows, find_at_corner};
use super::piece::Facing;
use crate::placement::StructureSet;
use crate::world::{Found, World};

const PLANKS: &str = "spruce_planks";
const LOG: &str = "oak_log";
const FENCE: &str = "oak_fence";
const STAIRS: &str = "spruce_stairs";
const TABLE: &str = "crafting_table";
const AIR: &str = "air";

fn build(facing: Facing) -> Layout {
    let mut p = Builder::new(facing, 7, 7, 9, 0);
    p.fill(1, 1, 1, 5, 1, 7, PLANKS);
    p.fill(1, 4, 2, 5, 4, 7, PLANKS);
    p.fill(2, 1, 0, 4, 1, 0, PLANKS);
    p.fill(2, 2, 2, 3, 3, 2, PLANKS);
    p.fill(1, 2, 3, 1, 3, 6, PLANKS);
    p.fill(5, 2, 3, 5, 3, 6, PLANKS);
    p.fill(2, 2, 7, 4, 3, 7, PLANKS);
    p.fill(1, 0, 2, 1, 3, 2, LOG);
    p.fill(5, 0, 2, 5, 3, 2, LOG);
    p.fill(1, 0, 7, 1, 3, 7, LOG);
    p.fill(5, 0, 7, 5, 3, 7, LOG);
    p.place(FENCE, 2, 3, 2);
    p.place(FENCE, 3, 3, 7);
    p.place(AIR, 1, 3, 4);
    p.place(AIR, 5, 3, 4);
    p.place(AIR, 5, 3, 5);
    p.place(UNSURE, 1, 3, 5);
    p.place(TABLE, 3, 2, 6);
    p.place(UNSURE, 4, 2, 6);
    p.place(FENCE, 1, 2, 1);
    p.place(FENCE, 5, 2, 1);
    p.fill(0, 4, 1, 6, 4, 1, STAIRS);
    p.fill(0, 4, 2, 0, 4, 7, STAIRS);
    p.fill(6, 4, 2, 6, 4, 7, STAIRS);
    p.fill(0, 4, 8, 6, 4, 8, STAIRS);
    p.build(&[TABLE, FENCE, STAIRS, LOG, PLANKS])
}

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    if !biome_allows(world, chunk_x, chunk_z, &["swamp"]) {
        return;
    }
    static LAYOUTS: OnceLock<Vec<Layout>> = OnceLock::new();
    let layouts = LAYOUTS.get_or_init(|| Facing::HORIZONTAL.iter().map(|&f| build(f)).collect());
    find_at_corner(world, chunk_x, chunk_z, layouts, StructureSet::SwampHuts, out);
}
