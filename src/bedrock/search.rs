use std::fmt;
use std::ops::Range;

use super::wide::Frontier;

const ADDEND: u64 = 0xB;
const NARROW_PERCENT_LIMIT: u64 = 41;
const PAIRED_CELL_LIMIT: usize = 3000;
const PAIR_LIMIT: usize = 30_000;
const PAIRS_PER_DEPTH: usize = 16;
const DIFFERING_BIT_LIMITS: [u32; 4] = [16, 18, 20, 22];
const WEIGHT_BASES: [f64; 3] = [1.5, 2.0, 3.0];
const FIXED_POINT_BITS: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Modulus {
    pub bits: u32,
    pub multiplier: u64,
}

impl Modulus {
    pub const JAVA: Modulus = Modulus { bits: 48, multiplier: 0x5_DEEC_E66D };

    #[cfg(test)]
    pub fn reduced(bits: u32) -> Modulus {
        assert!((8..=48).contains(&bits));
        Modulus { bits, multiplier: (Modulus::JAVA.multiplier >> (48 - bits)) | 1 }
    }

    pub fn size(self) -> u64 {
        1 << self.bits
    }

    pub fn mask(self) -> u64 {
        self.size() - 1
    }

    #[inline]
    pub fn state(self, seed: u64, hash: u64) -> u64 {
        ((seed ^ hash) & self.mask())
            .wrapping_mul(self.multiplier)
            .wrapping_add(ADDEND)
            & self.mask()
    }

    #[cfg(test)]
    pub fn seed_for_state(self, state: u64, hash: u64) -> u64 {
        let inverse = self.multiplier_inverse();
        (state.wrapping_sub(ADDEND).wrapping_mul(inverse) ^ hash) & self.mask()
    }

    #[cfg(test)]
    fn multiplier_inverse(self) -> u64 {
        let mut inverse = self.multiplier;
        for _ in 0..6 {
            inverse = inverse.wrapping_mul(2u64.wrapping_sub(self.multiplier.wrapping_mul(inverse)));
        }
        inverse
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Constraint {
    pub hash: u64,
    pub lo: u64,
    pub hi: u64,
}

impl Constraint {
    pub fn new(modulus: Modulus, hash: u64, accepted: Range<u64>) -> Constraint {
        assert!(accepted.start < accepted.end && accepted.end <= modulus.size());
        Constraint { hash: hash & modulus.mask(), lo: accepted.start, hi: accepted.end }
    }

    pub fn width(&self) -> u64 {
        self.hi - self.lo
    }

    #[inline]
    pub fn holds(&self, modulus: Modulus, seed: u64) -> bool {
        let state = modulus.state(seed, self.hash);
        state >= self.lo && state < self.hi
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overflow;

impl fmt::Display for Overflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "more seeds survive than the survivor limit allows")
    }
}

impl std::error::Error for Overflow {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lanes {
    #[cfg_attr(not(test), allow(dead_code))]
    Scalar,
    Widest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairTest {
    pub hash_i: u64,
    pub hash_j: u64,
    pub offset: u64,
    pub span: u64,
}

impl PairTest {
    fn between(modulus: Modulus, a: &Constraint, b: &Constraint) -> PairTest {
        let window_start = a.lo.wrapping_sub(b.hi - 1) & modulus.mask();
        PairTest {
            hash_i: a.hash,
            hash_j: b.hash,
            offset: window_start.wrapping_neg() & modulus.mask(),
            span: a.width() + b.width() - 2,
        }
    }

    #[inline]
    pub fn passes(&self, multiplier: u64, mask: u64, seed: u64) -> bool {
        ((seed ^ self.hash_i)
            .wrapping_sub(seed ^ self.hash_j)
            .wrapping_mul(multiplier)
            .wrapping_add(self.offset)
            & mask)
            <= self.span
    }
}

struct Candidate {
    test: PairTest,
    differing: u64,
    information: i64,
    width_sum: u64,
    rank: (usize, usize),
}

struct Plan {
    order: Vec<u32>,
    completed: Vec<Vec<PairTest>>,
    modelled_nodes: f64,
}

#[derive(Clone, Debug)]
pub struct Search {
    modulus: Modulus,
    order: Vec<u32>,
    completed: Vec<Vec<PairTest>>,
    constraints: Vec<Constraint>,
    unit_bits: u32,
}

pub struct Sink<'a> {
    out: &'a mut Vec<u64>,
    start: usize,
    limit: usize,
}

impl Sink<'_> {
    #[inline]
    pub fn push(&mut self, seed: u64) -> Result<(), Overflow> {
        if self.out.len() - self.start >= self.limit {
            return Err(Overflow);
        }
        self.out.push(seed);
        Ok(())
    }
}

impl Search {
    pub fn new(modulus: Modulus, constraints: &[Constraint]) -> Search {
        let mut sorted = constraints.to_vec();
        sorted.sort_by_key(|c| (c.width(), c.hash, c.lo, c.hi));
        sorted.dedup();
        let candidates = candidates(modulus, &sorted);
        let mut best: Option<Plan> = None;
        for &limit in &DIFFERING_BIT_LIMITS {
            let allowed = candidates.partition_point(|c| c.differing.count_ones() <= limit);
            let chosen = &candidates[..allowed.min(PAIR_LIMIT)];
            for &base in &WEIGHT_BASES {
                let plan = plan(modulus, chosen, base);
                if best.as_ref().is_none_or(|b| plan.modelled_nodes < b.modelled_nodes) {
                    best = Some(plan);
                }
            }
        }
        let best = best.expect("at least one plan variant");
        Search {
            modulus,
            order: best.order,
            completed: best.completed,
            constraints: sorted,
            unit_bits: modulus.bits / 2,
        }
    }

