use std::collections::BTreeSet;
use std::ops::Range;

use crate::cracker::{self, Check, Cracker, Event, JobKind, Mode, Stage, Task};
use crate::decorator::{DecoratorKind, DecoratorObservation};
use crate::hash;
use crate::lcg::{ADDEND, JavaRandom, Lcg, MASK, MULTIPLIER};
use crate::pillar::{self, PillarObservation};
use crate::placement::{Placement, StructureSet};
use crate::prefilter::Prefilter;
use crate::version::{GenRange, Version};
use crate::world::{Observation, StructureObservation};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1) as u64) as i32
    }

    fn seed(&mut self) -> i64 {
        (self.next() & MASK as u64) as i64
    }
}

fn inv48(a: i64) -> i64 {
    let mut x = a;
    for _ in 0..6 {
        x = x.wrapping_mul(2i64.wrapping_sub(a.wrapping_mul(x)));
    }
    x & MASK
}

fn back(state: i64) -> i64 {
    (state - ADDEND).wrapping_mul(inv48(MULTIPLIER)) & MASK
}

fn obs(set: StructureSet, rx: i32, rz: i32, offset: (i32, i32)) -> StructureObservation {
    let p = set.placement();
    StructureObservation { set, chunk_x: rx * p.spacing + offset.0, chunk_z: rz * p.spacing + offset.1 }
}

fn check(set: StructureSet, rx: i32, rz: i32, offset: (i32, i32)) -> Check {
    Check::new(&obs(set, rx, rz, offset))
}

fn true_check(set: StructureSet, seed: i64, rx: i32, rz: i32) -> Check {
    check(set, rx, rz, set.placement().region_offset(seed, rx, rz))
}

fn low_mask(bits: u32) -> i64 {
    (1i64 << bits) - 1
}

fn fast_low_bits(checks: &[Check]) -> u32 {
    17 + checks.iter().map(|c| c.placement.lift_bits()).max().unwrap_or(0)
}

fn run_scan(kind: &JobKind, mode: Mode, checks: &[Check], range: Range<u64>) -> Vec<i64> {
    let mut out = Vec::new();
    cracker::scan(kind, mode, checks, &[], GenRange::OVERWORLD, range, &mut out);
    out
}

fn lows(mode: Mode, checks: &[Check], low_bits: u32) -> Vec<i64> {
    run_scan(&JobKind::LiftLow { low_bits }, mode, checks, 0..1 << low_bits)
}

fn partial(seed: i64) -> u32 {
    let mut r = JavaRandom::new(seed);
    r.next(32);
    r.next(32);
    let state = r.state();
    ((((state >> 32) & 0xFFFF) << 16) | (state & 0xFFFF)) as u32
}

fn plant(seed: i64, set: StructureSet, count: usize, rng: &mut Rng) -> Vec<StructureObservation> {
    let p = set.placement();
    let mut out = Vec::new();
    while out.len() < count {
        let (rx, rz) = (rng.range(-60, 60), rng.range(-60, 60));
        let (cx, cz) = p.potential_chunk(seed, rx * p.spacing, rz * p.spacing);
        let o = StructureObservation { set, chunk_x: cx, chunk_z: cz };
        if p.is_structure_chunk(seed, cx, cz) && !out.contains(&o) {
            out.push(o);
        }
    }
    out
}

fn scenario(seed: i64, sets: &[(StructureSet, usize)], rng: &mut Rng) -> Vec<Check> {
    let mut checks = Vec::new();
    for &(set, n) in sets {
        for o in plant(seed, set, n, rng) {
            let c = Check::new(&o);
            assert!(c.test(seed), "{set:?}");
            checks.push(c);
        }
    }
    checks
}

const ALL_KINDS_OF_BOUND: [(i32, StructureSet); 5] = [
    (24, StructureSet::DesertPyramids),
    (20, StructureSet::Shipwrecks),
    (12, StructureSet::OceanRuins),
    (26, StructureSet::Villages),
    (22, StructureSet::TrialChambers),
];

fn old_test_low(p: Placement, low: i64, rx: i32, rz: i32, offset: (i32, i32)) -> bool {
    let mut r = JavaRandom::new(0);
    r.set_large_feature_with_salt(low, rx, rz, p.salt);
    r.next_int(p.limit()) % 4 == offset.0 % 4 && r.next_int(p.limit()) % 4 == offset.1 % 4
}

