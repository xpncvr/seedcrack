use std::ops::Range;

use crate::bedrock::{Band, BandSearch, BedrockCell, MAX_SURVIVORS, OverworldFloor, shared_structure_seeds};
use crate::decorator::{DecoratorKind, DecoratorObservation};
use crate::finders::{FinderKind, Yields};
use crate::hash;
use crate::pillar::{self, PillarObservation};
use crate::placement::{Placement, Spread, StructureSet};
use crate::prefilter::Prefilter;
use crate::version::{GenRange, Version};
use crate::world::{Observation, StructureObservation};
use crate::xoroshiro::WorldgenRandom;
use crate::lcg::{ADDEND, MASK, MULTIPLIER, REGION_A, REGION_B};

pub const PILLAR_BITS: f64 = 32.0;
pub const MAX_PILLAR_SEEDS: usize = 16;
pub const LIFT_BITS: f64 = 40.0;
pub const LIFT_COUNT: usize = 4;
pub const MAX_STRUCTURE_SEEDS: usize = 1024;
pub const DECORATOR_MARGIN: f64 = 4.0;
pub const BEDROCK_TARGET: usize = 256;
pub const OVERWORLD_BEDROCK_CAP: usize = 512;
pub const OVERWORLD_BEDROCK_MIN: usize = 256;

