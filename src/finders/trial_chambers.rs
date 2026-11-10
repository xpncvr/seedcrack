use super::template::{Rotation, template_matches};
use crate::placement::StructureSet;
use crate::world::{Cuboid, Found, Observation, StructureObservation, World};

const SIZE: [i32; 3] = [19, 20, 19];

const MIN_Y: i32 = -41;
const MAX_Y: i32 = -21;

const END_1: [([i32; 3], &str); 24] = [
    ([8, 5, 12], "chiseled_tuff"),
    ([16, 18, 0], "polished_tuff"),
    ([1, 12, 0], "chiseled_tuff_bricks"),
    ([15, 18, 16], "polished_tuff"),
    ([1, 17, 15], "chiseled_tuff_bricks"),
    ([15, 4, 0], "waxed_oxidized_copper"),
    ([3, 2, 3], "waxed_oxidized_copper"),
    ([8, 18, 7], "polished_tuff"),
    ([16, 10, 11], "waxed_copper_block"),
    ([1, 10, 9], "waxed_copper_block"),
    ([16, 2, 15], "waxed_oxidized_copper"),
    ([1, 6, 16], "waxed_copper_block"),
    ([16, 18, 8], "polished_tuff"),
    ([13, 2, 7], "waxed_oxidized_copper"),
    ([6, 18, 0], "polished_tuff"),
    ([1, 17, 5], "chiseled_tuff_bricks"),
    ([8, 18, 15], "polished_tuff"),
    ([2, 2, 10], "waxed_oxidized_cut_copper"),
    ([13, 9, 5], "waxed_copper_grate"),
    ([5, 8, 4], "polished_tuff"),
    ([9, 2, 0], "waxed_oxidized_copper"),
    ([6, 12, 12], "chiseled_tuff_bricks"),
    ([12, 7, 16], "waxed_copper_block"),
    ([16, 12, 0], "chiseled_tuff_bricks"),
];

const END_2: [([i32; 3], &str); 24] = [
    ([9, 4, 6], "waxed_oxidized_cut_copper_slab"),
    ([1, 17, 16], "waxed_copper_block"),
    ([15, 18, 0], "polished_tuff"),
    ([16, 14, 16], "waxed_copper_block"),
    ([1, 15, 2], "chiseled_tuff_bricks"),
    ([2, 2, 16], "waxed_oxidized_cut_copper"),
    ([16, 2, 16], "waxed_oxidized_cut_copper"),
    ([9, 18, 9], "polished_tuff"),
    ([1, 5, 0], "waxed_copper_block"),
    ([16, 8, 0], "polished_tuff"),
    ([1, 10, 10], "waxed_copper_block"),
    ([9, 7, 15], "waxed_oxidized_copper"),
    ([16, 12, 7], "chiseled_tuff_bricks"),
    ([16, 1, 8], "waxed_oxidized_copper"),
    ([2, 2, 8], "waxed_oxidized_cut_copper"),
    ([9, 0, 13], "waxed_oxidized_copper"),
    ([8, 18, 1], "polished_tuff"),
    ([8, 18, 16], "polished_tuff"),
    ([1, 17, 9], "waxed_copper_block"),
    ([7, 12, 6], "chiseled_tuff_bricks"),
    ([12, 2, 0], "waxed_oxidized_copper"),
    ([16, 18, 10], "polished_tuff"),
    ([11, 12, 11], "chiseled_tuff_bricks"),
    ([16, 7, 11], "chiseled_tuff_bricks"),
];

pub fn find(world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
    let (x0, z0) = (chunk_x << 4, chunk_z << 4);
    let low = MIN_Y.max(world.min_y());
    let high = MAX_Y.min(world.max_y() - SIZE[1]);
    for blocks in [&END_1, &END_2] {
        let (first, first_block) = blocks[0];
        for rotation in Rotation::ALL {
            let f = rotation.transform(first, [0, 0]);
            for y0 in low..=high {
                if world.block(x0 + f[0], y0 + f[1], z0 + f[2]) != first_block {
                    continue;
                }
                let origin = [x0, y0, z0];
                if !template_matches(world, origin, rotation, [0, 0], &blocks[1..]) {
                    continue;
                }
                let piece = rotation.template_box(origin, SIZE, [0, 0]);
                let mid_x = (piece.min[0] + piece.max[0] - 1) / 2;
                let mid_z = (piece.min[2] + piece.max[2] - 1) / 2;
                if world.biome(mid_x, y0 + 1, mid_z) == "deep_dark" {
                    return;
                }
                out.push(Found {
                    observation: Observation::Structure(StructureObservation {
                        set: StructureSet::TrialChambers,
                        chunk_x,
                        chunk_z,
                    }),
                    boxes: vec![piece, Cuboid::block(x0, y0, z0)],
                    color: 0,
                });
                return;
            }
        }
    }
}