#[test]
fn review_regressions_keep_the_real_seed() {
    let cases = [
        (StructureSet::DesertPyramids, (-971, -28), 81_400_241_335_560i64, (16, 11)),
        (StructureSet::Shipwrecks, (927, -510), 171_522_110_941_200i64, (8, 7)),
    ];
    for (set, (rx, rz), seed, offset) in cases {
        let p = set.placement();
        assert_eq!(p.region_offset(seed, rx, rz), offset, "{set:?}");
        let o = obs(set, rx, rz, offset);
        assert!(p.is_structure_chunk(seed, o.chunk_x, o.chunk_z), "{set:?}");
        let c = Check::new(&o);
        assert!(c.test(seed));
        let low19 = seed & low_mask(19);
        assert!(!old_test_low(p, low19, rx, rz, offset), "{set:?} the old test kept it");
        for bits in 18..=32 {
            assert!(c.test_low(seed & low_mask(bits), bits), "{set:?} at {bits} bits");
        }
        let lb = fast_low_bits(&[c]);
        assert!(run_scan(&JobKind::LiftLow { low_bits: 19 }, Mode::Normal, &[c], low19 as u64..low19 as u64 + 1).contains(&low19));
        let low = seed & low_mask(lb);
        assert!(run_scan(&JobKind::LiftLow { low_bits: lb }, Mode::Fast, &[c], low as u64..low as u64 + 1).contains(&low));
    }
    assert_eq!(81_400_241_335_560i64 & low_mask(19), 11528);
}

fn rejection_seed(set: StructureSet, rx: i32, rz: i32, z_axis: bool, rng: &mut Rng) -> i64 {
    let l = set.placement().limit() as i64;
    let rlim = ((1i64 << 31) / l) * l;
    let rejected = |rng: &mut Rng| ((rlim + rng.below(((1i64 << 31) - rlim) as u64) as i64) << 17) | rng.below(1 << 17) as i64;
    let s1 = if z_axis {
        loop {
            let s1 = back(rejected(rng));
            if s1 >> 17 < rlim {
                break s1;
            }
        }
    } else {
        rejected(rng)
    };
    let s0 = back(s1);
    let key = check(set, rx, rz, (0, 0)).key;
    let seed = ((s0 ^ MULTIPLIER) - key) & MASK;
    let mut r = JavaRandom::new(0);
    r.set_large_feature_with_salt(seed, rx, rz, set.placement().salt);
    assert_eq!(r.state(), s0);
    let first = r.next(31) as i64;
    let second = r.next(31) as i64;
    assert!(if z_axis { first < rlim && second >= rlim } else { first >= rlim });
    seed
}

#[test]
fn constructed_rejections_survive_every_path() {
    let mut rng = Rng(0x5EED_0001);
    for (bound, set) in ALL_KINDS_OF_BOUND {
        assert_eq!(set.placement().limit(), bound);
        for z_axis in [false, true] {
            for _ in 0..24 {
                let (rx, rz) = (rng.range(-3000, 3000), rng.range(-3000, 3000));
                let seed = rejection_seed(set, rx, rz, z_axis, &mut rng);
                let c = true_check(set, seed, rx, rz);
                assert!(c.test(seed));
                let what = format!("{set:?} z_axis {z_axis} seed {seed} region {rx},{rz}");

                for bits in 18..=32 {
                    assert!(c.test_low(seed & low_mask(bits), bits), "{what} at {bits} bits");
                }
                if c.placement.liftable() {
                    let low = seed & low_mask(19);
                    let found = run_scan(&JobKind::LiftLow { low_bits: 19 }, Mode::Normal, &[c], low as u64..low as u64 + 1);
                    assert_eq!(found, vec![low], "{what} normal low");
                    let high = (seed >> 19) as u64;
                    let kind = JobKind::LiftHigh { low_bits: 19, lows: vec![low] };
                    assert!(run_scan(&kind, Mode::Normal, &[c], high..high + 1).contains(&seed), "{what} normal high");
                }

                let lb = fast_low_bits(&[c]);
                let low = seed & low_mask(lb);
                let found = run_scan(&JobKind::LiftLow { low_bits: lb }, Mode::Fast, &[c], low as u64..low as u64 + 1);
                assert_eq!(found, vec![low], "{what} fast low");
                let high = (seed >> lb) as u64;
                let top = 1u64 << (48 - lb);
                let window = high.saturating_sub(5)..(high + 22).min(top);
                let kind = JobKind::LiftHigh { low_bits: lb, lows: vec![low] };
                assert!(run_scan(&kind, Mode::Fast, &[c], window.clone()).contains(&seed), "{what} fast high");
                let pre = Prefilter::new(lb, low, c.key, c.placement.limit(), c.offset);
                assert!(pre.keeps(high as u32), "{what} prefilter");
                let (mut a, mut b) = (Vec::new(), Vec::new());
                pre.run_scalar(window.start as u32, window.end as u32, &mut |h| a.push(h));
                if pre.run_simd(window.start as u32, window.end as u32, &mut |h| b.push(h)) {
                    assert_eq!(a, b, "{what} simd");
                }

                let p = pillar::pillar_seed(seed);
                let part = partial(seed);
                assert_eq!(pillar::structure_seed_from_pillar(part, p), seed);
                let lifts = [&c];
                assert!(cracker::pillar_low_ok(&lifts, p, part & 0xFFFF), "{what} klo");
                let unit = (((part & 0xFFFF) as u64) << 16) | (part >> 16) as u64;
                let kind = JobKind::Pillars { seeds: vec![p] };
                assert!(run_scan(&kind, Mode::Fast, &[c], unit..unit + 1).contains(&seed), "{what} fast pillars");
                assert!(run_scan(&kind, Mode::Normal, &[c], part as u64..part as u64 + 1).contains(&seed), "{what} normal pillars");
            }
        }
    }
}

