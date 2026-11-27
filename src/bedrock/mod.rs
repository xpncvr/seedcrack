pub(crate) mod search;
mod wide;

use std::ops::Range;
use std::sync::OnceLock;

use crate::hash::with_upper;
use crate::lcg::{ADDEND, JavaRandom, MASK, MULTIPLIER};
use crate::xoroshiro::WorldgenRandom;

use search::{Constraint, Lanes, Modulus, Search};

pub use search::Overflow;

pub const MIN_NARROW_CELLS: usize = 64;
pub const MAX_SURVIVORS: usize = 4096;

pub(crate) const GRADIENT_THRESHOLDS: [u64; 4] = [0xCC_CCCD, 0x99_999A, 0x66_6667, 0x33_3334];
const FLOAT_STEPS: u64 = 1 << 24;
const FLOOR_NAME_HASH: i64 = 2_042_456_806;
const ROOF_NAME_HASH: i64 = 343_340_730;
const OVERWORLD_FLOOR_NAME_MD5: (u64, u64) = (0xBBF7_928B_7BF1_D285, 0xC4DC_7CF9_0E1B_3B94);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Band {
    NetherFloor,
    NetherRoof,
    OverworldFloor,
}

impl Band {
    pub const ALL: [Band; 3] = [Band::NetherFloor, Band::NetherRoof, Band::OverworldFloor];

    pub fn gradient_ys(self) -> Range<i32> {
        let first = match self {
            Band::NetherFloor => 1,
            Band::NetherRoof => 123,
            Band::OverworldFloor => -63,
        };
        first..first + 4
    }

    pub fn of_y(y: i32) -> Option<Band> {
        Band::ALL.into_iter().find(|b| b.gradient_ys().contains(&y))
    }

    fn bedrock_below_threshold(self) -> bool {
        self != Band::NetherRoof
    }

