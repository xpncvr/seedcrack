use sha2::{Digest, Sha256};

pub fn obfuscate(seed: i64) -> i64 {
    let digest = Sha256::digest(seed.to_le_bytes());
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    i64::from_le_bytes(first)
}

#[inline]
pub const fn with_upper(structure_seed: i64, upper: u16) -> i64 {
    ((upper as i64) << 48) | (structure_seed & crate::lcg::MASK)
}

pub fn world_seeds_from_hash(structure_seed: i64, hashed: i64) -> Vec<i64> {
    (0..=u16::MAX)
        .map(|u| with_upper(structure_seed, u))
        .filter(|&w| obfuscate(w) == hashed)
        .collect()
}
