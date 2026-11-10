use crate::decorator::DecoratorObservation;
use crate::pillar::PillarObservation;
use crate::placement::StructureSet;

pub trait World {
    fn block(&self, x: i32, y: i32, z: i32) -> &str;

    fn biome(&self, x: i32, y: i32, z: i32) -> &str;

    fn min_y(&self) -> i32;

    fn max_y(&self) -> i32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StructureObservation {
    pub set: StructureSet,
    pub chunk_x: i32,
    pub chunk_z: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Observation {
    Structure(StructureObservation),
    Decorator(DecoratorObservation),
    Pillars(PillarObservation),
    HashedSeed(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cuboid {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl Cuboid {
    pub const fn new(min: [i32; 3], max: [i32; 3]) -> Cuboid {
        Cuboid { min, max }
    }

    pub const fn sized(min: [i32; 3], size: [i32; 3]) -> Cuboid {
        Cuboid { min, max: [min[0] + size[0], min[1] + size[1], min[2] + size[2]] }
    }

    pub const fn block(x: i32, y: i32, z: i32) -> Cuboid {
        Cuboid::sized([x, y, z], [1, 1, 1])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub observation: Observation,
    pub boxes: Vec<Cuboid>,
    pub color: u32,
}