#[test]
fn low_test_never_rejects_a_placed_structure() {
    let mut rng = Rng(0x5EED_0002);
    for set in StructureSet::ALL {
        let p = set.placement();
        for _ in 0..4000 {
            let seed = rng.seed();
            let (rx, rz) = (rng.range(-2000, 2000), rng.range(-2000, 2000));
            let offset = p.region_offset(seed, rx, rz);
            let c = check(set, rx, rz, offset);
            assert!(c.test(seed) || p.frequency < 1.0 || p.exclusion.is_some());
            for bits in 17..=32 {
                assert!(c.test_low(seed & low_mask(bits), bits), "{set:?} seed {seed} region {rx},{rz} bits {bits}");
            }
            let low = seed & low_mask(19);
            assert!(!run_scan(&JobKind::LiftLow { low_bits: 19 }, Mode::Normal, &[c], low as u64..low as u64 + 1).is_empty());
        }
    }
}

#[test]
fn lifted_bits_follow_the_bound() {
    let want = [
        (StructureSet::DesertPyramids, 3),
        (StructureSet::PillagerOutposts, 3),
        (StructureSet::Shipwrecks, 2),
        (StructureSet::OceanRuins, 2),
        (StructureSet::Villages, 1),
        (StructureSet::TrailRuins, 1),
        (StructureSet::TrialChambers, 1),
        (StructureSet::AncientCities, 0),
        (StructureSet::RuinedPortals, 0),
        (StructureSet::NetherComplexes, 0),
        (StructureSet::NetherFossils, 0),
        (StructureSet::BuriedTreasures, 0),
        (StructureSet::Mineshafts, 0),
        (StructureSet::EndCities, 0),
        (StructureSet::OceanMonuments, 0),
        (StructureSet::WoodlandMansions, 0),
    ];
    for (set, bits) in want {
        assert_eq!(set.placement().lift_bits(), bits, "{set:?}");
        assert_eq!(set.placement().liftable(), bits >= 2, "{set:?}");
    }
}

fn reach(target: u32, c: u32, e: u32, up: bool) -> u32 {
    let step = 1i64 << e;
    let below = target as i64 - (target.wrapping_sub(c) as i64 & (step - 1));
    let above = if below == target as i64 { below } else { below + step };
    let pick = if up { above } else { below };
    (if pick < 0 { above } else if pick > 0x7FFF_FFFF { below } else { pick }) as u32
}

fn inv32(a: u32) -> u32 {
    let mut x = a;
    for _ in 0..5 {
        x = x.wrapping_mul(2u32.wrapping_sub(a.wrapping_mul(x)));
    }
    x
}

#[test]
fn prefilter_lanes_equal_java_draws() {
    let mut rng = Rng(0x5EED_0003);
    let sets = [StructureSet::DesertPyramids, StructureSet::Villages, StructureSet::OceanRuins, StructureSet::AncientCities, StructureSet::RuinedPortals];
    for low_bits in [18u32, 19, 20] {
        let e = low_bits - 17;
        let high_bits = 48 - low_bits;
        let hmask = ((1u64 << high_bits) - 1) as u32;
        for set in sets {
            let p = set.placement();
            let l = p.limit() as u32;
            let rlim = ((1u32 << 31) / l) * l;
            for low in [0, low_mask(low_bits), 0x5A5A5 & low_mask(low_bits), rng.seed() & low_mask(low_bits), rng.seed() & low_mask(low_bits)] {
                let (rx, rz) = (rng.range(-5000, 5000), rng.range(-5000, 5000));
                let key = check(set, rx, rz, (0, 0)).key;
                let pre = Prefilter::new(low_bits, low, key, p.limit(), (0, 0));
                let y = low.wrapping_add(key) & MASK;
                let xl = (y ^ MULTIPLIER) & low_mask(low_bits);
                let yh = (y >> low_bits) as u32 & hmask;
                let mp = (MULTIPLIER >> low_bits) as u32 & hmask;
                let mut highs: Vec<u32> = vec![0, hmask, 1, hmask - 1];
                for _ in 0..64 {
                    highs.push(rng.below(hmask as u64 + 1) as u32);
                }
                for draw in [1i64, 2] {
                    let lcg = Lcg::JAVA.combine(draw);
                    let c = ((xl.wrapping_mul(lcg.mul).wrapping_add(lcg.add) & MASK) >> 17) as u32;
                    let inv = inv32(lcg.mul as u32);
                    for target in [0u32, l - 1, rlim.min(0x7FFF_FFFF), rlim.saturating_sub(1), 0x7FFF_FFFF] {
                        for up in [false, true] {
                            let v = reach(target, c, e, up);
                            assert_eq!(v & ((1 << e) - 1), c & ((1 << e) - 1));
                            let z = ((v.wrapping_sub(c) & 0x7FFF_FFFF) >> e).wrapping_mul(inv) & hmask;
                            let h = (z ^ mp).wrapping_sub(yh) & hmask;
                            let got = pre.draws(h);
                            assert_eq!(if draw == 1 { got.0 } else { got.1 }, v, "{set:?} bits {low_bits} draw {draw} target {target}");
                            highs.push(h);
                        }
                    }
                }
                for h in highs {
                    let seed = ((h as i64) << low_bits) | low;
                    let mut r = JavaRandom::new(0);
                    r.set_large_feature_with_salt(seed, rx, rz, p.salt);
                    let want = (r.next(31) as u32, r.next(31) as u32);
                    assert_eq!(pre.draws(h), want, "{set:?} bits {low_bits} low {low} high {h}");
                }
            }
        }
    }
}

