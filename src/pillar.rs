use crate::lcg::{JavaRandom, Lcg, MASK, MULTIPLIER};

pub const CENTERS: [(i32, i32); 10] = [
    (42, 0),
    (33, 24),
    (12, 39),
    (-13, 39),
    (-34, 24),
    (-42, -1),
    (-34, -25),
    (-13, -40),
    (12, -40),
    (33, -25),
];

pub const BASE_HEIGHT: i32 = 76;
pub const HEIGHT_STEP: i32 = 3;

pub fn pillar_seed(structure_seed: i64) -> u16 {
    (JavaRandom::new(structure_seed).next_long() & 0xFFFF) as u16
}

pub fn sizes(pillar_seed: u16) -> [u8; 10] {
    let mut r = JavaRandom::new(pillar_seed as i64);
    let mut sizes = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    for i in (2..=10).rev() {
        let swap_to = r.next_int(i as i32) as usize;
        sizes.swap(i - 1, swap_to);
    }
    sizes
}

pub fn heights(pillar_seed: u16) -> [i32; 10] {
    sizes(pillar_seed).map(|s| BASE_HEIGHT + HEIGHT_STEP * s as i32)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PillarObservation {
    pub heights: [Option<i32>; 10],
}

impl PillarObservation {
    pub fn known(&self) -> usize {
        self.heights.iter().filter(|h| h.is_some()).count()
    }

    pub fn matches(&self, pillar_seed: u16) -> bool {
        let h = heights(pillar_seed);
        self.heights.iter().zip(h).all(|(seen, real)| seen.is_none_or(|s| s == real))
    }

    pub fn candidates(&self) -> Vec<u16> {
        (0..=u16::MAX).filter(|&p| self.matches(p)).collect()
    }

    pub fn merge(&mut self, other: &PillarObservation) -> bool {
        let conflict = self.heights.iter().zip(other.heights).any(|(mine, theirs)| matches!((*mine, theirs), (Some(a), Some(b)) if a != b));
        if conflict {
            return false;
        }
        for (mine, theirs) in self.heights.iter_mut().zip(other.heights) {
            if mine.is_none() {
                *mine = theirs;
            }
        }
        true
    }
}

const BACK_TWO: Lcg = Lcg::JAVA.combine(-2);

#[inline]
pub fn structure_seed_from_pillar(partial: u32, pillar_seed: u16) -> i64 {
    let partial = partial as i64;
    let state = ((partial & 0xFFFF_0000) << 16) | ((pillar_seed as i64) << 16) | (partial & 0xFFFF);
    (BACK_TWO.next_seed(state) ^ MULTIPLIER) & MASK
}
