use crate::version::GenRange;
use crate::xoroshiro::WorldgenRandom;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecoratorKind {
    Dungeon,
    DeepDungeon,
    DesertWell,
    EndGateway,
}

impl DecoratorKind {
    pub const ALL: [DecoratorKind; 4] =
        [DecoratorKind::Dungeon, DecoratorKind::DeepDungeon, DecoratorKind::DesertWell, DecoratorKind::EndGateway];

    pub const fn feature(self) -> (i32, i32) {
        match self {
            DecoratorKind::Dungeon => (3, 2),
            DecoratorKind::DeepDungeon => (3, 3),
            DecoratorKind::DesertWell => (4, 2),
            DecoratorKind::EndGateway => (4, 0),
        }
    }

    pub fn bits(self, range: GenRange, height_known: bool) -> f64 {
        let p = match self {
            DecoratorKind::Dungeon => 10.0 / (256.0 * (range.top() + 1) as f64),
            DecoratorKind::DeepDungeon => 4.0 / (256.0 * (-1 - (range.min_y + 6) + 1) as f64),
            DecoratorKind::DesertWell => 1.0 / (1000.0 * 256.0),
            DecoratorKind::EndGateway => {
                let h = if height_known { 7.0 } else { 1.0 };
                1.0 / (700.0 * 256.0 * h)
            }
        };
        -p.log2()
    }

    pub const fn name(self) -> &'static str {
        match self {
            DecoratorKind::Dungeon => "dungeon",
            DecoratorKind::DeepDungeon => "deep dungeon",
            DecoratorKind::DesertWell => "desert well",
            DecoratorKind::EndGateway => "end gateway",
        }
    }
}

const DUNGEON_FAILED_DRAWS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DecoratorObservation {
    pub kind: DecoratorKind,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub height_above_surface: Option<i32>,
}

impl DecoratorObservation {
    pub const fn chunk(&self) -> (i32, i32) {
        (self.x >> 4, self.z >> 4)
    }

    pub fn test(&self, world_seed: i64, range: GenRange, rand: &mut WorldgenRandom) -> bool {
        let (cx, cz) = self.chunk();
        let decoration = rand.set_decoration_seed(world_seed, cx << 4, cz << 4);
        let (step, index) = self.kind.feature();
        rand.set_feature_seed(decoration, index, step);
        let (ox, oz) = (self.x & 15, self.z & 15);
        match self.kind {
            DecoratorKind::Dungeon | DecoratorKind::DeepDungeon => {
                let (count, lo, hi) = if self.kind == DecoratorKind::Dungeon {
                    (10, 0, range.top())
                } else {
                    (4, range.min_y + 6, -1)
                };
                for _ in 0..count {
                    let x = rand.next_int(16);
                    let z = rand.next_int(16);
                    let y = rand.between(lo, hi);
                    if x == ox && z == oz && y == self.y {
                        return true;
                    }
                    for _ in 0..DUNGEON_FAILED_DRAWS {
                        rand.next_int(2);
                    }
                }
                false
            }
            DecoratorKind::DesertWell => {
                rand.next_float() < 1.0f32 / 1000.0f32 && rand.next_int(16) == ox && rand.next_int(16) == oz
            }
            DecoratorKind::EndGateway => {
                if !(rand.next_float() < 1.0f32 / 700.0f32) {
                    return false;
                }
                if rand.next_int(16) != ox || rand.next_int(16) != oz {
                    return false;
                }
                let dy = rand.between(3, 9);
                self.height_above_surface.is_none_or(|h| h == dy)
            }
        }
    }

    pub fn bits(&self, range: GenRange) -> f64 {
        self.kind.bits(range, self.height_above_surface.is_some())
    }
}
