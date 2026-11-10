use std::ops::Range;

use crate::decorator::{DecoratorKind, DecoratorObservation};
use crate::finders::{FinderKind, Yields};
use crate::hash;
use crate::pillar::{self, PillarObservation};
use crate::placement::{Placement, StructureSet};
use crate::version::{GenRange, Version};
use crate::world::{Observation, StructureObservation};
use crate::xoroshiro::WorldgenRandom;
use crate::lcg::JavaRandom;

pub const PILLAR_BITS: f64 = 32.0;
pub const MAX_PILLAR_SEEDS: usize = 16;
pub const LIFT_BITS: f64 = 40.0;
pub const LIFT_COUNT: usize = 4;
pub const MAX_STRUCTURE_SEEDS: usize = 1024;
pub const DECORATOR_MARGIN: f64 = 4.0;

const LOW_BITS: u32 = 19;
const HIGH_BITS: u32 = 48 - LOW_BITS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Idle,
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hint {
    pub finder: FinderKind,
    pub bits: f64,
    pub why: &'static str,
    pub rank: u8,
}

#[derive(Clone, Debug)]
enum JobKind {
    LiftLow,
    LiftHigh { lows: Vec<i64> },
    Pillars { seeds: Vec<u16> },
    WorldByHash { hashed: i64, structures: Vec<i64> },
    WorldByDecorators { structures: Vec<i64> },
}

#[derive(Clone, Debug)]
struct Job {
    id: u64,
    kind: JobKind,
    next: u64,
    returned: Vec<Range<u64>>,
    done: u64,
    total: u64,
    found: Vec<i64>,
}

impl Job {
    fn new(id: u64, kind: JobKind) -> Job {
        let total = match &kind {
            JobKind::LiftLow => 1 << LOW_BITS,
            JobKind::LiftHigh { lows } => (lows.len() as u64) << HIGH_BITS,
            JobKind::Pillars { seeds } => (seeds.len() as u64) << 32,
            JobKind::WorldByHash { structures, .. } | JobKind::WorldByDecorators { structures } => {
                (structures.len() as u64) << 16
            }
        };
        Job { id, kind, next: 0, returned: Vec::new(), done: 0, total, found: Vec::new() }
    }

    fn cost(&self) -> u64 {
        self.kind.cost()
    }

    fn stage(&self) -> Stage {
        self.kind.stage()
    }
}

impl JobKind {
    fn cost(&self) -> u64 {
        match self {
            JobKind::WorldByHash { .. } => 32,
            JobKind::WorldByDecorators { .. } => 4,
            _ => 1,
        }
    }