    pub fn modulus(&self) -> Modulus {
        self.modulus
    }

    pub fn total(&self) -> u64 {
        1 << self.unit_bits
    }

    pub fn order(&self) -> &[u32] {
        &self.order
    }

    pub fn completed(&self, depth: usize) -> &[PairTest] {
        &self.completed[depth]
    }

    #[cfg(test)]
    pub fn pair_test_count(&self) -> usize {
        self.completed.iter().map(Vec::len).sum()
    }

    #[inline]
    pub fn passes_depth(&self, depth: usize, seed: u64) -> bool {
        let (multiplier, mask) = (self.modulus.multiplier, self.modulus.mask());
        self.completed[depth].iter().all(|t| t.passes(multiplier, mask, seed))
    }

    #[inline]
    pub fn holds_everywhere(&self, seed: u64) -> bool {
        self.constraints.iter().all(|c| c.holds(self.modulus, seed))
    }

    pub fn scan(&self, units: Range<u64>, out: &mut Vec<u64>, limit: usize, lanes: Lanes) -> Result<(), Overflow> {
        let wanted = units.start.min(self.total())..units.end.min(self.total());
        let start = out.len();
        if wanted.is_empty() {
            return Ok(());
        }
        let mut sink = Sink { out, start, limit };
        let mut frontier = (lanes == Lanes::Widest && super::wide::available())
            .then(|| Frontier::new(self, self.unit_bits as usize));
        let mut result = self.descend_units(0, 0, 0, &wanted, &mut frontier, &mut sink);
        if let (Ok(()), Some(frontier)) = (&result, frontier.as_mut()) {
            result = frontier.finish(self, &mut sink);
        }
        match result {
            Ok(()) => {
                out[start..].sort_unstable();
                Ok(())
            }
            Err(overflow) => {
                out.truncate(start);
                Err(overflow)
            }
        }
    }

    fn descend_units(
        &self,
        depth: usize,
        seed: u64,
        first_unit: u64,
        wanted: &Range<u64>,
        frontier: &mut Option<Frontier>,
        sink: &mut Sink,
    ) -> Result<(), Overflow> {
        if !self.passes_depth(depth, seed) {
            return Ok(());
        }
        if depth == self.unit_bits as usize {
            return match frontier {
                Some(frontier) => frontier.push(self, seed, sink),
                None => self.descend_scalar(depth, seed, sink),
            };
        }
        let half = 1u64 << (self.unit_bits as usize - 1 - depth);
        let bit = 1u64 << self.order[depth];
        for (child, child_first) in [(seed, first_unit), (seed | bit, first_unit + half)] {
            if child_first < wanted.end && child_first + half > wanted.start {
                self.descend_units(depth + 1, child, child_first, wanted, frontier, sink)?;
            }
        }
        Ok(())
    }

