use crate::lcg::JavaRandom;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Spread {
    Linear,
    Triangular,
}

impl Spread {
    #[inline]
    pub fn evaluate(self, random: &mut JavaRandom, limit: i32) -> i32 {
        match self {
            Spread::Linear => random.next_int(limit),
            Spread::Triangular => (random.next_int(limit) + random.next_int(limit)) / 2,
        }
    }

    pub fn probability(self, limit: i32, offset: i32) -> f64 {
        if offset < 0 || offset >= limit {
            return 0.0;
        }
        match self {
            Spread::Linear => 1.0 / limit as f64,
            Spread::Triangular => {
                let pairs = |s: i32| -> i32 {
                    if s < 0 || s > 2 * limit - 2 { 0 } else { s.min(2 * limit - 2 - s) + 1 }
                };
                (pairs(2 * offset) + pairs(2 * offset + 1)) as f64 / (limit as f64 * limit as f64)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reduction {
    Default,
    LegacyType1,
    LegacyType2,
    LegacyType3,
}

pub const LEGACY_TYPE_2_SALT: i32 = 10_387_320;

impl Reduction {
    pub fn should_generate(self, seed: i64, salt: i32, x: i32, z: i32, probability: f32) -> bool {
        let mut r = JavaRandom::new(0);
        match self {
            Reduction::Default => {
                r.set_large_feature_with_salt(seed, salt, x, z);
                r.next_float() < probability
            }
            Reduction::LegacyType1 => {
                let cx = x >> 4;
                let cz = z >> 4;
                r.set_seed((cx ^ cz << 4) as i64 ^ seed);
                r.next_int_any();
                r.next_int((1.0f32 / probability) as i32) == 0
            }
            Reduction::LegacyType2 => {
                r.set_large_feature_with_salt(seed, x, z, LEGACY_TYPE_2_SALT);
                r.next_float() < probability
            }
            Reduction::LegacyType3 => {
                r.set_large_feature_seed(seed, x, z);
                r.next_double() < probability as f64
            }
        }
    }

    pub fn pass_rate(self, probability: f32) -> f64 {
        match self {
            Reduction::LegacyType1 => 1.0 / (1.0f32 / probability) as i32 as f64,
            _ => probability as f64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StructureSet {
    AncientCities,
    BuriedTreasures,
    DesertPyramids,
    EndCities,
    Igloos,
    JungleTemples,
    Mineshafts,
    NetherComplexes,
    NetherFossils,
    OceanMonuments,
    OceanRuins,
    PillagerOutposts,
    RuinedPortals,
    Shipwrecks,
    SwampHuts,
    TrailRuins,
    TrialChambers,
    Villages,
    WoodlandMansions,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exclusion {
    pub other: StructureSet,
    pub chunk_count: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub spacing: i32,
    pub separation: i32,
    pub salt: i32,
    pub spread: Spread,
    pub frequency: f32,
    pub reduction: Reduction,
    pub exclusion: Option<Exclusion>,
}

const fn linear(spacing: i32, separation: i32, salt: i32) -> Placement {
    Placement {
        spacing,
        separation,
        salt,
        spread: Spread::Linear,
        frequency: 1.0,
        reduction: Reduction::Default,
        exclusion: None,
    }
}

const fn triangular(spacing: i32, separation: i32, salt: i32) -> Placement {
    Placement { spread: Spread::Triangular, ..linear(spacing, separation, salt) }
}

const fn reduced(p: Placement, frequency: f32, reduction: Reduction) -> Placement {
    Placement { frequency, reduction, ..p }
}

impl StructureSet {
    pub const ALL: [StructureSet; 19] = [
        StructureSet::AncientCities,
        StructureSet::BuriedTreasures,
        StructureSet::DesertPyramids,
        StructureSet::EndCities,
        StructureSet::Igloos,
        StructureSet::JungleTemples,
        StructureSet::Mineshafts,
        StructureSet::NetherComplexes,
        StructureSet::NetherFossils,
        StructureSet::OceanMonuments,
        StructureSet::OceanRuins,
        StructureSet::PillagerOutposts,
        StructureSet::RuinedPortals,
        StructureSet::Shipwrecks,
        StructureSet::SwampHuts,
        StructureSet::TrailRuins,
        StructureSet::TrialChambers,
        StructureSet::Villages,
        StructureSet::WoodlandMansions,
    ];

    pub const fn placement(self) -> Placement {
        match self {
            StructureSet::AncientCities => linear(24, 8, 20_083_232),
            StructureSet::BuriedTreasures => reduced(linear(1, 0, 0), 0.01, Reduction::LegacyType2),
            StructureSet::DesertPyramids => linear(32, 8, 14_357_617),
            StructureSet::EndCities => triangular(20, 11, 10_387_313),
            StructureSet::Igloos => linear(32, 8, 14_357_618),
            StructureSet::JungleTemples => linear(32, 8, 14_357_619),
            StructureSet::Mineshafts => reduced(linear(1, 0, 0), 0.004, Reduction::LegacyType3),
            StructureSet::NetherComplexes => linear(27, 4, 30_084_232),
            StructureSet::NetherFossils => linear(2, 1, 14_357_921),
            StructureSet::OceanMonuments => triangular(32, 5, 10_387_313),
            StructureSet::OceanRuins => linear(20, 8, 14_357_621),
            StructureSet::PillagerOutposts => Placement {
                exclusion: Some(Exclusion { other: StructureSet::Villages, chunk_count: 10 }),
                ..reduced(linear(32, 8, 165_745_296), 0.2, Reduction::LegacyType1)
            },
            StructureSet::RuinedPortals => linear(40, 15, 34_222_645),
            StructureSet::Shipwrecks => linear(24, 4, 165_745_295),
            StructureSet::SwampHuts => linear(32, 8, 14_357_620),
            StructureSet::TrailRuins => linear(34, 8, 83_469_867),
            StructureSet::TrialChambers => linear(34, 12, 94_251_327),
            StructureSet::Villages => linear(34, 8, 10_387_312),
            StructureSet::WoodlandMansions => triangular(80, 20, 10_387_319),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            StructureSet::AncientCities => "ancient_cities",
            StructureSet::BuriedTreasures => "buried_treasures",
            StructureSet::DesertPyramids => "desert_pyramids",
            StructureSet::EndCities => "end_cities",
            StructureSet::Igloos => "igloos",
            StructureSet::JungleTemples => "jungle_temples",
            StructureSet::Mineshafts => "mineshafts",
            StructureSet::NetherComplexes => "nether_complexes",
            StructureSet::NetherFossils => "nether_fossils",
            StructureSet::OceanMonuments => "ocean_monuments",
            StructureSet::OceanRuins => "ocean_ruins",
            StructureSet::PillagerOutposts => "pillager_outposts",
            StructureSet::RuinedPortals => "ruined_portals",
            StructureSet::Shipwrecks => "shipwrecks",
            StructureSet::SwampHuts => "swamp_huts",
            StructureSet::TrailRuins => "trail_ruins",
            StructureSet::TrialChambers => "trial_chambers",
            StructureSet::Villages => "villages",
            StructureSet::WoodlandMansions => "woodland_mansions",
        }
    }

    pub fn from_name(name: &str) -> Option<StructureSet> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        StructureSet::ALL.into_iter().find(|s| s.name() == name)
    }
}

impl Placement {
    #[inline]
    pub const fn limit(&self) -> i32 {
        self.spacing - self.separation
    }

    #[inline]
    pub const fn region_of(&self, chunk_x: i32, chunk_z: i32) -> (i32, i32) {
        (chunk_x.div_euclid(self.spacing), chunk_z.div_euclid(self.spacing))
    }

    #[inline]
    pub fn region_offset(&self, seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
        let mut r = JavaRandom::new(0);
        r.set_large_feature_with_salt(seed, region_x, region_z, self.salt);
        let limit = self.limit();
        let x = self.spread.evaluate(&mut r, limit);
        let z = self.spread.evaluate(&mut r, limit);
        (x, z)
    }

    pub fn potential_chunk(&self, seed: i64, chunk_x: i32, chunk_z: i32) -> (i32, i32) {
        let (rx, rz) = self.region_of(chunk_x, chunk_z);
        let (ox, oz) = self.region_offset(seed, rx, rz);
        (rx * self.spacing + ox, rz * self.spacing + oz)
    }

    #[inline]
    pub fn is_placement_chunk(&self, seed: i64, chunk_x: i32, chunk_z: i32) -> bool {
        self.potential_chunk(seed, chunk_x, chunk_z) == (chunk_x, chunk_z)
    }

    #[inline]
    pub fn passes_frequency(&self, seed: i64, chunk_x: i32, chunk_z: i32) -> bool {
        !(self.frequency < 1.0)
            || self.reduction.should_generate(seed, self.salt, chunk_x, chunk_z, self.frequency)
    }

    pub fn is_structure_chunk(&self, seed: i64, chunk_x: i32, chunk_z: i32) -> bool {
        if !self.is_placement_chunk(seed, chunk_x, chunk_z) || !self.passes_frequency(seed, chunk_x, chunk_z) {
            return false;
        }
        match self.exclusion {
            None => true,
            Some(e) => !e.other.placement().has_structure_chunk_in_range(seed, chunk_x, chunk_z, e.chunk_count),
        }
    }

    pub fn has_structure_chunk_in_range(&self, seed: i64, chunk_x: i32, chunk_z: i32, range: i32) -> bool {
        let (rx0, rz0) = self.region_of(chunk_x - range, chunk_z - range);
        let (rx1, rz1) = self.region_of(chunk_x + range, chunk_z + range);
        for rx in rx0..=rx1 {
            for rz in rz0..=rz1 {
                let (ox, oz) = self.region_offset(seed, rx, rz);
                let (cx, cz) = (rx * self.spacing + ox, rz * self.spacing + oz);
                if (cx - chunk_x).abs() <= range
                    && (cz - chunk_z).abs() <= range
                    && self.is_structure_chunk(seed, cx, cz)
                {
                    return true;
                }
            }
        }
        false
    }

    pub fn bits(&self, chunk_x: i32, chunk_z: i32) -> f64 {
        let (rx, rz) = self.region_of(chunk_x, chunk_z);
        let ox = chunk_x - rx * self.spacing;
        let oz = chunk_z - rz * self.spacing;
        let mut p = self.spread.probability(self.limit(), ox) * self.spread.probability(self.limit(), oz);
        if self.frequency < 1.0 {
            p *= self.reduction.pass_rate(self.frequency);
        }
        if p <= 0.0 { 0.0 } else { -p.log2() }
    }

    pub fn expected_bits(&self) -> f64 {
        let limit = self.limit();
        let axis: f64 = (0..limit)
            .map(|v| self.spread.probability(limit, v))
            .filter(|&p| p > 0.0)
            .map(|p| -p * p.log2())
            .sum();
        let reduce = if self.frequency < 1.0 { -self.reduction.pass_rate(self.frequency).log2() } else { 0.0 };
        2.0 * axis + reduce
    }

    pub fn liftable(&self) -> bool {
        let l = self.limit();
        self.spread == Spread::Linear && l % 4 == 0 && l & (l - 1) != 0
    }
}