#[test]
fn prefilter_simd_equals_scalar() {
    let mut rng = Rng(0x5EED_0004);
    let sets = [
        StructureSet::DesertPyramids,
        StructureSet::Shipwrecks,
        StructureSet::OceanRuins,
        StructureSet::Villages,
        StructureSet::TrialChambers,
        StructureSet::AncientCities,
        StructureSet::RuinedPortals,
        StructureSet::NetherComplexes,
    ];
    let mut compared = 0;
    for low_bits in [18u32, 19, 20] {
        let hmask = ((1u64 << (48 - low_bits)) - 1) as u32;
        for set in sets {
            let l = set.placement().limit();
            for _ in 0..4 {
                let seed = rng.seed();
                let low = seed & low_mask(low_bits);
                let c = true_check(set, seed, rng.range(-900, 900), rng.range(-900, 900));
                let pre = Prefilter::new(low_bits, low, c.key, l, c.offset);
                assert!(pre.keeps((seed >> low_bits) as u32));
                let h0 = 1000 + rng.below(hmask as u64 - (1 << 20)) as u32;
                let mut first = Vec::new();
                pre.run_scalar(h0, h0 + (1 << 18) + 7, &mut |h| first.push(h));
                assert!(!first.is_empty(), "{set:?}");
                let last = *first.last().unwrap();
                let ranges = [
                    (h0, h0 + (1 << 18) + 7),
                    (last.saturating_sub(16 * 20 + 5), last + 1),
                    (last.saturating_sub(3), last + 1),
                    (h0 + 1, h0 + 1),
                    (hmask - 37, hmask + 1),
                ];
                for (a, b) in ranges {
                    let (mut s, mut v) = (Vec::new(), Vec::new());
                    pre.run_scalar(a, b, &mut |h| s.push(h));
                    if pre.run_simd(a, b, &mut |h| v.push(h)) {
                        assert_eq!(s, v, "{set:?} bits {low_bits} range {a}..{b}");
                        compared += 1;
                    }
                    let mut r = Vec::new();
                    pre.run(a, b, &mut |h| r.push(h));
                    assert_eq!(s, r);
                    assert_eq!(s, (a..b).filter(|&h| pre.keeps(h)).collect::<Vec<_>>());
                }
                let tail = (last.saturating_sub(16 * 20 + 5), last + 1);
                assert_ne!((tail.1 - tail.0) % 16, 0);
            }
        }
    }
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx512f") {
        assert!(compared > 0);
    }
    let _ = compared;
}

fn lift_window(mode: Mode, checks: &[Check], low_bits: u32, lows: &[i64], a: Range<i64>, top: u32, t: u64) -> BTreeSet<i64> {
    let high_bits = 48 - low_bits;
    let sh = top - low_bits;
    let kind = JobKind::LiftHigh { low_bits, lows: lows.to_vec() };
    let mut out = BTreeSet::new();
    for (i, &low) in lows.iter().enumerate() {
        if a.contains(&(low & 0x1FFFF)) {
            let base = (i as u64) << high_bits;
            out.extend(run_scan(&kind, mode, checks, base | (t << sh)..base | ((t + 1) << sh)));
        }
    }
    out
}

fn lift_equivalence(seed: i64, checks: &[Check], width: i64, top: u32) -> usize {
    let a0 = ((seed & 0x1FFFF) - width / 2).clamp(0, 0x20000 - width);
    let a = a0..a0 + width;
    let t = (seed >> top) as u64;
    let normal_lows = lows(Mode::Normal, checks, 19);
    assert!(normal_lows.contains(&(seed & low_mask(19))));
    let lb = fast_low_bits(checks);
    let fast_lows = lows(Mode::Fast, checks, lb);
    assert!(fast_lows.contains(&(seed & low_mask(lb))));
    let n = lift_window(Mode::Normal, checks, 19, &normal_lows, a.clone(), top, t);
    let f = lift_window(Mode::Fast, checks, lb, &fast_lows, a, top, t);
    assert!(n.contains(&seed));
    assert_eq!(n, f);
    n.len()
}

