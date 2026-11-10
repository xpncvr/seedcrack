use super::piece::{Facing, Piece, is_air};
use crate::placement::StructureSet;
use crate::world::{Cuboid, Found, Observation, StructureObservation, World};

pub(super) const UNSURE: &str = "?";

pub(super) const COBBLE_OR_MOSSY: &str = "#cobblestone|mossy_cobblestone";

fn block_is(got: &str, want: &str) -> bool {
    match want {
        COBBLE_OR_MOSSY => matches!(got, "cobblestone" | "mossy_cobblestone"),
        _ => got == want,
    }
}

pub(super) fn biome_allows(world: &dyn World, chunk_x: i32, chunk_z: i32, allowed: &[&str]) -> bool {
    let mut y = world.min_y().max(48);
    while y < world.max_y() {
        let biome = super::chunk_biome(world, chunk_x, y, chunk_z);
        if biome.is_empty() || allowed.iter().any(|a| *a == biome) {
            return true;
        }
        y += 24;
    }
    false
}

pub(super) struct Builder {
    piece: Piece,
    below: i32,
    height: i32,
}

impl Builder {
    pub(super) fn new(facing: Facing, width: i32, height: i32, depth: i32, below: i32) -> Builder {
        Builder { piece: Piece::new(Some(facing), width, height + below, depth), below, height }
    }

    pub(super) fn place(&mut self, block: &'static str, x: i32, y: i32, z: i32) {
        self.piece.place(block, x, y + self.below, z);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn generate_box(
        &mut self,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        edge: &'static str,
        fill: &'static str,
        skip_air: bool,
    ) {
        let b = self.below;
        self.piece.generate_box(x0, y0 + b, z0, x1, y1 + b, z1, edge, fill, skip_air);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, block: &'static str) {
        self.generate_box(x0, y0, z0, x1, y1, z1, block, block, false);
    }

    pub(super) fn build(self, rare: &[&str]) -> Layout {
        let below = self.below;
        let mut checks: Vec<([i32; 3], &'static str)> = self
            .piece
            .blocks()
            .iter()
            .filter(|(_, b)| !is_air(b) && *b != UNSURE)
            .map(|&([x, y, z], b)| ([x, y - below, z], b))
            .collect();
        checks.sort_by_key(|(_, b)| rare.iter().position(|r| r == b).unwrap_or(rare.len()));
        let y_lo = checks.iter().map(|(p, _)| p[1]).min().unwrap_or(0);
        let y_hi = checks.iter().map(|(p, _)| p[1]).max().unwrap_or(0);
        let s = self.piece.size();
        Layout { size: [s[0], self.height, s[2]], checks, y_lo, y_hi }
    }
}

pub(super) struct Layout {
    size: [i32; 3],
    checks: Vec<([i32; 3], &'static str)>,
    y_lo: i32,
    y_hi: i32,
}

impl Layout {
    pub(super) fn matches(&self, world: &dyn World, origin: [i32; 3]) -> bool {
        self.checks.iter().all(|&(p, want)| {
            block_is(world.block(origin[0] + p[0], origin[1] + p[1], origin[2] + p[2]), want)
        })
    }

    pub(super) fn find_y(&self, world: &dyn World, x: i32, z: i32) -> Option<i32> {
        (world.min_y() - self.y_lo..world.max_y() - self.y_hi).find(|&y| self.matches(world, [x, y, z]))
    }
}

pub(super) fn find_at_corner(
    world: &dyn World,
    chunk_x: i32,
    chunk_z: i32,
    layouts: &[Layout],
    set: StructureSet,
    out: &mut Vec<Found>,
) {
    let (x0, z0) = (chunk_x << 4, chunk_z << 4);
    for layout in layouts {
        if let Some(y) = layout.find_y(world, x0, z0) {
            out.push(Found {
                observation: Observation::Structure(StructureObservation { set, chunk_x, chunk_z }),
                boxes: vec![Cuboid::sized([x0, y, z0], layout.size), Cuboid::sized([x0, y, z0], [16, 1, 16])],
                color: 0,
            });
            return;
        }
    }
}
