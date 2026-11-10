pub mod piece;
mod scattered;
mod template;

mod buried_treasure;
mod desert_pyramid;
mod desert_well;
mod dungeon;
mod end_city;
mod end_gateway;
mod end_pillars;
mod igloo;
mod jungle_temple;
mod monument;
mod outpost;
mod shipwreck;
mod swamp_hut;
mod trial_chambers;

use crate::decorator::DecoratorKind;
use crate::placement::StructureSet;
use crate::version::Dimension;
use crate::world::{Found, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Yields {
    Structure(StructureSet),
    Decorator(DecoratorKind),
    Pillars,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FinderKind {
    BuriedTreasure,
    DesertPyramid,
    JungleTemple,
    SwampHut,
    Igloo,
    Shipwreck,
    Monument,
    Outpost,
    TrialChambers,
    EndCity,
    EndPillars,
    EndGateway,
    Dungeon,
    DesertWell,
}

impl FinderKind {
    pub const ALL: [FinderKind; 14] = [
        FinderKind::BuriedTreasure,
        FinderKind::DesertPyramid,
        FinderKind::JungleTemple,
        FinderKind::SwampHut,
        FinderKind::Igloo,
        FinderKind::Shipwreck,
        FinderKind::Monument,
        FinderKind::Outpost,
        FinderKind::TrialChambers,
        FinderKind::EndCity,
        FinderKind::EndPillars,
        FinderKind::EndGateway,
        FinderKind::Dungeon,
        FinderKind::DesertWell,
    ];

    pub const fn dimension(self) -> Dimension {
        match self {
            FinderKind::EndCity | FinderKind::EndPillars | FinderKind::EndGateway => Dimension::End,
            _ => Dimension::Overworld,
        }
    }

    pub const fn yields(self) -> Yields {
        match self {
            FinderKind::BuriedTreasure => Yields::Structure(StructureSet::BuriedTreasures),
            FinderKind::DesertPyramid => Yields::Structure(StructureSet::DesertPyramids),
            FinderKind::JungleTemple => Yields::Structure(StructureSet::JungleTemples),
            FinderKind::SwampHut => Yields::Structure(StructureSet::SwampHuts),
            FinderKind::Igloo => Yields::Structure(StructureSet::Igloos),
            FinderKind::Shipwreck => Yields::Structure(StructureSet::Shipwrecks),
            FinderKind::Monument => Yields::Structure(StructureSet::OceanMonuments),
            FinderKind::Outpost => Yields::Structure(StructureSet::PillagerOutposts),
            FinderKind::TrialChambers => Yields::Structure(StructureSet::TrialChambers),
            FinderKind::EndCity => Yields::Structure(StructureSet::EndCities),
            FinderKind::EndPillars => Yields::Pillars,
            FinderKind::EndGateway => Yields::Decorator(DecoratorKind::EndGateway),
            FinderKind::Dungeon => Yields::Decorator(DecoratorKind::Dungeon),
            FinderKind::DesertWell => Yields::Decorator(DecoratorKind::DesertWell),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            FinderKind::BuriedTreasure => "Buried treasure",
            FinderKind::DesertPyramid => "Desert pyramid",
            FinderKind::JungleTemple => "Jungle temple",
            FinderKind::SwampHut => "Swamp hut",
            FinderKind::Igloo => "Igloo",
            FinderKind::Shipwreck => "Shipwreck",
            FinderKind::Monument => "Ocean monument",
            FinderKind::Outpost => "Pillager outpost",
            FinderKind::TrialChambers => "Trial chambers",
            FinderKind::EndCity => "End city",
            FinderKind::EndPillars => "End pillars",
            FinderKind::EndGateway => "End gateway",
            FinderKind::Dungeon => "Dungeon",
            FinderKind::DesertWell => "Desert well",
        }
    }

    pub const fn color(self) -> u32 {
        match self {
            FinderKind::BuriedTreasure => 0xFFFF00,
            FinderKind::DesertPyramid => 0xFFC800,
            FinderKind::JungleTemple => 0x00FF00,
            FinderKind::SwampHut => 0xFF00FF,
            FinderKind::Igloo => 0xB0CFFC,
            FinderKind::Shipwreck => 0x00FFFF,
            FinderKind::Monument => 0x0000FF,
            FinderKind::Outpost => 0xAA5403,
            FinderKind::TrialChambers => 0xB87333,
            FinderKind::EndCity => 0x9900FF,
            FinderKind::EndPillars => 0x8080FF,
            FinderKind::EndGateway => 0x6666D2,
            FinderKind::Dungeon => 0xFF0000,
            FinderKind::DesertWell => 0x8080FF,
        }
    }

    pub fn run(self, world: &dyn World, chunk_x: i32, chunk_z: i32, out: &mut Vec<Found>) {
        let color = self.color();
        let start = out.len();
        match self {
            FinderKind::BuriedTreasure => buried_treasure::find(world, chunk_x, chunk_z, out),
            FinderKind::DesertPyramid => desert_pyramid::find(world, chunk_x, chunk_z, out),
            FinderKind::JungleTemple => jungle_temple::find(world, chunk_x, chunk_z, out),
            FinderKind::SwampHut => swamp_hut::find(world, chunk_x, chunk_z, out),
            FinderKind::Igloo => igloo::find(world, chunk_x, chunk_z, out),
            FinderKind::Shipwreck => shipwreck::find(world, chunk_x, chunk_z, out),
            FinderKind::Monument => monument::find(world, chunk_x, chunk_z, out),
            FinderKind::Outpost => outpost::find(world, chunk_x, chunk_z, out),
            FinderKind::TrialChambers => trial_chambers::find(world, chunk_x, chunk_z, out),
            FinderKind::EndCity => end_city::find(world, chunk_x, chunk_z, out),
            FinderKind::EndPillars => end_pillars::find(world, chunk_x, chunk_z, out),
            FinderKind::EndGateway => end_gateway::find(world, chunk_x, chunk_z, out),
            FinderKind::Dungeon => dungeon::find(world, chunk_x, chunk_z, out),
            FinderKind::DesertWell => desert_well::find(world, chunk_x, chunk_z, out),
        }
        for f in &mut out[start..] {
            f.color = color;
        }
    }
}

pub fn scan_chunk(
    world: &dyn World,
    dimension: Dimension,
    chunk_x: i32,
    chunk_z: i32,
    enabled: impl Fn(FinderKind) -> bool,
) -> Vec<Found> {
    let mut out = Vec::new();
    for kind in FinderKind::ALL {
        if kind.dimension() == dimension && enabled(kind) {
            kind.run(world, chunk_x, chunk_z, &mut out);
        }
    }
    out
}

pub(crate) fn chunk_biome(world: &dyn World, chunk_x: i32, y: i32, chunk_z: i32) -> &str {
    world.biome((chunk_x << 4) + 8, y, (chunk_z << 4) + 8)
}
