pub const MULTIPLIER: i64 = 0x5_DEEC_E66D;
pub const ADDEND: i64 = 0xB;
pub const MASK: i64 = (1 << 48) - 1;

const FLOAT_MULTIPLIER: f32 = 5.960_464_5e-8;
const DOUBLE_MULTIPLIER: f64 = 1.110_223_024_625_156_5e-16;

pub const REGION_A: i64 = 341_873_128_712;
pub const REGION_B: i64 = 132_897_987_541;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lcg {
    pub mul: i64,
    pub add: i64,
}

impl Lcg {
    pub const JAVA: Lcg = Lcg { mul: MULTIPLIER, add: ADDEND };

    pub const fn combine(self, steps: i64) -> Lcg {
        let mut n = (steps & MASK) as u64;
        let mut acc = Lcg { mul: 1, add: 0 };
        let mut base = self;
        while n != 0 {
            if n & 1 == 1 {
                acc = acc.then(base);
            }
            base = base.then(base);
            n >>= 1;
        }
        acc
    }

    const fn then(self, next: Lcg) -> Lcg {
        Lcg {
            mul: self.mul.wrapping_mul(next.mul) & MASK,
            add: self.add.wrapping_mul(next.mul).wrapping_add(next.add) & MASK,
        }
    }

    #[inline]
    pub const fn next_seed(self, seed: i64) -> i64 {
        seed.wrapping_mul(self.mul).wrapping_add(self.add) & MASK
    }
}

#[derive(Clone, Copy, Debug)]
pub struct JavaRandom {
    seed: i64,
}

impl JavaRandom {
    pub fn new(seed: i64) -> JavaRandom {
        let mut r = JavaRandom { seed: 0 };
        r.set_seed(seed);
        r
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ MULTIPLIER) & MASK;
    }

    pub fn state(&self) -> i64 {
        self.seed
    }

    pub fn set_state(&mut self, state: i64) {
        self.seed = state & MASK;
    }

    pub fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int_any(&mut self) -> i32 {
        self.next(32)
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        debug_assert!(bound > 0);
        if bound & (bound - 1) == 0 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let sample = self.next(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    pub fn next_long(&mut self) -> i64 {
        let upper = self.next(32) as i64;
        let lower = self.next(32) as i64;
        (upper << 32).wrapping_add(lower)
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * FLOAT_MULTIPLIER
    }

    pub fn next_double(&mut self) -> f64 {
        let upper = self.next(26) as i64;
        let lower = self.next(27) as i64;
        ((upper << 27) + lower) as f64 * DOUBLE_MULTIPLIER
    }

    pub fn set_large_feature_with_salt(&mut self, seed: i64, x: i32, z: i32, salt: i32) {
        let s = (x as i64)
            .wrapping_mul(REGION_A)
            .wrapping_add((z as i64).wrapping_mul(REGION_B))
            .wrapping_add(seed)
            .wrapping_add(salt as i64);
        self.set_seed(s);
    }

    pub fn set_large_feature_seed(&mut self, seed: i64, x: i32, z: i32) {
        self.set_seed(seed);
        let x_scale = self.next_long();
        let z_scale = self.next_long();
        let result = (x as i64).wrapping_mul(x_scale) ^ (z as i64).wrapping_mul(z_scale) ^ seed;
        self.set_seed(result);
    }
}