#[test]
fn fast_lifting_equals_normal_lifting() {
    use StructureSet as S;
    let mut rng = Rng(0x5EED_0005);
    let mixed = [
        (S::Villages, 1),
        (S::OceanRuins, 1),
        (S::TrialChambers, 1),
        (S::TrailRuins, 1),
        (S::PillagerOutposts, 1),
        (S::AncientCities, 1),
        (S::OceanMonuments, 1),
        (S::WoodlandMansions, 1),
        (S::Shipwrecks, 1),
        (S::DesertPyramids, 1),
        (S::Igloos, 1),
        (S::SwampHuts, 1),
    ];
    for _ in 0..2 {
        let seed = rng.seed();
        let checks = scenario(seed, &mixed, &mut rng);
        lift_equivalence(seed, &checks, 1 << 9, 34);
    }
    let seed = rng.seed();
    let checks = scenario(seed, &[(S::Villages, 1)], &mut rng);
    assert_eq!(fast_low_bits(&checks), 18);
    assert!(lift_equivalence(seed, &checks, 1 << 7, 34) >= 1000);
    let seed = rng.seed();
    let checks = scenario(seed, &[(S::Shipwrecks, 1)], &mut rng);
    assert!(lift_equivalence(seed, &checks, 1 << 6, 34) >= 1000);
    let seed = rng.seed();
    let checks = scenario(seed, &[(S::Igloos, 2), (S::OceanRuins, 1), (S::TrailRuins, 1)], &mut rng);
    lift_equivalence(seed, &checks, 1 << 9, 32);
}

fn pillar_window(mode: Mode, checks: &[Check], p: u16, kh0: u32, w: u32) -> BTreeSet<i64> {
    let kind = JobKind::Pillars { seeds: vec![p] };
    match mode {
        Mode::Normal => run_scan(&kind, mode, checks, (kh0 as u64) << 16..((kh0 + (1 << w)) as u64) << 16).into_iter().collect(),
        Mode::Fast => (0..1u64 << 16)
            .flat_map(|klo| run_scan(&kind, mode, checks, klo << 16 | kh0 as u64..klo << 16 | (kh0 + (1 << w)) as u64))
            .collect(),
    }
}

fn klo_passing(checks: &[Check], p: u16) -> usize {
    let lifts: Vec<&Check> = checks.iter().filter(|c| c.placement.lift_bits() > 0).collect();
    (0..1u32 << 16).filter(|&klo| cracker::pillar_low_ok(&lifts, p, klo)).count()
}

fn pillar_equivalence(seed: i64, checks: &[Check], w: u32) -> (usize, usize) {
    let p = pillar::pillar_seed(seed);
    let khi = partial(seed) >> 16;
    let kh0 = khi & !((1 << w) - 1);
    let n = pillar_window(Mode::Normal, checks, p, kh0, w);
    let f = pillar_window(Mode::Fast, checks, p, kh0, w);
    assert!(n.contains(&seed));
    assert_eq!(n, f);
    (n.len(), klo_passing(checks, p))
}

#[test]
fn fast_pillars_equal_normal_pillars() {
    use StructureSet as S;
    let mut rng = Rng(0x5EED_0006);
    let mixed = [
        (S::Villages, 1),
        (S::DesertPyramids, 1),
        (S::Shipwrecks, 1),
        (S::RuinedPortals, 1),
        (S::TrialChambers, 1),
        (S::TrailRuins, 1),
        (S::OceanRuins, 1),
        (S::PillagerOutposts, 1),
        (S::AncientCities, 1),
        (S::SwampHuts, 1),
        (S::Igloos, 1),
    ];
    for _ in 0..2 {
        let seed = rng.seed();
        let checks = scenario(seed, &mixed, &mut rng);
        let (_, pass) = pillar_equivalence(seed, &checks, 4);
        assert!(pass < 1 << 12, "{pass}");
    }
    let seed = rng.seed();
    let checks = scenario(seed, &[(S::OceanMonuments, 2), (S::WoodlandMansions, 1), (S::RuinedPortals, 1), (S::NetherComplexes, 1), (S::EndCities, 2)], &mut rng);
    let (_, pass) = pillar_equivalence(seed, &checks, 3);
    assert_eq!(pass, 1 << 16);
    let seed = rng.seed();
    let checks = scenario(seed, &[(S::Villages, 1)], &mut rng);
    let (found, pass) = pillar_equivalence(seed, &checks, 4);
    assert!(pass < 1 << 16 && pass > 1 << 13, "{pass}");
    assert!(found >= 1000, "{found}");
    let seed = rng.seed();
    let checks = scenario(seed, &[(S::Shipwrecks, 1)], &mut rng);
    let (found, _) = pillar_equivalence(seed, &checks, 4);
    assert!(found >= 1000, "{found}");
}