    fn name_hash(self) -> Option<i64> {
        match self {
            Band::NetherFloor => Some(FLOOR_NAME_HASH),
            Band::NetherRoof => Some(ROOF_NAME_HASH),
            Band::OverworldFloor => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BedrockCell {
    x: i32,
    y: i32,
    z: i32,
    bedrock: bool,
}

impl BedrockCell {
    pub fn new(band: Band, x: i32, y: i32, z: i32, bedrock: bool) -> Option<BedrockCell> {
        band.gradient_ys().contains(&y).then_some(BedrockCell { x, y, z, bedrock })
    }

    pub fn x(&self) -> i32 {
        self.x
    }

    pub fn y(&self) -> i32 {
        self.y
    }

    pub fn z(&self) -> i32 {
        self.z
    }

    pub fn bedrock(&self) -> bool {
        self.bedrock
    }

    pub fn band(&self) -> Band {
        Band::of_y(self.y).expect("cells are only built inside a gradient band")
    }

    pub fn narrow(&self) -> bool {
        self.accepted_steps().end - self.accepted_steps().start < FLOAT_STEPS / 4
    }

    pub fn bits(&self) -> f64 {
        let accepted = self.accepted_steps();
        -((accepted.end - accepted.start) as f64 / FLOAT_STEPS as f64).log2()
    }

    pub(crate) fn accepted_steps(&self) -> Range<u64> {
        let band = self.band();
        let threshold = GRADIENT_THRESHOLDS[(self.y - band.gradient_ys().start) as usize];
        if self.bedrock == band.bedrock_below_threshold() {
            0..threshold
        } else {
            threshold..FLOAT_STEPS
        }
    }

    pub(crate) fn position_hash(&self) -> i64 {
        position_hash(self.x, self.y, self.z)
    }

    pub(crate) fn legacy_constraint(&self) -> Constraint {
        let accepted = self.accepted_steps();
        Constraint::new(
            Modulus::JAVA,
            (self.position_hash() ^ MULTIPLIER) as u64,
            accepted.start << 24..accepted.end << 24,
        )
    }

    pub fn holds_for_band_seed(&self, band_seed: i64) -> bool {
        self.legacy_constraint().holds(Modulus::JAVA, band_seed as u64)
    }
}

pub fn position_hash(x: i32, y: i32, z: i32) -> i64 {
    let l = (x.wrapping_mul(3_129_871) as i64) ^ (z as i64).wrapping_mul(116_129_781) ^ (y as i64);
    let l = l.wrapping_mul(l).wrapping_mul(42_317_861).wrapping_add(l.wrapping_mul(11));
    l >> 16
}

#[derive(Clone, Debug)]
pub struct BandSearch {
    band: Band,
    search: Search,
}

impl BandSearch {
    pub fn new(band: Band, cells: &[BedrockCell]) -> Option<BandSearch> {
        band.name_hash()?;
        let mut chosen: Vec<BedrockCell> = cells.iter().filter(|c| c.band() == band).copied().collect();
        chosen.sort_unstable();
        chosen.dedup();
        if chosen.iter().filter(|c| c.narrow()).count() < MIN_NARROW_CELLS {
            return None;
        }
        let constraints: Vec<Constraint> = chosen.iter().map(BedrockCell::legacy_constraint).collect();
        Some(BandSearch { band, search: Search::new(Modulus::JAVA, &constraints) })
    }

    pub fn band(&self) -> Band {
        self.band
    }

    pub fn total(&self) -> u64 {
        self.search.total()
    }

    pub fn scan(&self, range: Range<u64>, out: &mut Vec<i64>) -> Result<(), Overflow> {
        self.scan_with(range, out, Lanes::Widest)
    }

    pub(crate) fn scan_limited(&self, range: Range<u64>, out: &mut Vec<i64>, limit: usize) -> Result<(), Overflow> {
        let mut found = Vec::new();
        self.search.scan(range, &mut found, limit, Lanes::Widest)?;
        out.extend(found.into_iter().map(|s| s as i64));
        Ok(())
    }

    pub(crate) fn scan_with(&self, range: Range<u64>, out: &mut Vec<i64>, lanes: Lanes) -> Result<(), Overflow> {
        let mut found = Vec::new();
        self.search.scan(range, &mut found, MAX_SURVIVORS, lanes)?;
        out.extend(found.into_iter().map(|s| s as i64));
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn search(&self) -> &Search {
        &self.search
    }
}

#[cfg(test)]
pub(crate) fn wide_lanes_available() -> bool {
    wide::available()
}

fn legacy_next_long(seed: i64) -> i64 {
    JavaRandom::new(seed).next_long()
}

fn band_seed_after_fork(name_hash: i64, positional: i64) -> i64 {
    legacy_next_long(name_hash ^ positional) & MASK
}

pub fn band_seed(band: Band, structure_seed: i64) -> Option<i64> {
    Some(band_seed_after_fork(band.name_hash()?, legacy_next_long(structure_seed)))
}

pub fn structure_seeds(band: Band, band_seed: i64) -> Vec<i64> {
    let Some(name_hash) = band.name_hash() else {
        return Vec::new();
    };
    let mut seeds: Vec<i64> = next_long_preimages(band_seed)
        .into_iter()
        .flat_map(|forked| next_long_preimages(forked ^ name_hash))
        .collect();
    seeds.sort_unstable();
    seeds.dedup();
    seeds
}

pub fn shared_structure_seeds(floor: Option<&[i64]>, roof: Option<&[i64]>) -> Vec<i64> {
    let mut seeds = match (floor, roof) {
        (None, None) => Vec::new(),
        (Some(only), None) => only.iter().flat_map(|&g| structure_seeds(Band::NetherFloor, g)).collect(),
        (None, Some(only)) => only.iter().flat_map(|&g| structure_seeds(Band::NetherRoof, g)).collect(),
        (Some(floor), Some(roof)) => {
            let mut roof: Vec<i64> = roof.iter().map(|g| g & MASK).collect();
            roof.sort_unstable();
            floor
                .iter()
                .flat_map(|&g| next_long_preimages(g))
                .map(|forked| (forked ^ FLOOR_NAME_HASH) & MASK)
                .filter(|&positional| {
                    roof.binary_search(&band_seed_after_fork(ROOF_NAME_HASH, positional)).is_ok()
                })
                .flat_map(next_long_preimages)
                .collect()
        }
    };
    seeds.sort_unstable();
    seeds.dedup();
    seeds
}

const MULTIPLIER_INVERSE: u64 = multiplier_inverse();

const fn multiplier_inverse() -> u64 {
    let multiplier = MULTIPLIER as u64;
    let mut inverse = multiplier;
    let mut round = 0;
    while round < 6 {
        inverse = inverse.wrapping_mul(2u64.wrapping_sub(multiplier.wrapping_mul(inverse)));
        round += 1;
    }
    inverse
}

fn low_multiples() -> &'static [(u32, u16)] {
    static TABLE: OnceLock<Vec<(u32, u16)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table: Vec<(u32, u16)> = (0..=u16::MAX)
            .map(|low| ((low as u32).wrapping_mul(MULTIPLIER as u32), low))
            .collect();
        table.sort_unstable();
        table
    })
}

pub fn next_long_preimages(value: i64) -> Vec<i64> {
    let target = value as u64 & MASK as u64;
    let lower_word = target & 0xFFFF_FFFF;
    let upper_middle = ((target >> 32) + (lower_word >> 31)) & 0xFFFF;
    let multiplier = MULTIPLIER as u64;
    let mask = MASK as u64;
    let base = ((upper_middle << 16).wrapping_mul(multiplier).wrapping_add(ADDEND as u64)) & mask;
    let window_start = ((lower_word & 0xFFFF) << 16).wrapping_sub(base) as u32;
    let table = low_multiples();
    let preimage = |&(_, low): &(u32, u16)| -> i64 {
        let low = low as u64;
        let partial = base.wrapping_add(low.wrapping_mul(multiplier)) & mask;
        let missing = ((lower_word >> 16).wrapping_sub(partial >> 32)) & 0xFFFF;
        let top = missing.wrapping_mul(MULTIPLIER_INVERSE) & 0xFFFF;
        let first_state = (top << 32) | (upper_middle << 16) | low;
        let seed_state = first_state.wrapping_sub(ADDEND as u64).wrapping_mul(MULTIPLIER_INVERSE) & mask;
        ((seed_state ^ multiplier) & mask) as i64
    };
    let window_end = window_start as u64 + (1 << 16);
    let from = table.partition_point(|&(v, _)| v < window_start);
    let mut found: Vec<i64> = table[from..]
        .iter()
        .take_while(|&&(v, _)| (v as u64) < window_end)
        .map(preimage)
        .collect();
    if window_end > 1 << 32 {
        let wrapped_end = (window_end - (1 << 32)) as u32;
        found.extend(table.iter().take_while(|&&(v, _)| v < wrapped_end).map(preimage));
    }
    found.sort_unstable();
    found
}

#[derive(Clone, Debug)]
pub struct OverworldFloor {
    cells: Vec<(u64, Range<u64>)>,
}

impl OverworldFloor {
    pub fn new(cells: &[BedrockCell]) -> OverworldFloor {
        let mut chosen: Vec<BedrockCell> =
            cells.iter().filter(|c| c.band() == Band::OverworldFloor).copied().collect();
        chosen.sort_unstable();
        chosen.dedup();
        let mut cells: Vec<(u64, Range<u64>)> =
            chosen.iter().map(|c| (c.position_hash() as u64, c.accepted_steps())).collect();
        cells.sort_by_key(|(hash, accepted)| (accepted.end - accepted.start, *hash, accepted.start));
        OverworldFloor { cells }
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn test(&self, world_seed: i64) -> bool {
        let mut world = WorldgenRandom::new();
        world.set_seed(world_seed);
        let positional = (world.next_raw(), world.next_raw());
        let mut named = WorldgenRandom::from_state(
            positional.0 ^ OVERWORLD_FLOOR_NAME_MD5.0,
            positional.1 ^ OVERWORLD_FLOOR_NAME_MD5.1,
        );
        let floor = (named.next_raw(), named.next_raw());
        self.cells.iter().all(|(hash, accepted)| {
            let steps = WorldgenRandom::from_state(hash ^ floor.0, floor.1).next_raw() >> 40;
            accepted.contains(&steps)
        })
    }

    pub fn world_seeds(&self, structure_seed: i64) -> Vec<i64> {
        (0..=u16::MAX)
            .map(|upper| with_upper(structure_seed, upper))
            .filter(|&w| self.test(w))
            .collect()
    }
}