const BEDROCK_OVERFLOW: i64 = -1;
const LOW_BITS: u32 = 19;
const NORMAL_LIFT: u32 = LOW_BITS - 17;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    Normal,
    Fast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Bedrock,
    LiftLow,
    LiftHigh,
    Pillars,
    WorldByHash,
    WorldByDecorators,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    PillarSeeds(Vec<u16>),
    PillarsInconsistent,
    Started(Stage),
    LiftedLowParts(usize),
    StructureSeeds(Vec<i64>),
    NoStructureSeed,
    WorldSeeds(Vec<i64>),
    HashMismatch,
    NoWorldSeed,
    BandSeeds(Band, usize),
    BedrockOverflow(Band),
    BedrockInconsistent,
    BedrockConflict,
    SightingIgnored(StructureObservation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Trust {
    Bedrock,
    Structures,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BedrockNeed {
    VisitNether,
    VisitOverworld,
    Collecting,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum BandResult {
    #[default]
    Pending,
    Found(Vec<i64>),
    Overflow,
}

impl BandResult {
    fn usable(&self) -> Option<&[i64]> {
        match self {
            BandResult::Found(seeds) if !seeds.is_empty() => Some(seeds),
            _ => None,
        }
    }
}

fn nether_slot(band: Band) -> Option<usize> {
    match band {
        Band::NetherFloor => Some(0),
        Band::NetherRoof => Some(1),
        Band::OverworldFloor => None,
    }
}

const NETHER_BANDS: [Band; 2] = [Band::NetherFloor, Band::NetherRoof];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hint {
    pub finder: FinderKind,
    pub bits: f64,
    pub why: &'static str,
    pub rank: u8,
}

#[derive(Clone, Debug)]
pub(crate) enum JobKind {
    LiftLow { low_bits: u32 },
    LiftHigh { low_bits: u32, lows: Vec<i64> },
    Pillars { seeds: Vec<u16> },
    WorldByHash { hashed: i64, structures: Vec<i64> },
    WorldByDecorators { structures: Vec<i64>, overworld: Vec<BedrockCell>, floor: OverworldFloor },
    Bedrock { cells: Vec<BedrockCell>, limit: usize, search: BandSearch },
}

#[derive(Clone, Debug)]
struct Job {
    id: u64,
    kind: JobKind,
    mode: Mode,
    next: u64,
    returned: Vec<Range<u64>>,
    done: u64,
    total: u64,
    found: Vec<i64>,
}

impl Job {
    fn new(id: u64, kind: JobKind, mode: Mode) -> Job {
        let total = kind.total();
        Job { id, kind, mode, next: 0, returned: Vec::new(), done: 0, total, found: Vec::new() }
    }

    fn cost(&self) -> u64 {
        self.kind.cost()
    }

    fn stage(&self) -> Stage {
        self.kind.stage()
    }
}

impl JobKind {
    pub(crate) fn world(structures: Vec<i64>, overworld: Vec<BedrockCell>) -> JobKind {
        let floor = OverworldFloor::new(&overworld);
        JobKind::WorldByDecorators { structures, overworld, floor }
    }

    pub(crate) fn bedrock(band: Band, cells: Vec<BedrockCell>, limit: usize) -> Option<JobKind> {
        if cells.iter().any(|c| c.band() != band) {
            return None;
        }
        let search = BandSearch::new(band, &cells)?;
        Some(JobKind::Bedrock { cells, limit, search })
    }

    pub(crate) fn total(&self) -> u64 {
        match self {
            JobKind::LiftLow { low_bits } => 1 << low_bits,
            JobKind::LiftHigh { low_bits, lows } => (lows.len() as u64) << (48 - low_bits),
            JobKind::Pillars { seeds } => (seeds.len() as u64) << 32,
            JobKind::WorldByHash { structures, .. } | JobKind::WorldByDecorators { structures, .. } => {
                (structures.len() as u64) << 16
            }
            JobKind::Bedrock { search, .. } => search.total(),
        }
    }

    fn cost(&self) -> u64 {
        match self {
            JobKind::WorldByHash { .. } => 32,
            JobKind::WorldByDecorators { .. } => 4,
            _ => 1,
        }
    }

    fn stage(&self) -> Stage {
        match self {
            JobKind::LiftLow { .. } => Stage::LiftLow,
            JobKind::LiftHigh { .. } => Stage::LiftHigh,
            JobKind::Pillars { .. } => Stage::Pillars,
            JobKind::WorldByHash { .. } => Stage::WorldByHash,
            JobKind::WorldByDecorators { .. } => Stage::WorldByDecorators,
            JobKind::Bedrock { .. } => Stage::Bedrock,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Check {
    pub(crate) placement: Placement,
    chunk: (i32, i32),
    pub(crate) region: (i32, i32),
    pub(crate) offset: (i32, i32),
    pub(crate) key: i64,
    bits: f64,
}

impl Check {
    pub(crate) fn new(o: &StructureObservation) -> Check {
        let placement = o.set.placement();
        let region = placement.region_of(o.chunk_x, o.chunk_z);
        let offset = (o.chunk_x - region.0 * placement.spacing, o.chunk_z - region.1 * placement.spacing);
        let key = (region.0 as i64)
            .wrapping_mul(REGION_A)
            .wrapping_add((region.1 as i64).wrapping_mul(REGION_B))
            .wrapping_add(placement.salt as i64);
        Check { placement, chunk: (o.chunk_x, o.chunk_z), region, offset, key, bits: placement.bits(o.chunk_x, o.chunk_z) }
    }

    #[inline]
    pub(crate) fn test(&self, seed: i64) -> bool {
        self.placement.region_offset(seed, self.region.0, self.region.1) == self.offset
            && self.placement.passes_frequency(seed, self.chunk.0, self.chunk.1)
            && match self.placement.exclusion {
                None => true,
                Some(e) => !e.other.placement().has_structure_chunk_in_range(seed, self.chunk.0, self.chunk.1, e.chunk_count),
            }
    }

    #[inline]
    pub(crate) fn test_low(&self, low: i64, bits: u32) -> bool {
        let p = self.placement.lift_bits().min(bits - 17);
        if p == 0 {
            return true;
        }
        let m = (1u64 << bits) - 1;
        let x = ((low.wrapping_add(self.key) ^ MULTIPLIER) as u64) & m;
        let step = |s: u64| s.wrapping_mul(MULTIPLIER as u64).wrapping_add(ADDEND as u64) & m;
        let (s1, s2, s3) = { let a = step(x); let b = step(a); (a, b, step(b)) };
        let pm = (1u64 << p) - 1;
        let (ox, oz) = (self.offset.0 as u64 & pm, self.offset.1 as u64 & pm);
        let (a, b, d) = ((s1 >> 17) & pm, (s2 >> 17) & pm, (s3 >> 17) & pm);
        (a == ox && b == oz) || (b == ox && d == oz) || (a == ox && d == oz)
    }

    fn prefilter_power(&self, low_bits: u32) -> f64 {
        let used = self.placement.lift_bits().min(low_bits - 17) as f64;
        2.0 * ((self.placement.limit() as f64).log2() - used)
    }
}

pub(crate) fn prefilter_check(checks: &[Check], low_bits: u32) -> Option<&Check> {
    checks
        .iter()
        .filter(|c| c.placement.spread == Spread::Linear && c.placement.limit() > 1)
        .max_by(|a, b| a.prefilter_power(low_bits).total_cmp(&b.prefilter_power(low_bits)))
}

pub(crate) fn pillar_low_ok(lifts: &[&Check], pillar_seed: u16, klo: u32) -> bool {
    let low = pillar::structure_seed_from_pillar(klo & 0xFFFF, pillar_seed) & 0xFFFF_FFFF;
    lifts.iter().all(|c| c.test_low(low, 32))
}

#[derive(Clone, Debug)]
pub struct Cracker {
    version: Version,
    mode: Mode,
    range: GenRange,
    structures: Vec<StructureObservation>,
    checks: Vec<Check>,
    decorators: Vec<DecoratorObservation>,
    pillars: Option<PillarObservation>,
    hashed: Option<i64>,
    hash_failed: bool,

    pillar_seeds: Option<Vec<u16>>,
    structure_seeds: Option<Vec<i64>>,
    world_seeds: Option<Vec<i64>>,
    dirty: bool,
    job: Option<Job>,
    next_job: u64,
    events: Vec<Event>,

    nether: [Vec<BedrockCell>; 2],
    overworld: Vec<BedrockCell>,
    floor: OverworldFloor,
    band_results: [BandResult; 2],
    bedrock_structures: Option<Vec<i64>>,
    bedrock_on: bool,
    bedrock_distrusted: bool,
    conflict: bool,
    structures_from_bedrock: bool,
    pub(crate) survivor_limit: usize,
    pub(crate) overworld_min: usize,
}

impl Cracker {
    pub fn new(version: Version) -> Cracker {
        Cracker {
            nether: Default::default(),
            overworld: Vec::new(),
            floor: OverworldFloor::new(&[]),
            band_results: Default::default(),
            bedrock_structures: None,
            bedrock_on: true,
            bedrock_distrusted: false,
            conflict: false,
            structures_from_bedrock: false,
            survivor_limit: MAX_SURVIVORS,
            overworld_min: OVERWORLD_BEDROCK_MIN,
            version,
            mode: Mode::Normal,
            range: GenRange::OVERWORLD,
            structures: Vec::new(),
            checks: Vec::new(),
            decorators: Vec::new(),
            pillars: None,
            hashed: None,
            hash_failed: false,
            pillar_seeds: None,
            structure_seeds: None,
            world_seeds: None,
            dirty: false,
            job: None,
            next_job: 1,
            events: Vec::new(),
        }
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn set_gen_range(&mut self, range: GenRange) {
        self.range = range;
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        if self.job.as_ref().is_some_and(|j| matches!(j.kind, JobKind::LiftLow { .. } | JobKind::LiftHigh { .. } | JobKind::Pillars { .. })) {
            self.job = None;
        }
    }

    pub fn add(&mut self, observation: Observation) -> bool {
        let new = match observation {
            Observation::Structure(s) => {
                if self.structures.contains(&s) {
                    false
                } else if self.confirmed_seed().is_some_and(|w| !Check::new(&s).test(w & MASK)) {
                    self.events.push(Event::SightingIgnored(s));
                    false
                } else {
                    self.structures.push(s);
                    self.checks.push(Check::new(&s));
                    self.checks.sort_by(|a, b| b.bits.total_cmp(&a.bits));
                    true
                }
            }
            Observation::Decorator(d) => {
                if self.decorators.contains(&d) {
                    false
                } else {
                    self.decorators.push(d);
                    true
                }
            }
            Observation::Pillars(p) => match &mut self.pillars {
                Some(held) => {
                    let before = *held;
                    if !held.merge(&p) {
                        return false;
                    }
                    if *held == before {
                        false
                    } else {
                        self.pillar_seeds = None;
                        true
                    }
                }
                None => {
                    self.pillars = Some(p);
                    true
                }
            },
            Observation::HashedSeed(h) => {
                if self.hashed == Some(h) {
                    false
                } else {
                    self.hashed = Some(h);
                    self.hash_failed = false;
                    if matches!(self.job.as_ref().map(|j| &j.kind), Some(JobKind::WorldByHash { .. })) {
                        self.job = None;
                    }
                    self.world_seeds = None;
                    true
                }
            }
            Observation::Bedrock(cell) => self.add_bedrock(cell),
        };
        if new {
            self.dirty = true;
        }
        new
    }

    fn add_bedrock(&mut self, cell: BedrockCell) -> bool {
        if !self.bedrock_active() {
            return false;
        }
        match nether_slot(cell.band()) {
            Some(i) => {
                if !cell.narrow() || self.nether[i].len() >= BEDROCK_TARGET || self.nether[i].contains(&cell) {
                    return false;
                }
                self.nether[i].push(cell);
            }
            None => {
                if self.overworld.len() >= OVERWORLD_BEDROCK_CAP || self.overworld.contains(&cell) {
                    return false;
                }
                self.overworld.push(cell);
                self.floor = OverworldFloor::new(&self.overworld);
            }
        }
        true
    }

    pub fn clear(&mut self) {
        *self = Cracker {
            range: self.range,
            mode: self.mode,
            next_job: self.next_job,
            bedrock_on: self.bedrock_on,
            survivor_limit: self.survivor_limit,
            overworld_min: self.overworld_min,
            ..Cracker::new(self.version)
        };
    }

    pub fn restart(&mut self) {
        self.pillar_seeds = None;
        self.structure_seeds = None;
        self.world_seeds = None;
        self.hash_failed = false;
        self.job = None;
        self.band_results = Default::default();
        self.bedrock_structures = None;
        self.conflict = false;
        self.structures_from_bedrock = false;
        self.dirty = true;
    }

    pub fn confirmed_seed(&self) -> Option<i64> {
        self.world_seed().filter(|&w| self.hashed == Some(hash::obfuscate(w)))
    }

    fn bedrock_active(&self) -> bool {
        self.bedrock_on && !self.bedrock_distrusted
    }

    pub fn bedrock_enabled(&self) -> bool {
        self.bedrock_active()
    }

    pub fn set_bedrock(&mut self, on: bool) {
        if self.bedrock_on == on {
            return;
        }
        self.bedrock_on = on;
        if !on {
            self.drop_bedrock();
        }
    }

    pub fn bedrock_cells(&self, band: Band) -> usize {
        match nether_slot(band) {
            Some(i) => self.nether[i].len(),
            None => self.overworld.len(),
        }
    }

    pub fn bedrock_seeds(&self, band: Band) -> Option<usize> {
        match nether_slot(band).map(|i| &self.band_results[i]) {
            Some(BandResult::Found(seeds)) => Some(seeds.len()),
            _ => None,
        }
    }

    pub fn bedrock_structure_seeds(&self) -> Option<&[i64]> {
        self.bedrock_structures.as_deref()
    }

    pub fn conflict(&self) -> bool {
        self.conflict
    }

    pub fn resolve_conflict(&mut self, trust: Trust) {
        if !self.conflict {
            return;
        }
        self.conflict = false;
        match trust {
            Trust::Structures => {
                self.bedrock_distrusted = true;
                self.drop_bedrock();
            }
            Trust::Bedrock => {
                self.structures.clear();
                self.checks.clear();
                self.pillars = None;
                self.pillar_seeds = None;
                self.structure_seeds = None;
                self.world_seeds = None;
                self.hash_failed = false;
                if self.job.as_ref().is_some_and(|j| !matches!(j.kind, JobKind::Bedrock { .. })) {
                    self.job = None;
                }
                self.dirty = true;
                self.apply_bedrock();
            }
        }
    }

    pub fn bedrock_hint(&self) -> Option<BedrockNeed> {
        if !self.bedrock_active() || self.conflict || self.world_seed().is_some() {
            return None;
        }
        if !self.structure_seeds.as_ref().is_some_and(|s| !s.is_empty()) {
            if self.nether.iter().all(Vec::is_empty) {
                return Some(BedrockNeed::VisitNether);
            }
            return self.band_results.contains(&BandResult::Pending).then_some(BedrockNeed::Collecting);
        }
        let hash_usable = self.hashed.is_some() && !self.hash_failed;
        (!hash_usable && self.world_seeds.is_none() && self.overworld.len() < self.overworld_min)
            .then_some(BedrockNeed::VisitOverworld)
    }

    fn drop_bedrock(&mut self) {
        self.nether = Default::default();
        self.overworld.clear();
        self.floor = OverworldFloor::new(&[]);
        self.band_results = Default::default();
        self.bedrock_structures = None;
        self.conflict = false;
        if self.job.as_ref().is_some_and(|j| matches!(j.kind, JobKind::Bedrock { .. } | JobKind::WorldByDecorators { .. })) {
            self.job = None;
        }
        if self.structures_from_bedrock {
            self.structures_from_bedrock = false;
            self.structure_seeds = None;
            self.world_seeds = None;
            self.hash_failed = false;
            self.job = None;
        }
        self.dirty = true;
    }

    fn join_bedrock(&mut self) {
        let (floor, roof) = (self.band_results[0].usable(), self.band_results[1].usable());
        let joined = (floor.is_some() || roof.is_some()).then(|| shared_structure_seeds(floor, roof));
        self.bedrock_structures = joined;
        self.apply_bedrock();
    }

    fn apply_bedrock(&mut self) {
        let Some(candidates) = self.bedrock_structures.clone() else {
            return;
        };
        if candidates.is_empty() {
            self.events.push(Event::BedrockInconsistent);
            return;
        }
        if self.conflict {
            return;
        }
        let next: Vec<i64> = match &self.structure_seeds {
            None => {
                let pillar_seeds = self.usable_pillar_seeds();
                candidates.iter().copied().filter(|&s| self.fits_sightings(s, pillar_seeds.as_deref())).collect()
            }
            Some(held) => held.iter().copied().filter(|s| candidates.binary_search(s).is_ok()).collect(),
        };
        if next.is_empty() {
            self.raise_conflict();
            return;
        }
        if self.structure_seeds.as_ref() != Some(&next) {
            self.structures_from_bedrock = true;
            self.set_structure_seeds(next);
            self.hash_failed = false;
            if self.job.as_ref().is_some_and(|j| matches!(j.kind, JobKind::WorldByHash { .. } | JobKind::WorldByDecorators { .. })) {
                self.job = None;
            }
        }
    }

    fn raise_conflict(&mut self) {
        self.conflict = true;
        self.events.push(Event::BedrockConflict);
        if self.job.as_ref().is_some_and(|j| !matches!(j.kind, JobKind::Bedrock { .. })) {
            self.job = None;
        }
    }

    fn usable_pillar_seeds(&self) -> Option<Vec<u16>> {
        self.pillar_seeds.clone().filter(|p| !p.is_empty())
    }

    fn fits_sightings(&self, seed: i64, pillar_seeds: Option<&[u16]>) -> bool {
        self.test_structure_seed(seed) && pillar_seeds.is_none_or(|p| p.contains(&pillar::pillar_seed(seed)))
    }

    pub fn structures(&self) -> &[StructureObservation] {
        &self.structures
    }

    pub fn decorators(&self) -> &[DecoratorObservation] {
        &self.decorators
    }

    pub fn pillars(&self) -> Option<&PillarObservation> {
        self.pillars.as_ref()
    }

    pub fn hashed_seed(&self) -> Option<i64> {
        self.hashed
    }

    pub fn pillar_seeds(&self) -> Option<&[u16]> {
        self.pillar_seeds.as_deref()
    }

    pub fn structure_seeds(&self) -> Option<&[i64]> {
        self.structure_seeds.as_deref()
    }

    pub fn world_seeds(&self) -> Option<&[i64]> {
        self.world_seeds.as_deref()
    }

    pub fn world_seed(&self) -> Option<i64> {
        match self.world_seeds.as_deref() {
            Some([one]) => Some(*one),
            _ => None,
        }
    }

    pub fn structure_bits(&self) -> f64 {
        self.checks.iter().map(|c| c.bits).sum()
    }

    pub fn liftable_count(&self) -> usize {
        match self.mode {
            Mode::Normal => self.checks.iter().filter(|c| c.placement.liftable()).count(),
            Mode::Fast => {
                let bits: u32 = self.checks.iter().map(|c| 2 * c.placement.lift_bits()).sum();
                (bits / (2 * NORMAL_LIFT)) as usize
            }
        }
    }

    fn lift_low_bits(&self) -> u32 {
        match self.mode {
            Mode::Normal => LOW_BITS,
            Mode::Fast => 17 + self.checks.iter().map(|c| c.placement.lift_bits()).max().unwrap_or(0),
        }
    }

    pub fn decorator_bits(&self) -> f64 {
        self.decorators.iter().map(|d| d.bits(self.range)).sum()
    }

    pub fn stage(&self) -> Stage {
        match &self.job {
            Some(j) => j.stage(),
            None if self.world_seed().is_some() => Stage::Done,
            None => Stage::Idle,
        }
    }

    pub fn progress(&self) -> f32 {
        self.job.as_ref().map_or(0.0, |j| j.done as f32 / j.total.max(1) as f32)
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn step(&mut self, budget: u64) -> bool {
        self.advance(budget, 1)
    }

    pub fn step_parallel(&mut self, budget: u64, threads: usize) -> bool {
        self.advance(budget, threads.max(1))
    }

    fn advance(&mut self, budget: u64, threads: usize) -> bool {
        let Some(slice) = self.next_slice(budget) else {
            return self.job.is_some();
        };
        let Some(job) = self.job.as_ref() else {
            return false;
        };
        let found = self.scan_split(&job.kind, job.mode, slice.range.clone(), threads);
        self.complete(slice, &found);
        self.job.is_some()
    }

    pub fn task(&mut self) -> Option<Task> {
        if self.job.is_none() {
            self.plan();
        }
        let job = self.job.as_ref()?;
        Some(Task {
            id: job.id,
            kind: job.kind.clone(),
            mode: job.mode,
            structures: self.structures.clone(),
            checks: self.checks.clone(),
            decorators: self.decorators.clone(),
            range: self.range,
        })
    }

    pub fn next_slice(&mut self, budget: u64) -> Option<Slice> {
        if self.job.is_none() {
            self.plan();
        }
        let job = self.job.as_mut()?;
        if let Some(range) = job.returned.pop() {
            return Some(Slice { job: job.id, range });
        }
        if job.next >= job.total {
            return None;
        }
        let end = job.next.saturating_add((budget / job.cost()).max(1)).min(job.total);
        let range = job.next..end;
        job.next = end;
        Some(Slice { job: job.id, range })
    }

    pub fn complete(&mut self, slice: Slice, found: &[i64]) {
        let Some(job) = self.job.as_mut().filter(|j| j.id == slice.job) else {
            return;
        };
        job.found.extend_from_slice(found);
        job.done += slice.range.end - slice.range.start;
        if job.done >= job.total {
            let job = self.job.take().expect("checked above");
            self.finish(job);
            self.plan();
        }
    }

    pub fn requeue(&mut self, slice: Slice) {
        if let Some(job) = self.job.as_mut().filter(|j| j.id == slice.job) {
            job.returned.push(slice.range);
        }
    }

    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    fn scan_split(&self, kind: &JobKind, mode: Mode, range: Range<u64>, threads: usize) -> Vec<i64> {
        let len = range.end - range.start;
        if threads <= 1 || len < threads as u64 * 4096 {
            let mut out = Vec::new();
            self.scan(kind, mode, range, &mut out);
            return out;
        }
        let chunk = len.div_ceil(threads as u64);
        std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads as u64)
                .map(|t| {
                    let start = range.start + t * chunk;
                    let stop = (start + chunk).min(range.end);
                    s.spawn(move || {
                        let mut out = Vec::new();
                        if start < stop {
                            self.scan(kind, mode, start..stop, &mut out);
                        }
                        out
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().expect("seedcrack worker panicked")).collect()
        })
    }

    #[cfg(not(all(feature = "threads", not(target_arch = "wasm32"))))]
    fn scan_split(&self, kind: &JobKind, mode: Mode, range: Range<u64>, _threads: usize) -> Vec<i64> {
        let mut out = Vec::new();
        self.scan(kind, mode, range, &mut out);
        out
    }

    #[inline]
    fn test_structure_seed(&self, seed: i64) -> bool {
        self.checks.iter().all(|c| c.test(seed))
    }

    fn test_world_seed(&self, seed: i64, rand: &mut WorldgenRandom) -> bool {
        self.decorators.iter().all(|d| d.test(seed, self.range, rand)) && (self.floor.is_empty() || self.floor.test(seed))
    }

    fn scan(&self, kind: &JobKind, mode: Mode, range: Range<u64>, out: &mut Vec<i64>) {
        scan(kind, mode, &self.checks, &self.decorators, self.range, range, out);
    }

    fn finish(&mut self, job: Job) {
        let mut found = job.found;
        found.sort_unstable();
        found.dedup();
        match job.kind {
            JobKind::LiftLow { low_bits } => {
                self.events.push(Event::LiftedLowParts(found.len()));
                if found.is_empty() {
                    self.structure_seeds = Some(Vec::new());
                    self.events.push(Event::NoStructureSeed);
                } else {
                    self.start(JobKind::LiftHigh { low_bits, lows: found });
                }
            }
            JobKind::LiftHigh { .. } | JobKind::Pillars { .. } => {
                self.set_structure_seeds(found);
            }
            JobKind::WorldByHash { .. } => {
                if found.is_empty() {
                    self.hash_failed = true;
                    self.events.push(Event::HashMismatch);
                } else {
                    self.set_world_seeds(found);
                }
            }
            JobKind::WorldByDecorators { .. } => {
                if found.is_empty() {
                    self.world_seeds = Some(Vec::new());
                    self.events.push(Event::NoWorldSeed);
                } else {
                    self.set_world_seeds(found);
                }
            }
            JobKind::Bedrock { limit, search, .. } => {
                let band = search.band();
                let Some(slot) = nether_slot(band) else {
                    return;
                };
                if found.contains(&BEDROCK_OVERFLOW) || found.len() > limit {
                    self.band_results[slot] = BandResult::Overflow;
                    self.events.push(Event::BedrockOverflow(band));
                } else {
                    self.events.push(Event::BandSeeds(band, found.len()));
                    self.band_results[slot] = BandResult::Found(found);
                }
                self.join_bedrock();
            }
        }
    }

    fn set_structure_seeds(&mut self, seeds: Vec<i64>) {
        if seeds.is_empty() {
            self.events.push(Event::NoStructureSeed);
        } else {
            self.events.push(Event::StructureSeeds(seeds.clone()));
        }
        self.structure_seeds = Some(seeds);
        self.world_seeds = None;
    }

    fn set_world_seeds(&mut self, seeds: Vec<i64>) {
        self.events.push(Event::WorldSeeds(seeds.clone()));
        self.world_seeds = Some(seeds);
    }

    fn start(&mut self, kind: JobKind) {
        let job = Job::new(self.next_job, kind, self.mode);
        self.next_job += 1;
        self.events.push(Event::Started(job.stage()));
        self.job = Some(job);
    }

    fn plan(&mut self) {
        if self.job.is_some() {
            return;
        }

        if self.pillar_seeds.is_none() {
            if let Some(p) = &self.pillars {
                let seeds = p.candidates();
                if seeds.is_empty() {
                    self.events.push(Event::PillarsInconsistent);
                }
                self.events.push(Event::PillarSeeds(seeds.clone()));
                self.pillar_seeds = Some(seeds);
                self.dirty = true;
            }
        }

        if self.dirty {
            self.dirty = false;
            self.narrow();
        }

        if self.bedrock_active() {
            let ready = NETHER_BANDS.into_iter().find(|&band| {
                let slot = nether_slot(band).expect("nether band");
                self.nether[slot].len() >= BEDROCK_TARGET && self.band_results[slot] == BandResult::Pending
            });
            if let Some(band) = ready {
                let slot = nether_slot(band).expect("nether band");
                match JobKind::bedrock(band, self.nether[slot].clone(), self.survivor_limit) {
                    Some(kind) => {
                        self.start(kind);
                        return;
                    }
                    None => self.band_results[slot] = BandResult::Found(Vec::new()),
                }
            }
        }

        if self.conflict {
            return;
        }

        if self.structure_seeds.is_none() {
            let bits = self.structure_bits();
            if let Some(seeds) = self.pillar_seeds.as_ref().filter(|s| !s.is_empty() && s.len() <= MAX_PILLAR_SEEDS) {
                if bits >= PILLAR_BITS {
                    let seeds = seeds.clone();
                    self.start(JobKind::Pillars { seeds });
                    return;
                }
            }
            if bits >= LIFT_BITS && self.liftable_count() >= LIFT_COUNT {
                let low_bits = self.lift_low_bits();
                self.start(JobKind::LiftLow { low_bits });
            }
            return;
        }

        let structures = self.structure_seeds.clone().unwrap_or_default();
        if structures.is_empty() || structures.len() > MAX_STRUCTURE_SEEDS || self.world_seeds.is_some() {
            return;
        }
        if let (Some(hashed), false) = (self.hashed, self.hash_failed) {
            self.start(JobKind::WorldByHash { hashed, structures });
            return;
        }
        let wanted = 16.0 + (structures.len() as f64).log2() + DECORATOR_MARGIN;
        let by_decorators = !self.decorators.is_empty() && self.decorator_bits() >= wanted;
        if by_decorators || self.overworld.len() >= self.overworld_min {
            self.start(JobKind::world(structures, self.overworld.clone()));
        }
    }

    pub fn hints(&self) -> Vec<Hint> {
        if self.world_seed().is_some() {
            return Vec::new();
        }
        let structure_hints = |urgent_lift: bool| -> Vec<Hint> {
            let pillars_open = self.pillar_seeds.as_ref().is_none_or(|p| p.is_empty() || p.len() > MAX_PILLAR_SEEDS);
            FinderKind::ALL
                .into_iter()
                .filter_map(|finder| match finder.yields() {
                    Yields::Structure(set) => {
                        let p = set.placement();
                        let lift = p.liftable();
                        let rank = if urgent_lift && lift { 0 } else { 1 };
                        let why = if lift { "for lifting" } else { "more bits" };
                        Some(Hint { finder, bits: p.expected_bits(), why, rank })
                    }
                    Yields::Pillars if pillars_open => {
                        Some(Hint { finder, bits: 16.0, why: "End search", rank: 0 })
                    }
                    _ => None,
                })
                .collect()
        };
        let mut hints = match (&self.structure_seeds, &self.world_seeds) {
            (Some(s), _) if s.is_empty() => Vec::new(),
            (None, _) => structure_hints(self.liftable_count() < LIFT_COUNT),
            (Some(s), None) if s.len() > MAX_STRUCTURE_SEEDS => structure_hints(false),
            (Some(_), None) if self.hashed.is_some() && !self.hash_failed => Vec::new(),
            (Some(_), Some(w)) if w.is_empty() => Vec::new(),
            (Some(_), _) => FinderKind::ALL
                .into_iter()
                .filter_map(|finder| match finder.yields() {
                    Yields::Decorator(kind) => Some(Hint {
                        finder,
                        bits: kind.bits(self.range, true),
                        why: "world seed",
                        rank: 0,
                    }),
                    _ => None,
                })
                .collect(),
        };
        hints.sort_by(|a, b| a.rank.cmp(&b.rank).then(b.bits.total_cmp(&a.bits)));
        hints
    }

    fn narrow(&mut self) {
        let held = self.structure_seeds.as_ref().filter(|s| !self.conflict && !s.is_empty()).map(|seeds| {
            let pillar_seeds = self.usable_pillar_seeds();
            let kept: Vec<i64> =
                seeds.iter().copied().filter(|&s| self.fits_sightings(s, pillar_seeds.as_deref())).collect();
            (seeds.len(), kept)
        });
        if let Some((_, kept)) = held.filter(|(before, kept)| *before != kept.len()) {
            if kept.is_empty() && self.bedrock_structures.as_ref().is_some_and(|b| !b.is_empty()) {
                self.raise_conflict();
            } else {
                if kept.is_empty() {
                    self.events.push(Event::NoStructureSeed);
                } else {
                    self.events.push(Event::StructureSeeds(kept.clone()));
                }
                self.structure_seeds = Some(kept);
                self.world_seeds = None;
                self.hash_failed = false;
                if self.job.as_ref().is_some_and(|j| matches!(j.kind, JobKind::WorldByHash { .. } | JobKind::WorldByDecorators { .. })) {
                    self.job = None;
                }
            }
        }
        if let Some(worlds) = &self.world_seeds {
            if worlds.len() > 1 && (!self.decorators.is_empty() || !self.floor.is_empty()) {
                let mut rand = WorldgenRandom::new();
                let kept: Vec<i64> = worlds.iter().copied().filter(|&w| self.test_world_seed(w, &mut rand)).collect();
                if !kept.is_empty() && kept.len() != worlds.len() {
                    self.set_world_seeds(kept);
                }
            }
        }
    }
}

pub(crate) fn scan(
    kind: &JobKind,
    mode: Mode,
    checks: &[Check],
    decorators: &[DecoratorObservation],
    gen_range: GenRange,
    range: Range<u64>,
    out: &mut Vec<i64>,
) {
    let structure = |seed: i64| checks.iter().all(|c| c.test(seed));
    match kind {
        JobKind::LiftLow { low_bits } => {
            let lifts: Vec<&Check> = match mode {
                Mode::Normal => checks.iter().filter(|c| c.placement.liftable()).collect(),
                Mode::Fast => checks.iter().filter(|c| c.placement.lift_bits() > 0).collect(),
            };
            for low in range {
                let low = low as i64;
                if lifts.iter().all(|c| c.test_low(low, *low_bits)) {
                    out.push(low);
                }
            }
        }
        JobKind::LiftHigh { low_bits, lows } => {
            let low_bits = *low_bits;
            let high_bits = 48 - low_bits;
            let high_mask = (1u64 << high_bits) - 1;
            match mode {
                Mode::Normal => {
                    for k in range {
                        let low = lows[(k >> high_bits) as usize];
                        let high = (k & high_mask) as i64;
                        let seed = (high << low_bits) | low;
                        if structure(seed) {
                            out.push(seed);
                        }
                    }
                }
                Mode::Fast => {
                    let pick = prefilter_check(checks, low_bits);
                    let mut k = range.start;
                    while k < range.end {
                        let i = k >> high_bits;
                        let stop = ((i + 1) << high_bits).min(range.end);
                        let low = lows[i as usize];
                        let (h0, h1) = ((k & high_mask) as u32, (stop - (i << high_bits)) as u32);
                        let mut full = |h: u32| {
                            let seed = ((h as i64) << low_bits) | low;
                            if structure(seed) {
                                out.push(seed);
                            }
                        };
                        match pick {
                            Some(c) => Prefilter::new(low_bits, low, c.key, c.placement.limit(), c.offset).run(h0, h1, &mut full),
                            None => (h0..h1).for_each(&mut full),
                        }
                        k = stop;
                    }
                }
            }
        }
        JobKind::Pillars { seeds } => match mode {
            Mode::Normal => {
                for k in range {
                    let p = seeds[(k >> 32) as usize];
                    let seed = pillar::structure_seed_from_pillar(k as u32, p);
                    if structure(seed) {
                        out.push(seed);
                    }
                }
            }
            Mode::Fast => {
                let lifts: Vec<&Check> = checks.iter().filter(|c| c.placement.lift_bits() > 0).collect();
                let mut k = range.start;
                while k < range.end {
                    let p = seeds[(k >> 32) as usize];
                    let klo = (k >> 16) as u32 & 0xFFFF;
                    let stop = (((k >> 16) + 1) << 16).min(range.end);
                    if pillar_low_ok(&lifts, p, klo) {
                        for khi in (k & 0xFFFF) as u32..=((stop - 1) & 0xFFFF) as u32 {
                            let seed = pillar::structure_seed_from_pillar((khi << 16) | klo, p);
                            if structure(seed) {
                                out.push(seed);
                            }
                        }
                    }
                    k = stop;
                }
            }
        },
        JobKind::WorldByHash { hashed, structures } => {
            for k in range {
                let w = hash::with_upper(structures[(k >> 16) as usize], k as u16);
                if hash::obfuscate(w) == *hashed {
                    out.push(w);
                }
            }
        }
        JobKind::WorldByDecorators { structures, floor, .. } => {
            let mut rand = WorldgenRandom::new();
            for k in range {
                let w = hash::with_upper(structures[(k >> 16) as usize], k as u16);
                if decorators.iter().all(|d| d.test(w, gen_range, &mut rand)) && (floor.is_empty() || floor.test(w)) {
                    out.push(w);
                }
            }
        }
        JobKind::Bedrock { limit, search, .. } => {
            if search.scan_limited(range, out, (*limit).min(MAX_SURVIVORS)).is_err() {
                out.push(BEDROCK_OVERFLOW);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slice {
    pub job: u64,
    pub range: Range<u64>,
}

#[derive(Clone, Debug)]
pub struct Task {
    id: u64,
    kind: JobKind,
    mode: Mode,
    structures: Vec<StructureObservation>,
    checks: Vec<Check>,
    decorators: Vec<DecoratorObservation>,
    range: GenRange,
}

impl Task {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn stage(&self) -> Stage {
        self.kind.stage()
    }

    pub fn scan(&self, range: Range<u64>) -> Vec<i64> {
        let mut out = Vec::new();
        scan(&self.kind, self.mode, &self.checks, &self.decorators, self.range, range, &mut out);
        out
    }

    #[cfg(test)]
    pub(crate) fn parts(&self) -> (&JobKind, Mode) {
        (&self.kind, self.mode)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Vec::new();
        w.push(TASK_FORMAT);
        w.extend_from_slice(&self.id.to_le_bytes());
        w.push(match self.mode {
            Mode::Normal => 0,
            Mode::Fast => 1,
        });
        let longs = |w: &mut Vec<u8>, v: &[i64]| {
            w.extend_from_slice(&(v.len() as u32).to_le_bytes());
            for x in v {
                w.extend_from_slice(&x.to_le_bytes());
            }
        };
        match &self.kind {
            JobKind::LiftLow { low_bits } => {
                w.push(0);
                w.push(*low_bits as u8);
            }
            JobKind::LiftHigh { low_bits, lows } => {
                w.push(1);
                w.push(*low_bits as u8);
                longs(&mut w, lows);
            }
            JobKind::Pillars { seeds } => {
                w.push(2);
                w.extend_from_slice(&(seeds.len() as u32).to_le_bytes());
                for p in seeds {
                    w.extend_from_slice(&p.to_le_bytes());
                }
            }
            JobKind::WorldByHash { hashed, structures } => {
                w.push(3);
                w.extend_from_slice(&hashed.to_le_bytes());
                longs(&mut w, structures);
            }
            JobKind::WorldByDecorators { structures, overworld, .. } => {
                w.push(4);
                longs(&mut w, structures);
                write_cells(&mut w, overworld);
            }
            JobKind::Bedrock { cells, limit, search } => {
                w.push(5);
                w.push(nether_slot(search.band()).expect("bedrock jobs search a nether band") as u8);
                w.extend_from_slice(&(*limit as u32).to_le_bytes());
                write_cells(&mut w, cells);
            }
        }
        w.extend_from_slice(&self.range.min_y.to_le_bytes());
        w.extend_from_slice(&self.range.height.to_le_bytes());
        w.extend_from_slice(&(self.structures.len() as u32).to_le_bytes());
        for s in &self.structures {
            let set = StructureSet::ALL.iter().position(|x| *x == s.set).expect("every set is in ALL");
            w.push(set as u8);
            w.extend_from_slice(&s.chunk_x.to_le_bytes());
            w.extend_from_slice(&s.chunk_z.to_le_bytes());
        }
        w.extend_from_slice(&(self.decorators.len() as u32).to_le_bytes());
        for d in &self.decorators {
            w.push(DecoratorKind::ALL.iter().position(|k| *k == d.kind).expect("every kind is in ALL") as u8);
            for v in [d.x, d.y, d.z, d.height_above_surface.unwrap_or(i32::MIN)] {
                w.extend_from_slice(&v.to_le_bytes());
            }
        }
        w
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Task> {
        let mut r = Reader(bytes);
        if r.u8()? != TASK_FORMAT {
            return None;
        }
        let id = r.u64()?;
        let mode = match r.u8()? {
            0 => Mode::Normal,
            1 => Mode::Fast,
            _ => return None,
        };
        let low_bits = |r: &mut Reader| -> Option<u32> { r.u8().map(u32::from).filter(|b| (17..=31).contains(b)) };
        let longs = |r: &mut Reader| -> Option<Vec<i64>> { (0..r.u32()?).map(|_| r.i64()).collect() };
        let kind = match r.u8()? {
            0 => JobKind::LiftLow { low_bits: low_bits(&mut r)? },
            1 => {
                let low_bits = low_bits(&mut r)?;
                JobKind::LiftHigh { low_bits, lows: longs(&mut r)? }
            }
            2 => JobKind::Pillars { seeds: (0..r.u32()?).map(|_| r.u16()).collect::<Option<_>>()? },
            3 => {
                let hashed = r.i64()?;
                JobKind::WorldByHash { hashed, structures: longs(&mut r)? }
            }
            4 => {
                let structures = longs(&mut r)?;
                JobKind::world(structures, read_cells(&mut r, Band::OverworldFloor)?)
            }
            5 => {
                let band = *NETHER_BANDS.get(r.u8()? as usize)?;
                let limit = r.u32()? as usize;
                JobKind::bedrock(band, read_cells(&mut r, band)?, limit)?
            }
            _ => return None,
        };
        let range = GenRange { min_y: r.i32()?, height: r.i32()? };
        let structures: Vec<StructureObservation> = (0..r.u32()?)
            .map(|_| {
                Some(StructureObservation {
                    set: *StructureSet::ALL.get(r.u8()? as usize)?,
                    chunk_x: r.i32()?,
                    chunk_z: r.i32()?,
                })
            })
            .collect::<Option<_>>()?;
        let decorators: Vec<DecoratorObservation> = (0..r.u32()?)
            .map(|_| {
                let kind = *DecoratorKind::ALL.get(r.u8()? as usize)?;
                let (x, y, z, h) = (r.i32()?, r.i32()?, r.i32()?, r.i32()?);
                Some(DecoratorObservation { kind, x, y, z, height_above_surface: (h != i32::MIN).then_some(h) })
            })
            .collect::<Option<_>>()?;
        if !r.0.is_empty() {
            return None;
        }
        Some(Task::with(id, kind, mode, structures, decorators, range))
    }

    pub(crate) fn with(
        id: u64,
        kind: JobKind,
        mode: Mode,
        structures: Vec<StructureObservation>,
        decorators: Vec<DecoratorObservation>,
        range: GenRange,
    ) -> Task {
        let mut checks: Vec<Check> = structures.iter().map(Check::new).collect();
        checks.sort_by(|a, b| b.bits.total_cmp(&a.bits));
        Task { id, kind, mode, structures, checks, decorators, range }
    }
}

const TASK_FORMAT: u8 = 3;

fn write_cells(w: &mut Vec<u8>, cells: &[BedrockCell]) {
    w.extend_from_slice(&(cells.len() as u32).to_le_bytes());
    for c in cells {
        for v in [c.x(), c.y(), c.z()] {
            w.extend_from_slice(&v.to_le_bytes());
        }
        w.push(u8::from(c.bedrock()));
    }
}

fn read_cells(r: &mut Reader, band: Band) -> Option<Vec<BedrockCell>> {
    (0..r.u32()?)
        .map(|_| {
            let (x, y, z) = (r.i32()?, r.i32()?, r.i32()?);
            let bedrock = match r.u8()? {
                0 => false,
                1 => true,
                _ => return None,
            };
            BedrockCell::new(band, x, y, z, bedrock)
        })
        .collect()
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_le_bytes)
    }
    fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }
    fn i32(&mut self) -> Option<i32> {
        self.take().map(i32::from_le_bytes)
    }
    fn u64(&mut self) -> Option<u64> {
        self.take().map(u64::from_le_bytes)
    }
    fn i64(&mut self) -> Option<i64> {
        self.take().map(i64::from_le_bytes)
    }
}