#[test]
fn pillar_low_bits_ignore_khi() {
    let mut rng = Rng(0x5EED_0007);
    for _ in 0..2000 {
        let p = rng.below(1 << 16) as u16;
        let klo = rng.below(1 << 16) as u32;
        let base = pillar::structure_seed_from_pillar(klo, p) & 0xFFFF_FFFF;
        let khi = rng.below(1 << 16) as u32;
        assert_eq!(pillar::structure_seed_from_pillar((khi << 16) | klo, p) & 0xFFFF_FFFF, base);
    }
}

#[test]
fn fast_pillars_reach_the_last_klo() {
    use StructureSet as S;
    let mut rng = Rng(0x5EED_0008);
    for _ in 0..3 {
        let p = rng.below(1 << 16) as u16;
        let khi = rng.below(1 << 16) as u32;
        let seed = pillar::structure_seed_from_pillar((khi << 16) | 0xFFFF, p);
        assert_eq!(pillar::pillar_seed(seed), p);
        let checks = scenario(seed, &[(S::DesertPyramids, 2), (S::Shipwrecks, 1), (S::Villages, 1)], &mut rng);
        let kind = JobKind::Pillars { seeds: vec![7, p] };
        let total = kind.total();
        assert_eq!(total, 2 << 32);
        let found = run_scan(&kind, Mode::Fast, &checks, total - (1 << 16)..total);
        assert!(found.contains(&seed));
        let found = run_scan(&kind, Mode::Fast, &checks, total - (1 << 16) + khi as u64..total - (1 << 16) + khi as u64 + 1);
        assert_eq!(found, vec![seed]);
    }
}

fn sample_structures() -> Vec<StructureObservation> {
    super::liftable_sightings(super::WORLD)
        .into_iter()
        .filter_map(|o| match o {
            Observation::Structure(s) => Some(s),
            _ => None,
        })
        .collect()
}

#[test]
fn task_bytes_round_trip_every_kind_and_mode() {
    let structures = sample_structures();
    let decorators = vec![DecoratorObservation { kind: DecoratorKind::Dungeon, x: 5, y: -20, z: 9, height_above_surface: None }];
    let range = GenRange { min_y: -64, height: 384 };
    for mode in [Mode::Normal, Mode::Fast] {
        let kinds = [
            JobKind::LiftLow { low_bits: 19 },
            JobKind::LiftLow { low_bits: 20 },
            JobKind::LiftHigh { low_bits: 19, lows: vec![1, 2, 524_287] },
            JobKind::LiftHigh { low_bits: 18, lows: vec![0, 262_143] },
            JobKind::Pillars { seeds: vec![0, 77, 65_535] },
            JobKind::WorldByHash { hashed: -5, structures: vec![WORLD_STRUCTURE, 3] },
            JobKind::world(vec![WORLD_STRUCTURE], Vec::new()),
        ];
        for (i, kind) in kinds.into_iter().enumerate() {
            let task = Task::with(40 + i as u64, kind, mode, structures.clone(), decorators.clone(), range);
            let bytes = task.to_bytes();
            let back = Task::from_bytes(&bytes).expect("round trip");
            assert_eq!(back.id(), task.id());
            assert_eq!(back.stage(), task.stage());
            assert_eq!(back.parts().1, mode);
            assert_eq!(format!("{:?}", back.parts().0), format!("{:?}", task.parts().0));
            assert_eq!(back.to_bytes(), bytes);
            let probe = 0..4096;
            assert_eq!(back.scan(probe.clone()), task.scan(probe));
            let mut stale = bytes.clone();
            stale[0] = 1;
            assert!(Task::from_bytes(&stale).is_none());
            assert!(Task::from_bytes(&bytes[..bytes.len() - 1]).is_none());
            let mut longer = bytes.clone();
            longer.push(0);
            assert!(Task::from_bytes(&longer).is_none());
            let mut bad_mode = bytes.clone();
            bad_mode[9] = 2;
            assert!(Task::from_bytes(&bad_mode).is_none());
        }
    }
}

const WORLD_STRUCTURE: i64 = super::WORLD & MASK;

#[test]
fn old_format_tasks_are_rejected() {
    let mut old = vec![1u8];
    old.extend_from_slice(&9u64.to_le_bytes());
    old.push(0);
    old.extend_from_slice(&(-64i32).to_le_bytes());
    old.extend_from_slice(&384i32.to_le_bytes());
    old.extend_from_slice(&0u32.to_le_bytes());
    old.extend_from_slice(&0u32.to_le_bytes());
    assert!(Task::from_bytes(&old).is_none());
    let mut old_high = vec![1u8];
    old_high.extend_from_slice(&9u64.to_le_bytes());
    old_high.push(1);
    old_high.extend_from_slice(&1u32.to_le_bytes());
    old_high.extend_from_slice(&5i64.to_le_bytes());
    old_high.extend_from_slice(&(-64i32).to_le_bytes());
    old_high.extend_from_slice(&384i32.to_le_bytes());
    old_high.extend_from_slice(&0u32.to_le_bytes());
    old_high.extend_from_slice(&0u32.to_le_bytes());
    assert!(Task::from_bytes(&old_high).is_none());
    let mut relabelled = old_high.clone();
    relabelled[0] = 2;
    assert!(Task::from_bytes(&relabelled).is_none());
}

