use super::search::{Overflow, Search, Sink};

#[cfg(target_arch = "x86_64")]
pub fn available() -> bool {
    std::arch::is_x86_feature_detected!("avx512f") && std::arch::is_x86_feature_detected!("avx512dq")
}

#[cfg(not(target_arch = "x86_64"))]
pub fn available() -> bool {
    false
}

#[cfg(not(target_arch = "x86_64"))]
pub struct Frontier;

#[cfg(not(target_arch = "x86_64"))]
impl Frontier {
    pub fn new(_: &Search, _: usize) -> Frontier {
        unreachable!("wide lanes are never reported available off x86_64")
    }

    pub fn push(&mut self, _: &Search, _: u64, _: &mut Sink) -> Result<(), Overflow> {
        unreachable!("wide lanes are never reported available off x86_64")
    }

    pub fn finish(&mut self, _: &Search, _: &mut Sink) -> Result<(), Overflow> {
        unreachable!("wide lanes are never reported available off x86_64")
    }
}

#[cfg(target_arch = "x86_64")]
const BATCH: usize = 512;
#[cfg(target_arch = "x86_64")]
const LANES: usize = 8;

#[cfg(target_arch = "x86_64")]
pub struct Frontier {
    depth: usize,
    pending: Vec<u64>,
    stack: Vec<Vec<u64>>,
}

#[cfg(target_arch = "x86_64")]
impl Frontier {
    pub fn new(search: &Search, depth: usize) -> Frontier {
        assert!(available());
        let levels = search.modulus().bits as usize - depth;
        Frontier {
            depth,
            pending: Vec::with_capacity(BATCH),
            stack: (0..levels).map(|_| vec![0u64; 2 * BATCH + LANES]).collect(),
        }
    }

    pub fn push(&mut self, search: &Search, seed: u64, sink: &mut Sink) -> Result<(), Overflow> {
        self.pending.push(seed);
        if self.pending.len() == BATCH {
            return self.finish(search, sink);
        }
        Ok(())
    }

    pub fn finish(&mut self, search: &Search, sink: &mut Sink) -> Result<(), Overflow> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let result = unsafe { expand(search, self.depth, &self.pending, &mut self.stack, sink) };
        self.pending.clear();
        result
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512dq")]
unsafe fn expand(
    search: &Search,
    depth: usize,
    batch: &[u64],
    stack: &mut [Vec<u64>],
    sink: &mut Sink,
) -> Result<(), Overflow> {
    if depth == search.modulus().bits as usize {
        for &seed in batch {
            if search.holds_everywhere(seed) {
                sink.push(seed)?;
            }
        }
        return Ok(());
    }
    let (children, deeper) = stack.split_first_mut().expect("one buffer per remaining depth");
    let bit = 1u64 << search.order()[depth];
    for chunk in batch.chunks(BATCH) {
        let count = chunk.len();
        children[..count].copy_from_slice(chunk);
        for (slot, &seed) in children[count..2 * count].iter_mut().zip(chunk) {
            *slot = seed | bit;
        }
        let kept = unsafe { filter(search, depth + 1, children, 2 * count) };
        if kept > 0 {
            unsafe { expand(search, depth + 1, &children[..kept], deeper, sink)? };
        }
    }
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512dq")]
unsafe fn filter(search: &Search, depth: usize, seeds: &mut [u64], count: usize) -> usize {
    use std::arch::x86_64::*;
    assert!(seeds.len() >= count + LANES);
    let modulus = search.modulus();
    let mask = _mm512_set1_epi64(modulus.mask() as i64);
    let multiplier = _mm512_set1_epi64(modulus.multiplier as i64);
    let mut alive = count;
    for test in search.completed(depth) {
        if alive == 0 {
            break;
        }
        let hash_i = _mm512_set1_epi64(test.hash_i as i64);
        let hash_j = _mm512_set1_epi64(test.hash_j as i64);
        let offset = _mm512_set1_epi64(test.offset as i64);
        let span = _mm512_set1_epi64(test.span as i64);
        let mut kept = 0usize;
        let mut index = 0usize;
        while index < alive {
            let left = alive - index;
            let load: __mmask8 = if left >= LANES { 0xFF } else { (1u8 << left) - 1 };
            let seed = unsafe { _mm512_maskz_loadu_epi64(load, seeds.as_ptr().add(index) as *const i64) };
            let difference = _mm512_sub_epi64(_mm512_xor_si512(seed, hash_i), _mm512_xor_si512(seed, hash_j));
            let shifted = _mm512_and_si512(_mm512_add_epi64(_mm512_mullo_epi64(difference, multiplier), offset), mask);
            let passing = _mm512_mask_cmple_epu64_mask(load, shifted, span);
            unsafe {
                _mm512_storeu_si512(
                    seeds.as_mut_ptr().add(kept) as *mut __m512i,
                    _mm512_maskz_compress_epi64(passing, seed),
                )
            };
            kept += passing.count_ones() as usize;
            index += LANES;
        }
        alive = kept;
    }
    alive
}
