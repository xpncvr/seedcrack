use crate::world::World;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Facing {
    North,
    East,
    South,
    West,
}

impl Facing {
    pub const HORIZONTAL: [Facing; 4] = [Facing::North, Facing::East, Facing::South, Facing::West];

    pub const fn is_z_axis(self) -> bool {
        matches!(self, Facing::North | Facing::South)
    }
}

pub fn is_air(block: &str) -> bool {
    matches!(block, "air" | "cave_air" | "void_air")
}

#[derive(Clone, Debug)]
pub struct Piece {
    facing: Option<Facing>,
    size: [i32; 3],
    blocks: Vec<([i32; 3], &'static str)>,
}

impl Piece {
    pub fn new(facing: Option<Facing>, width: i32, height: i32, depth: i32) -> Piece {
        let size = match facing {
            Some(f) if !f.is_z_axis() => [depth, height, width],
            _ => [width, height, depth],
        };
        Piece { facing, size, blocks: Vec::new() }
    }

    pub fn facing(&self) -> Option<Facing> {
        self.facing
    }

    pub fn size(&self) -> [i32; 3] {
        self.size
    }

    pub fn blocks(&self) -> &[([i32; 3], &'static str)] {
        &self.blocks
    }

    pub fn world_x(&self, x: i32, z: i32) -> i32 {
        match self.facing {
            None => x,
            Some(Facing::North | Facing::South) => x,
            Some(Facing::West) => self.size[0] - 1 - z,
            Some(Facing::East) => z,
        }
    }

    pub fn world_y(&self, y: i32) -> i32 {
        y
    }

    pub fn world_z(&self, x: i32, z: i32) -> i32 {
        match self.facing {
            None => z,
            Some(Facing::North) => self.size[2] - 1 - z,
            Some(Facing::South) => z,
            Some(Facing::West | Facing::East) => x,
        }
    }

    fn world(&self, x: i32, y: i32, z: i32) -> [i32; 3] {
        [self.world_x(x, z), self.world_y(y), self.world_z(x, z)]
    }

    fn inside(&self, p: [i32; 3]) -> bool {
        (0..3).all(|i| p[i] >= 0 && p[i] < self.size[i])
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> &'static str {
        let p = self.world(x, y, z);
        if !self.inside(p) {
            return "air";
        }
        self.blocks.iter().rev().find(|(q, _)| *q == p).map_or("air", |(_, b)| *b)
    }

    pub fn place(&mut self, block: &'static str, x: i32, y: i32, z: i32) {
        let p = self.world(x, y, z);
        if !self.inside(p) {
            return;
        }
        if let Some(slot) = self.blocks.iter_mut().find(|(q, _)| *q == p) {
            slot.1 = block;
        } else {
            self.blocks.push((p, block));
        }
    }

    pub fn forget(&mut self, x: i32, y: i32, z: i32) {
        let p = self.world(x, y, z);
        self.blocks.retain(|(q, _)| *q != p);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate_box(
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
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if skip_air && is_air(self.get(x, y, z)) {
                        continue;
                    }
                    let inner = y != y0 && y != y1 && x != x0 && x != x1 && z != z0 && z != z1;
                    self.place(if inner { fill } else { edge }, x, y, z);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, block: &'static str) {
        self.generate_box(x0, y0, z0, x1, y1, z1, block, block, false);
    }

    pub fn matches(&self, world: &dyn World, origin: [i32; 3]) -> bool {
        self.blocks.iter().all(|(p, want)| {
            let got = world.block(origin[0] + p[0], origin[1] + p[1], origin[2] + p[2]);
            if is_air(want) { is_air(got) } else { got == *want }
        })
    }
}
