use crate::world::{Cuboid, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Rotation {
    None,
    Clockwise90,
    Clockwise180,
    Counterclockwise90,
}

impl Rotation {
    pub(super) const ALL: [Rotation; 4] =
        [Rotation::None, Rotation::Clockwise90, Rotation::Clockwise180, Rotation::Counterclockwise90];

    pub(super) const fn transform(self, p: [i32; 3], pivot: [i32; 2]) -> [i32; 3] {
        let [x, y, z] = p;
        let [px, pz] = pivot;
        match self {
            Rotation::None => [x, y, z],
            Rotation::Clockwise90 => [px + pz - z, y, pz - px + x],
            Rotation::Clockwise180 => [px + px - x, y, pz + pz - z],
            Rotation::Counterclockwise90 => [px - pz + z, y, px + pz - x],
        }
    }

    pub(super) fn bounds(self, origin: [i32; 3], lo: [i32; 3], hi: [i32; 3], pivot: [i32; 2]) -> Cuboid {
        let a = self.transform(lo, pivot);
        let b = self.transform(hi, pivot);
        let mut min = [0; 3];
        let mut max = [0; 3];
        for i in 0..3 {
            min[i] = origin[i] + a[i].min(b[i]);
            max[i] = origin[i] + a[i].max(b[i]) + 1;
        }
        Cuboid::new(min, max)
    }

    pub(super) fn template_box(self, origin: [i32; 3], size: [i32; 3], pivot: [i32; 2]) -> Cuboid {
        self.bounds(origin, [0, 0, 0], [size[0] - 1, size[1] - 1, size[2] - 1], pivot)
    }
}

pub(super) fn template_matches(
    world: &dyn World,
    origin: [i32; 3],
    rotation: Rotation,
    pivot: [i32; 2],
    blocks: &[([i32; 3], &str)],
) -> bool {
    blocks.iter().all(|&(p, want)| {
        let q = rotation.transform(p, pivot);
        world.block(origin[0] + q[0], origin[1] + q[1], origin[2] + q[2]) == want
    })
}
