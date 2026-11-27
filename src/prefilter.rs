use crate::lcg::{ADDEND, Lcg, MASK, MULTIPLIER};

const TWO: Lcg = Lcg::JAVA.combine(2);
const M31: u32 = 0x7FFF_FFFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Prefilter {
    hmask: u32,
    y: u32,
    mp: u32,
    k1: u32,
    c1: u32,
    k2: u32,
    c2: u32,
    limit: u32,
    ox: u32,
    oz: u32,
    rlim: u32,
    pow2: bool,
    shift: u32,
    p: u32,
    dinv: u32,
    dlim: u32,
}

impl Prefilter {
    pub(crate) fn new(low_bits: u32, low: i64, key: i64, limit: i32, offset: (i32, i32)) -> Prefilter {
        debug_assert!((17..=31).contains(&low_bits) && limit > 0);
        let high_bits = 48 - low_bits;
        let hmask = ((1u64 << high_bits) - 1) as u32;
        let y = low.wrapping_add(key) & MASK;
        let xl = (y ^ MULTIPLIER) & ((1i64 << low_bits) - 1);
        let e = low_bits - 17;
        let limit = limit as u32;
        let pow2 = limit & (limit - 1) == 0;
        let p = limit.trailing_zeros();
        let d = limit >> p;
        let mut dinv = d;
        for _ in 0..5 {
            dinv = dinv.wrapping_mul(2u32.wrapping_sub(d.wrapping_mul(dinv)));
        }
        Prefilter {
            hmask,
            y: (y >> low_bits) as u32 & hmask,
            mp: (MULTIPLIER >> low_bits) as u32 & hmask,
            k1: (MULTIPLIER as u32) << e,
            c1: ((xl.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK) >> 17) as u32,
            k2: (TWO.mul as u32) << e,
            c2: ((xl.wrapping_mul(TWO.mul).wrapping_add(TWO.add) & MASK) >> 17) as u32,
            limit,
            ox: offset.0 as u32,
            oz: offset.1 as u32,
            rlim: if pow2 { 1 << 31 } else { ((1u32 << 31) / limit) * limit },
            pow2,
            shift: 31 - p,
            p,
            dinv,
            dlim: u32::MAX / d,
        }
    }

    #[inline]
    pub(crate) fn draws(&self, high: u32) -> (u32, u32) {
        let z = (high.wrapping_add(self.y) & self.hmask) ^ self.mp;
        (z.wrapping_mul(self.k1).wrapping_add(self.c1) & M31, z.wrapping_mul(self.k2).wrapping_add(self.c2) & M31)
    }

    #[inline]
    fn value(&self, s: u32) -> u32 {
        if self.pow2 { s >> self.shift } else { s % self.limit }
    }

    #[inline]
    pub(crate) fn keeps(&self, high: u32) -> bool {
        let (s1, s2) = self.draws(high);
        let r1 = s1 >= self.rlim;
        let r2 = s2 >= self.rlim;
        (r1 || self.value(s1) == self.ox) && (r1 || r2 || self.value(s2) == self.oz)
    }

    pub(crate) fn run(&self, h0: u32, h1: u32, f: &mut impl FnMut(u32)) {
        if !self.run_simd(h0, h1, f) {
            self.run_scalar(h0, h1, f);
        }
    }

    pub(crate) fn run_scalar(&self, h0: u32, h1: u32, f: &mut impl FnMut(u32)) {
        for h in h0..h1 {
            if self.keeps(h) {
                f(h);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub(crate) fn run_simd(&self, h0: u32, h1: u32, f: &mut impl FnMut(u32)) -> bool {
        if !std::arch::is_x86_feature_detected!("avx512f") {
            return false;
        }
        unsafe { self.run_avx512(h0, h1, f) };
        true
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub(crate) fn run_simd(&self, _h0: u32, _h1: u32, _f: &mut impl FnMut(u32)) -> bool {
        false
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx512f")]
    fn run_avx512(&self, h0: u32, h1: u32, f: &mut impl FnMut(u32)) {
        use std::arch::x86_64::*;
        let iota = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
        let y = _mm512_set1_epi32(self.y as i32);
        let hm = _mm512_set1_epi32(self.hmask as i32);
        let mp = _mm512_set1_epi32(self.mp as i32);
        let k1 = _mm512_set1_epi32(self.k1 as i32);
        let c1 = _mm512_set1_epi32(self.c1 as i32);
        let k2 = _mm512_set1_epi32(self.k2 as i32);
        let c2 = _mm512_set1_epi32(self.c2 as i32);
        let m31 = _mm512_set1_epi32(M31 as i32);
        let rl = _mm512_set1_epi32(self.rlim as i32);
        let mut h = h0;
        while h1 - h >= 16 {
            let hv = _mm512_add_epi32(_mm512_set1_epi32(h as i32), iota);
            let z = _mm512_xor_si512(_mm512_and_si512(_mm512_add_epi32(hv, y), hm), mp);
            let s1 = _mm512_and_si512(_mm512_add_epi32(_mm512_mullo_epi32(z, k1), c1), m31);
            let s2 = _mm512_and_si512(_mm512_add_epi32(_mm512_mullo_epi32(z, k2), c2), m31);
            let r1 = _mm512_cmpge_epu32_mask(s1, rl);
            let r2 = _mm512_cmpge_epu32_mask(s2, rl);
            let mut keep = (self.lane_eq(s1, self.ox) | r1) & (self.lane_eq(s2, self.oz) | r2 | r1);
            while keep != 0 {
                f(h + keep.trailing_zeros());
                keep &= keep - 1;
            }
            h += 16;
        }
        self.run_scalar(h, h1, f);
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx512f")]
    fn lane_eq(&self, s: std::arch::x86_64::__m512i, o: u32) -> std::arch::x86_64::__mmask16 {
        use std::arch::x86_64::*;
        if self.pow2 {
            return _mm512_cmpeq_epi32_mask(_mm512_srlv_epi32(s, _mm512_set1_epi32(self.shift as i32)), _mm512_set1_epi32(o as i32));
        }
        let t = _mm512_sub_epi32(s, _mm512_set1_epi32(o as i32));
        let nonneg = _mm512_cmplt_epu32_mask(t, _mm512_set1_epi32(i32::MIN));
        let low0 = _mm512_testn_epi32_mask(t, _mm512_set1_epi32(((1u32 << self.p) - 1) as i32));
        let u = _mm512_mullo_epi32(_mm512_srlv_epi32(t, _mm512_set1_epi32(self.p as i32)), _mm512_set1_epi32(self.dinv as i32));
        let div = _mm512_cmple_epu32_mask(u, _mm512_set1_epi32(self.dlim as i32));
        nonneg & low0 & div
    }
}