fn run(cracker: &mut Cracker) -> Vec<Event> {
    let mut events = Vec::new();
    while cracker.step(1 << 26) {
        events.extend(cracker.drain_events());
    }
    events.extend(cracker.drain_events());
    events
}

#[test]
fn fast_mode_cracks_by_lifting() {
    let mut cracker = Cracker::new(Version::V26_2);
    assert_eq!(cracker.mode(), Mode::Normal);
    assert_eq!(Mode::default(), Mode::Normal);
    cracker.set_mode(Mode::Fast);
    for o in super::liftable_sightings(super::WORLD) {
        cracker.add(o);
    }
    let task = cracker.task().unwrap();
    assert!(matches!(task.parts(), (JobKind::LiftLow { low_bits: 20 }, Mode::Fast)), "{:?}", task.parts());
    let mut normal = cracker.clone();
    normal.set_mode(Mode::Normal);
    let events = run(&mut cracker);
    assert!(events.contains(&Event::Started(Stage::LiftLow)), "{events:?}");
    assert!(events.contains(&Event::Started(Stage::LiftHigh)), "{events:?}");
    let fast = cracker.structure_seeds().unwrap().to_vec();
    assert!(fast.contains(&WORLD_STRUCTURE), "{events:?}");
    run(&mut normal);
    assert_eq!(normal.structure_seeds(), Some(&fast[..]));
}

#[test]
fn changing_mode_restarts_the_running_job() {
    let mut cracker = Cracker::new(Version::V26_2);
    for o in super::liftable_sightings(super::WORLD) {
        cracker.add(o);
    }
    let old = cracker.next_slice(1 << 12).unwrap();
    assert_eq!(cracker.stage(), Stage::LiftLow);
    assert_eq!(cracker.task().unwrap().parts().1, Mode::Normal);
    cracker.set_mode(Mode::Normal);
    assert_eq!(cracker.task().unwrap().id(), old.job);
    cracker.set_mode(Mode::Fast);
    let task = cracker.task().unwrap();
    assert_ne!(task.id(), old.job);
    assert_eq!(task.parts().1, Mode::Fast);
    let found = task.scan(old.range.clone());
    cracker.complete(old, &found);
    assert_eq!(cracker.progress(), 0.0);
    let events = run(&mut cracker);
    assert!(events.contains(&Event::Started(Stage::LiftLow)), "{events:?}");
    assert!(cracker.structure_seeds().unwrap().contains(&WORLD_STRUCTURE), "{events:?}");
    cracker.add(Observation::HashedSeed(hash::obfuscate(super::WORLD)));
    let slice = cracker.next_slice(1 << 12).unwrap();
    assert_eq!(cracker.stage(), Stage::WorldByHash);
    cracker.set_mode(Mode::Normal);
    assert_eq!(cracker.task().unwrap().id(), slice.job);
    cracker.requeue(slice);
    run(&mut cracker);
    assert_eq!(cracker.world_seed(), Some(super::WORLD));
    cracker.set_mode(Mode::Fast);
    cracker.clear();
    assert_eq!(cracker.mode(), Mode::Fast);
}

#[test]
fn fast_lifting_waits_for_as_much_evidence_as_normal() {
    use StructureSet as S;
    let mut rng = Rng(0x5EED_0009);
    for _ in 0..200 {
        let seed = rng.seed();
        let mut normal = Cracker::new(Version::V26_2);
        let mut fast = Cracker::new(Version::V26_2);
        fast.set_mode(Mode::Fast);
        let sets = [S::DesertPyramids, S::Shipwrecks, S::OceanRuins, S::Villages, S::TrialChambers, S::AncientCities, S::RuinedPortals, S::TrailRuins];
        for _ in 0..rng.range(1, 8) {
            let set = sets[rng.below(sets.len() as u64) as usize];
            for o in plant(seed, set, 1, &mut rng) {
                normal.add(Observation::Structure(o));
                fast.add(Observation::Structure(o));
            }
        }
        let evidence: u32 = fast.structures().iter().map(|o| 2 * o.set.placement().lift_bits()).sum();
        assert!(fast.liftable_count() >= normal.liftable_count());
        let n = normal.task().map(|t| t.stage());
        let f = fast.task().map(|t| t.stage());
        if n == Some(Stage::LiftLow) {
            assert_eq!(f, Some(Stage::LiftLow));
        }
        if f == Some(Stage::LiftLow) {
            assert!(evidence >= cracker::LIFT_COUNT as u32 * 4, "{evidence}");
            assert!(fast.structure_bits() >= cracker::LIFT_BITS);
        }
    }
}