    fn descend_scalar(&self, depth: usize, seed: u64, sink: &mut Sink) -> Result<(), Overflow> {
        if depth == self.modulus.bits as usize {
            if self.holds_everywhere(seed) {
                sink.push(seed)?;
            }
            return Ok(());
        }
        let bit = 1u64 << self.order[depth];
        for child in [seed, seed | bit] {
            if self.passes_depth(depth + 1, child) {
                self.descend_scalar(depth + 1, child, sink)?;
            }
        }
        Ok(())
    }
}

fn candidates(modulus: Modulus, sorted: &[Constraint]) -> Vec<Candidate> {
    let size = modulus.size();
    let differing_bit_limit = DIFFERING_BIT_LIMITS[DIFFERING_BIT_LIMITS.len() - 1];
    let narrow_limit = size / 100 * NARROW_PERCENT_LIMIT;
    let paired = sorted
        .iter()
        .take(PAIRED_CELL_LIMIT)
        .take_while(|c| c.width() <= narrow_limit)
        .count();
    let mut candidates = Vec::new();
    for i in 0..paired {
        for j in i + 1..paired {
            let (a, b) = (&sorted[i], &sorted[j]);
            let differing = (a.hash ^ b.hash) & modulus.mask();
            if differing == 0 || differing.count_ones() > differing_bit_limit {
                continue;
            }
            let width_sum = a.width() + b.width();
            if width_sum >= size - size / 16 {
                continue;
            }
            candidates.push(Candidate {
                test: PairTest::between(modulus, a, b),
                differing,
                information: information(modulus, width_sum),
                width_sum,
                rank: (i, j),
            });
        }
    }
    candidates.sort_by_key(|c| (c.differing.count_ones(), c.width_sum, c.rank));
    candidates
}

fn plan(modulus: Modulus, candidates: &[Candidate], base: f64) -> Plan {
    let bits = modulus.bits as usize;

    let mut weights = vec![1.0f64; bits + 2];
    for r in 2..weights.len() {
        weights[r] = weights[r - 1] / base;
    }
    let mut remaining: Vec<u32> = candidates.iter().map(|c| c.differing.count_ones()).collect();
    let mut fixed = 0u64;
    let mut order = Vec::with_capacity(bits);
    for _ in 0..bits {
        let mut score = vec![0.0f64; bits];
        for (c, &left) in candidates.iter().zip(&remaining) {
            if left == 0 {
                continue;
            }
            let value = c.information as f64 * weights[left as usize];
            let mut open = c.differing & !fixed;
            while open != 0 {
                score[open.trailing_zeros() as usize] += value;
                open &= open - 1;
            }
        }
        let mut best: Option<usize> = None;
        for b in 0..bits {
            if fixed >> b & 1 == 0 && best.is_none_or(|x| score[b] > score[x]) {
                best = Some(b);
            }
        }
        let chosen = best.expect("an unfixed bit remains");
        order.push(chosen as u32);
        fixed |= 1 << chosen;
        for (c, left) in candidates.iter().zip(remaining.iter_mut()) {
            if *left > 0 && c.differing >> chosen & 1 == 1 {
                *left -= 1;
            }
        }
    }

    let mut position = vec![0usize; bits];
    for (depth, &b) in order.iter().enumerate() {
        position[b as usize] = depth;
    }
    let mut strongest_first: Vec<&Candidate> = candidates.iter().collect();
    strongest_first.sort_by_key(|c| (std::cmp::Reverse(c.information), c.width_sum, c.rank));
    let mut completed: Vec<Vec<PairTest>> = vec![Vec::new(); bits + 1];
    let mut completed_information = vec![0i64; bits + 1];
    for c in strongest_first {
        let mut depth = 0;
        let mut open = c.differing;
        while open != 0 {
            depth = depth.max(position[open.trailing_zeros() as usize] + 1);
            open &= open - 1;
        }
        if completed[depth].len() < PAIRS_PER_DEPTH {
            completed[depth].push(c.test);
            completed_information[depth] += c.information;
        }
    }

    let unit = (1i64 << FIXED_POINT_BITS) as f64;
    let mut log_nodes = 0.0f64;
    let mut modelled_nodes = 0.0f64;
    for &info in &completed_information {
        log_nodes -= info as f64 / unit;
        modelled_nodes += exp2_approx(log_nodes.max(0.0));
        log_nodes = (log_nodes + 1.0).max(0.0);
    }
    Plan { order, completed, modelled_nodes }
}

fn information(modulus: Modulus, width_sum: u64) -> i64 {
    ((modulus.bits as i64) << FIXED_POINT_BITS) - log2_fixed(width_sum)
}

fn log2_fixed(x: u64) -> i64 {
    let whole = 63 - x.leading_zeros();
    let one: u128 = 1 << 63;
    let mut mantissa: u128 = (x as u128) << (63 - whole);
    let mut result = (whole as i64) << FIXED_POINT_BITS;
    for i in 1..=FIXED_POINT_BITS {
        mantissa = mantissa * mantissa >> 63;
        if mantissa >= one << 1 {
            mantissa >>= 1;
            result |= 1 << (FIXED_POINT_BITS - i);
        }
    }
    result
}

fn exp2_approx(x: f64) -> f64 {
    let whole = x.floor();
    let fraction = x - whole;
    let power = f64::from_bits(((1023 + whole as i64) as u64) << 52);
    power * (1.0 + fraction * (0.6565 + 0.3435 * fraction))
}