    fn stage(&self) -> Stage {
        match self {
            JobKind::LiftLow => Stage::LiftLow,
            JobKind::LiftHigh { .. } => Stage::LiftHigh,
            JobKind::Pillars { .. } => Stage::Pillars,
            JobKind::WorldByHash { .. } => Stage::WorldByHash,
            JobKind::WorldByDecorators { .. } => Stage::WorldByDecorators,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Check {
    placement: Placement,
    chunk: (i32, i32),
    region: (i32, i32),
    offset: (i32, i32),
    bits: f64,
}

impl Check {
    fn new(o: &StructureObservation) -> Check {
        let placement = o.set.placement();
        let region = placement.region_of(o.chunk_x, o.chunk_z);
        let offset = (o.chunk_x - region.0 * placement.spacing, o.chunk_z - region.1 * placement.spacing);
        Check { placement, chunk: (o.chunk_x, o.chunk_z), region, offset, bits: placement.bits(o.chunk_x, o.chunk_z) }
    }

    #[inline]
    fn test(&self, seed: i64) -> bool {
        self.placement.region_offset(seed, self.region.0, self.region.1) == self.offset
            && self.placement.passes_frequency(seed, self.chunk.0, self.chunk.1)
            && match self.placement.exclusion {
                None => true,
                Some(e) => !e.other.placement().has_structure_chunk_in_range(seed, self.chunk.0, self.chunk.1, e.chunk_count),
            }
    }

    #[inline]
    fn test_low(&self, low: i64) -> bool {
        let mut r = JavaRandom::new(0);
        r.set_large_feature_with_salt(low, self.region.0, self.region.1, self.placement.salt);
        let limit = self.placement.limit();
        r.next_int(limit) % 4 == self.offset.0 % 4 && r.next_int(limit) % 4 == self.offset.1 % 4
    }
}

#[derive(Clone, Debug)]
pub struct Cracker {
    version: Version,
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
}

impl Cracker {
    pub fn new(version: Version) -> Cracker {
        Cracker {
            version,
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

    pub fn add(&mut self, observation: Observation) -> bool {
        let new = match observation {
            Observation::Structure(s) => {
                if self.structures.contains(&s) {
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
        };
        if new {
            self.dirty = true;
        }
        new
    }

    pub fn clear(&mut self) {
        *self = Cracker { range: self.range, next_job: self.next_job, ..Cracker::new(self.version) };
    }

    pub fn restart(&mut self) {
        self.pillar_seeds = None;
        self.structure_seeds = None;
        self.world_seeds = None;
        self.hash_failed = false;
        self.job = None;
        self.dirty = true;
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
        self.checks.iter().filter(|c| c.placement.liftable()).count()
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
        let found = self.scan_split(&job.kind, slice.range.clone(), threads);
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
    fn scan_split(&self, kind: &JobKind, range: Range<u64>, threads: usize) -> Vec<i64> {
        let len = range.end - range.start;
        if threads <= 1 || len < threads as u64 * 4096 {
            let mut out = Vec::new();
            self.scan(kind, range, &mut out);
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
                            self.scan(kind, start..stop, &mut out);
                        }
                        out
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().expect("seedcrack worker panicked")).collect()
        })
    }

    #[cfg(not(all(feature = "threads", not(target_arch = "wasm32"))))]
    fn scan_split(&self, kind: &JobKind, range: Range<u64>, _threads: usize) -> Vec<i64> {
        let mut out = Vec::new();
        self.scan(kind, range, &mut out);
        out
    }

    #[inline]
    fn test_structure_seed(&self, seed: i64) -> bool {
        self.checks.iter().all(|c| c.test(seed))
    }

    fn test_world_seed(&self, seed: i64, rand: &mut WorldgenRandom) -> bool {
        self.decorators.iter().all(|d| d.test(seed, self.range, rand))
    }

    fn scan(&self, kind: &JobKind, range: Range<u64>, out: &mut Vec<i64>) {
        scan(kind, &self.checks, &self.decorators, self.range, range, out);
    }

    fn finish(&mut self, job: Job) {
        let mut found = job.found;
        found.sort_unstable();
        found.dedup();
        match job.kind {
            JobKind::LiftLow => {
                self.events.push(Event::LiftedLowParts(found.len()));
                if found.is_empty() {
                    self.structure_seeds = Some(Vec::new());
                    self.events.push(Event::NoStructureSeed);
                } else {
                    self.start(JobKind::LiftHigh { lows: found });
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
        let job = Job::new(self.next_job, kind);
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
                self.start(JobKind::LiftLow);
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
        if !self.decorators.is_empty() && self.decorator_bits() >= wanted {
            self.start(JobKind::WorldByDecorators { structures });
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
        if let Some(seeds) = &self.structure_seeds {
            if !seeds.is_empty() {
                let pillar_seeds = self.pillar_seeds.clone().filter(|p| !p.is_empty());
                let kept: Vec<i64> = seeds
                    .iter()
                    .copied()
                    .filter(|&s| self.test_structure_seed(s))
                    .filter(|&s| pillar_seeds.as_ref().is_none_or(|p| p.contains(&pillar::pillar_seed(s))))
                    .collect();
                if kept.len() != seeds.len() {
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
        }
        if let Some(worlds) = &self.world_seeds {
            if worlds.len() > 1 && !self.decorators.is_empty() {
                let mut rand = WorldgenRandom::new();
                let kept: Vec<i64> = worlds.iter().copied().filter(|&w| self.test_world_seed(w, &mut rand)).collect();
                if !kept.is_empty() && kept.len() != worlds.len() {
                    self.set_world_seeds(kept);
                }
            }
        }
    }
}

fn scan(
    kind: &JobKind,
    checks: &[Check],
    decorators: &[DecoratorObservation],
    gen_range: GenRange,
    range: Range<u64>,
    out: &mut Vec<i64>,
) {
    let structure = |seed: i64| checks.iter().all(|c| c.test(seed));
    match kind {
        JobKind::LiftLow => {
            let lifts: Vec<&Check> = checks.iter().filter(|c| c.placement.liftable()).collect();
            for low in range {
                let low = low as i64;
                if lifts.iter().all(|c| c.test_low(low)) {
                    out.push(low);
                }
            }
        }
        JobKind::LiftHigh { lows } => {
            for k in range {
                let low = lows[(k >> HIGH_BITS) as usize];
                let high = (k & ((1 << HIGH_BITS) - 1)) as i64;
                let seed = (high << LOW_BITS) | low;
                if structure(seed) {
                    out.push(seed);
                }
            }
        }
        JobKind::Pillars { seeds } => {
            for k in range {
                let p = seeds[(k >> 32) as usize];
                let seed = pillar::structure_seed_from_pillar(k as u32, p);
                if structure(seed) {
                    out.push(seed);
                }
            }
        }
        JobKind::WorldByHash { hashed, structures } => {
            for k in range {
                let w = hash::with_upper(structures[(k >> 16) as usize], k as u16);
                if hash::obfuscate(w) == *hashed {
                    out.push(w);
                }
            }
        }
        JobKind::WorldByDecorators { structures } => {
            let mut rand = WorldgenRandom::new();
            for k in range {
                let w = hash::with_upper(structures[(k >> 16) as usize], k as u16);
                if decorators.iter().all(|d| d.test(w, gen_range, &mut rand)) {
                    out.push(w);
                }
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
        scan(&self.kind, &self.checks, &self.decorators, self.range, range, &mut out);
        out
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Vec::new();
        w.push(TASK_FORMAT);
        w.extend_from_slice(&self.id.to_le_bytes());
        let longs = |w: &mut Vec<u8>, v: &[i64]| {
            w.extend_from_slice(&(v.len() as u32).to_le_bytes());
            for x in v {
                w.extend_from_slice(&x.to_le_bytes());
            }
        };
        match &self.kind {
            JobKind::LiftLow => w.push(0),
            JobKind::LiftHigh { lows } => {
                w.push(1);
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
            JobKind::WorldByDecorators { structures } => {
                w.push(4);
                longs(&mut w, structures);
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
        let longs = |r: &mut Reader| -> Option<Vec<i64>> { (0..r.u32()?).map(|_| r.i64()).collect() };
        let kind = match r.u8()? {
            0 => JobKind::LiftLow,
            1 => JobKind::LiftHigh { lows: longs(&mut r)? },
            2 => JobKind::Pillars { seeds: (0..r.u32()?).map(|_| r.u16()).collect::<Option<_>>()? },
            3 => {
                let hashed = r.i64()?;
                JobKind::WorldByHash { hashed, structures: longs(&mut r)? }
            }
            4 => JobKind::WorldByDecorators { structures: longs(&mut r)? },
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
        let mut checks: Vec<Check> = structures.iter().map(Check::new).collect();
        checks.sort_by(|a, b| b.bits.total_cmp(&a.bits));
        Some(Task { id, kind, structures, checks, decorators, range })
    }
}

const TASK_FORMAT: u8 = 1;

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