#[test]
fn fast_mode_cracks_by_pillars() {
    let mut cracker = Cracker::new(Version::V26_2);
    cracker.set_mode(Mode::Fast);
    let heights = pillar::heights(pillar::pillar_seed(super::WORLD));
    cracker.add(Observation::Pillars(PillarObservation { heights: heights.map(Some) }));
    let p = StructureSet::EndCities.placement();
    for (rx, rz) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0)] {
        let (cx, cz) = p.potential_chunk(super::WORLD, rx * p.spacing, rz * p.spacing);
        cracker.add(Observation::Structure(StructureObservation { set: StructureSet::EndCities, chunk_x: cx, chunk_z: cz }));
    }
    for o in super::liftable_sightings(super::WORLD).into_iter().take(3) {
        cracker.add(o);
    }
    cracker.add(Observation::HashedSeed(hash::obfuscate(super::WORLD)));
    let events = run(&mut cracker);
    assert!(events.contains(&Event::Started(Stage::Pillars)), "{events:?}");
    assert_eq!(cracker.world_seed(), Some(super::WORLD), "{events:?}");
}

#[test]
fn bench_normal_against_fast() {
    use StructureSet as S;
    use std::time::Instant;
    let Ok(path) = std::env::var("SEEDCRACK_BENCH") else { return };
    let mut rng = Rng(0x5EED_000A);
    let mut report = String::new();
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    report += &format!("load before: {load}");

    let seed = rng.seed();
    let checks = scenario(seed, &[(S::Villages, 1), (S::DesertPyramids, 1), (S::Shipwrecks, 1), (S::RuinedPortals, 1)], &mut rng);
    let p = pillar::pillar_seed(seed);
    let part = partial(seed) as u64;
    let kind = JobKind::Pillars { seeds: vec![p] };
    let slice = 1u64 << 26;
    let start = part & !(slice - 1);
    let t = Instant::now();
    let normal = run_scan(&kind, Mode::Normal, &checks, start..start + slice);
    let normal_time = t.elapsed().as_secs_f64() * (1u64 << 32) as f64 / slice as f64;
    let t = Instant::now();
    let fast = run_scan(&kind, Mode::Fast, &checks, 0..1 << 32);
    let fast_time = t.elapsed().as_secs_f64();
    report += &format!(
        "pillars (village, pyramid, shipwreck, portal): normal 2^26 slice extrapolated to 2^32: {normal_time:.2} s (planted in slice {}); fast full 2^32: {fast_time:.3} s, {} survivors, planted {}, klo passing {}\n",
        normal.contains(&seed),
        fast.len(),
        fast.contains(&seed),
        klo_passing(&checks, p)
    );

    let seed = rng.seed();
    let checks = scenario(seed, &[(S::DesertPyramids, 1), (S::Igloos, 1), (S::SwampHuts, 1), (S::Shipwrecks, 1), (S::Villages, 1)], &mut rng);
    let t = Instant::now();
    let normal_lows = lows(Mode::Normal, &checks, 19);
    let normal_low_time = t.elapsed().as_secs_f64();
    let i = normal_lows.iter().position(|&l| l == seed & low_mask(19)).unwrap() as u64;
    let kind = JobKind::LiftHigh { low_bits: 19, lows: normal_lows.clone() };
    let slice = 1u64 << 24;
    let start = (i << 29) | ((seed as u64 >> 19) & !(slice - 1));
    let t = Instant::now();
    let found = run_scan(&kind, Mode::Normal, &checks, start..start + slice);
    let per_low = t.elapsed().as_secs_f64() * (1u64 << 29) as f64 / slice as f64;
    report += &format!(
        "lifting (pyramid, igloo, swamp hut, shipwreck, village): normal low stage {normal_low_time:.3} s, {} lows; high stage 2^24 slice extrapolated: {:.1} s per low, {:.1} s total (planted in slice {})\n",
        normal_lows.len(),
        per_low,
        per_low * normal_lows.len() as f64,
        found.contains(&seed)
    );
    let strict = (0..1i64 << 19)
        .filter(|&low| {
            checks.iter().filter(|c| c.placement.liftable()).all(|c| old_test_low(c.placement, low, c.region.0, c.region.1, c.offset))
        })
        .count();
    report += &format!("normal low stage with the old strict test: {strict} lows\n");
    let lb = fast_low_bits(&checks);
    let t = Instant::now();
    let fast_lows = lows(Mode::Fast, &checks, lb);
    let fast_low_time = t.elapsed().as_secs_f64();
    let kind = JobKind::LiftHigh { low_bits: lb, lows: fast_lows.clone() };
    let t = Instant::now();
    let found = run_scan(&kind, Mode::Fast, &checks, 0..kind.total());
    let fast_time = t.elapsed().as_secs_f64();
    report += &format!(
        "fast: low bits {lb}, low stage {fast_low_time:.3} s, {} lows; full high stage {fast_time:.2} s, {} survivors, planted {}\n",
        fast_lows.len(),
        found.len(),
        found.contains(&seed)
    );
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    report += &format!("load after: {load}");
    std::fs::write(path, report).unwrap();
}

