use std::collections::BTreeSet;
use std::time::Instant;

use super::bedrock_vectors::{JVM_CELLS, REAL_CELLS, REAL_WORLD_SEED};
use super::{Mix, floor_cells_around, overworld_cells_around, roof_cells_around};
use crate::bedrock::search::{Constraint, Lanes, Modulus, Search};
use crate::bedrock::{
    BandSearch, Band, BedrockCell, GRADIENT_THRESHOLDS, MAX_SURVIVORS, MIN_NARROW_CELLS, OverworldFloor, Overflow,
    band_seed, next_long_preimages, shared_structure_seeds, structure_seeds,
};
use crate::lcg::{JavaRandom, MASK, MULTIPLIER};

fn next_long_low(seed: i64) -> i64 {
    JavaRandom::new(seed).next_long() & MASK
}

fn java_gradient_probability(y: i32, bottom: i32, top: i32) -> f64 {
    let t = (y as f64 - bottom as f64) / (top as f64 - bottom as f64);
    1.0 + t * (0.0 - 1.0)
}

fn java_float_below(steps: u64, probability: f64) -> bool {
    ((steps as f32 * 5.960_464_5e-8_f32) as f64) < probability
}

fn boundaries(modulus: Modulus) -> [u64; 4] {
    GRADIENT_THRESHOLDS.map(|t| (t << 24) >> (48 - modulus.bits))
}

fn true_side(modulus: Modulus, seed: u64, hash: u64, boundary: u64) -> Constraint {
    if modulus.state(seed, hash) < boundary {
        Constraint::new(modulus, hash, 0..boundary)
    } else {
        Constraint::new(modulus, hash, boundary..modulus.size())
    }
}

fn narrow_true(modulus: Modulus, seed: u64, mix: &mut Mix, clustered_base: Option<u64>) -> Constraint {
    let b = boundaries(modulus);
    let low = mix.next() & 1 == 0;
    loop {
        let hash = match clustered_base {
            Some(base) => base ^ (mix.next() & mix.next() & mix.next()),
            None => mix.next(),
        } & modulus.mask();
        let state = modulus.state(seed, hash);
        if low && state < b[3] {
            return Constraint::new(modulus, hash, 0..b[3]);
        }
        if !low && state >= b[0] {
            return Constraint::new(modulus, hash, b[0]..modulus.size());
        }
    }
}

fn planted(modulus: Modulus, seed: u64) -> Vec<Constraint> {
    let b = boundaries(modulus);
    let top = modulus.size();
    let plan = [
        (0, 0..b[3]),
        (b[3] - 1, 0..b[3]),
        (b[3], b[3]..top),
        (top - 1, b[0]..top),
        (b[0], b[0]..top),
        (b[0] - 1, 0..b[0]),
        (b[1] - 1, 0..b[1]),
        (b[1], b[1]..top),
        (b[2] - 1, 0..b[2]),
        (b[2], b[2]..top),
        (b[3] - 1, 0..b[3]),
        (0, 0..b[3]),
        (top - 1, b[0]..top),
        (b[0], b[0]..top),
    ];
    plan.into_iter()
        .map(|(state, accepted)| {
            let c = Constraint::new(modulus, modulus.seed_for_state(state, seed), accepted);
            assert_eq!(modulus.state(seed, c.hash), state);
            assert!(c.holds(modulus, seed));
            c
        })
        .collect()
}

fn brute_force(modulus: Modulus, constraints: &[Constraint]) -> Vec<u64> {
    let mut sorted = constraints.to_vec();
    sorted.sort_by_key(Constraint::width);
    (0..modulus.size()).filter(|&s| sorted.iter().all(|c| reference_holds(modulus, c, s))).collect()
}

fn reference_holds(modulus: Modulus, c: &Constraint, seed: u64) -> bool {
    let size = 1u128 << modulus.bits;
    let mixed = (seed as u128 ^ c.hash as u128) % size;
    let state = (mixed * modulus.multiplier as u128 + 11) % size;
    (c.lo as u128..c.hi as u128).contains(&state)
}

fn just_outside(modulus: Modulus, seed: u64, below: bool, boundary: u64) -> Constraint {
    if below {
        Constraint::new(modulus, modulus.seed_for_state(boundary, seed), 0..boundary)
    } else {
        Constraint::new(modulus, modulus.seed_for_state(boundary - 1, seed), boundary..modulus.size())
    }
}

#[test]
fn seeds_one_step_outside_a_cell_are_rejected() {
    let mut mix = Mix(0x5EED_0008);
    for bits in [20, 48] {
        let modulus = if bits == 48 { Modulus::JAVA } else { Modulus::reduced(bits) };
        for (k, boundary) in boundaries(modulus).into_iter().enumerate() {
            for below in [true, false] {
                let seed = mix.next() & modulus.mask();
                let mut cells = planted(modulus, seed);
                cells.extend((0..60).map(|_| narrow_true(modulus, seed, &mut mix, None)));
                let outside = just_outside(modulus, seed, below, boundary);
                assert!(!reference_holds(modulus, &outside, seed));
                cells.push(outside);
                let search = Search::new(modulus, &cells);
                let found = scan_all(&search, Lanes::Widest);
                assert!(!found.contains(&seed), "bits {bits} k {k} below {below}");
                assert!(found.iter().all(|&s| cells.iter().all(|c| reference_holds(modulus, c, s))));
                if bits == 20 {
                    assert_eq!(found, brute_force(modulus, &cells));
                }
                cells.pop();
                let search = Search::new(modulus, &cells);
                assert!(scan_all(&search, Lanes::Scalar).contains(&seed));
            }
        }
    }
}

#[test]
fn minimum_narrow_cell_count_finds_the_band_seed() {
    let mut mix = Mix(0x5EED_0009);
    for trial in 0..4 {
        let truth = (mix.next() & MASK as u64) as i64;
        let x0 = (mix.below(60_000_000) as i64 - 30_000_000) as i32;
        let z0 = (mix.below(60_000_000) as i64 - 30_000_000) as i32;
        let cells: Vec<BedrockCell> = floor_cells_around(truth, x0, z0, 1).into_iter().take(MIN_NARROW_CELLS).collect();
        let search = BandSearch::new(Band::NetherFloor, &cells).unwrap();
        let started = Instant::now();
        let mut scalar = Vec::new();
        search.scan_with(0..search.total(), &mut scalar, Lanes::Scalar).unwrap();
        let scalar_time = started.elapsed();
        let started = Instant::now();
        let mut widest = Vec::new();
        search.scan_with(0..search.total(), &mut widest, Lanes::Widest).unwrap();
        eprintln!(
            "minimum cells trial {trial}: scalar {scalar_time:?}, widest {:?}, {} survivors",
            started.elapsed(),
            scalar.len()
        );
        assert!(scalar.contains(&truth));
        assert_eq!(scalar, widest);
        assert!(scalar.iter().all(|&s| cells.iter().all(|c| c.holds_for_band_seed(s))));
    }
}

fn scan_all(search: &Search, lanes: Lanes) -> Vec<u64> {
    let mut out = Vec::new();
    search.scan(0..search.total(), &mut out, usize::MAX, lanes).unwrap();
    out
}

fn scan_in_pieces(search: &Search, mix: &mut Mix, pieces: u64, lanes: Lanes) -> Vec<u64> {
    let total = search.total();
    let mut cuts: Vec<u64> = (0..pieces).map(|_| mix.below(total + 1)).collect();
    cuts.extend([0, total]);
    cuts.sort_unstable();
    let mut ranges: Vec<(u64, u64)> = cuts.windows(2).map(|w| (w[0], w[1])).collect();
    mix.shuffle(&mut ranges);
    let mut out = Vec::new();
    for (start, end) in ranges {
        search.scan(start..end, &mut out, usize::MAX, lanes).unwrap();
    }
    out.sort_unstable();
    out
}

#[test]
fn thresholds_match_the_java_float_rule() {
    for band in Band::ALL {
        let ys = band.gradient_ys();
        let (bottom, top) = (ys.start - 1, ys.end);
        for (k, y) in ys.enumerate() {
            let probability = java_gradient_probability(y, bottom, top);
            let threshold = GRADIENT_THRESHOLDS[k];
            assert!(java_float_below(threshold - 1, probability), "{band:?} y {y}");
            assert!(!java_float_below(threshold, probability), "{band:?} y {y}");
        }
    }
}

#[test]
fn cells_exist_only_in_gradient_layers() {
    for band in Band::ALL {
        let ys = band.gradient_ys();
        assert_eq!(ys.len(), 4);
        assert!(BedrockCell::new(band, 0, ys.start - 1, 0, true).is_none());
        assert!(BedrockCell::new(band, 0, ys.end, 0, true).is_none());
        for y in ys {
            let cell = BedrockCell::new(band, 3, y, -7, false).unwrap();
            assert_eq!(cell.band(), band);
            for other in Band::ALL.into_iter().filter(|&b| b != band) {
                assert!(BedrockCell::new(other, 3, y, -7, false).is_none());
            }
        }
    }
    let narrow = [
        (Band::NetherFloor, 4, true),
        (Band::NetherFloor, 1, false),
        (Band::NetherRoof, 123, true),
        (Band::NetherRoof, 126, false),
        (Band::OverworldFloor, -60, true),
        (Band::OverworldFloor, -63, false),
    ];
    for band in Band::ALL {
        for y in band.gradient_ys() {
            for bedrock in [false, true] {
                let cell = BedrockCell::new(band, 0, y, 0, bedrock).unwrap();
                assert_eq!(cell.narrow(), narrow.contains(&(band, y, bedrock)), "{band:?} {y} {bedrock}");
                let k = (y - band.gradient_ys().start) as usize;
                let below = GRADIENT_THRESHOLDS[k] as f64 / (1u64 << 24) as f64;
                let probability = if bedrock == (band != Band::NetherRoof) { below } else { 1.0 - below };
                assert!((cell.bits() + probability.log2()).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn known_world_gives_known_band_seeds() {
    assert_eq!(band_seed(Band::NetherRoof, REAL_WORLD_SEED), Some(191_924_403_737_289));
    assert_eq!(band_seed(Band::NetherFloor, REAL_WORLD_SEED), Some(18_240_473_916_414));
    assert_eq!(band_seed(Band::OverworldFloor, REAL_WORLD_SEED), None);
    assert_eq!(band_seed(Band::NetherFloor, REAL_WORLD_SEED & MASK), band_seed(Band::NetherFloor, REAL_WORLD_SEED));
}

#[test]
fn real_observed_cells_hold_for_the_real_world() {
    assert_eq!(REAL_CELLS.len(), 36);
    let mut per_band = [0usize; 3];
    for &(x, y, z) in REAL_CELLS {
        let band = Band::of_y(y).unwrap();
        let seed = band_seed(band, REAL_WORLD_SEED).unwrap();
        let seen = BedrockCell::new(band, x, y, z, true).unwrap();
        let other = BedrockCell::new(band, x, y, z, false).unwrap();
        assert!(seen.holds_for_band_seed(seed), "{x} {y} {z}");
        assert!(!other.holds_for_band_seed(seed), "{x} {y} {z}");
        per_band[band as usize] += 1;
    }
    assert_eq!(per_band, [4, 32, 0]);
    let cells: Vec<BedrockCell> = REAL_CELLS
        .iter()
        .map(|&(x, y, z)| BedrockCell::new(Band::of_y(y).unwrap(), x, y, z, true).unwrap())
        .collect();
    assert!(cells.len() < MIN_NARROW_CELLS);
    assert!(BandSearch::new(Band::NetherRoof, &cells).is_none());
}

#[test]
fn jvm_vectors_match() {
    let mut checked = 0;
    for &(world, x, z, floor, roof, overworld) in JVM_CELLS {
        let mut overworld_cells = Vec::new();
        for (band, bits) in [(Band::NetherFloor, floor), (Band::NetherRoof, roof), (Band::OverworldFloor, overworld)] {
            let first = band.gradient_ys().start - 1;
            for offset in 0..6 {
                let y = first + offset;
                let bedrock = bits >> offset & 1 == 1;
                let built = BedrockCell::new(band, x, y, z, bedrock);
                if !(1..=4).contains(&offset) {
                    assert!(built.is_none());
                    continue;
                }
                let cell = built.unwrap();
                let flipped = BedrockCell::new(band, x, y, z, !bedrock).unwrap();
                match band_seed(band, world) {
                    Some(seed) => {
                        assert!(cell.holds_for_band_seed(seed), "{world} {x} {y} {z}");
                        assert!(!flipped.holds_for_band_seed(seed), "{world} {x} {y} {z}");
                    }
                    None => {
                        assert!(OverworldFloor::new(&[cell]).test(world), "{world} {x} {y} {z}");
                        assert!(!OverworldFloor::new(&[flipped]).test(world), "{world} {x} {y} {z}");
                        overworld_cells.push(cell);
                    }
                }
                checked += 1;
            }
        }
        let floor = OverworldFloor::new(&overworld_cells);
        assert_eq!(floor.len(), 4);
        assert!(floor.test(world));
    }
    assert_eq!(checked, JVM_CELLS.len() * 12);
}

fn multiplier_inverse_48() -> u64 {
    let m = MULTIPLIER as u64;
    let mut inverse = m;
    for _ in 0..6 {
        inverse = inverse.wrapping_mul(2u64.wrapping_sub(m.wrapping_mul(inverse)));
    }
    inverse & MASK as u64
}

fn brute_preimages(value: i64) -> Vec<i64> {
    let inverse = multiplier_inverse_48();
    let mask = MASK as u64;
    let target = value & MASK;
    let lower = target as u64 & 0xFFFF_FFFF;
    let mut found: Vec<i64> = (0..1u64 << 16)
        .map(|low| {
            let second = (lower << 16) | low;
            let first = second.wrapping_sub(0xB).wrapping_mul(inverse) & mask;
            let initial = first.wrapping_sub(0xB).wrapping_mul(inverse) & mask;
            ((initial ^ MULTIPLIER as u64) & mask) as i64
        })
        .filter(|&s| next_long_low(s) == target)
        .collect();
    found.sort_unstable();
    found
}

#[test]
fn next_long_inversion_is_complete_and_sound() {
    let mut mix = Mix(0x5EED_0001);
    let mut seeds: Vec<i64> = (0..100_000).map(|_| (mix.next() & MASK as u64) as i64).collect();
    for high in [0u64, 1, 0x7FFF, 0x8000, 0xFFFF, 0x1234] {
        for low in [0u64, 1, 0x7FFF_FFFF, 0x8000_0000, 0x8000_0001, 0xFFFF_FFFF, 0xFFFF_0000, 0x0000_FFFF] {
            seeds.push(((high << 32) | low) as i64);
        }
    }
    seeds.extend([0, MASK, MASK - 1, 1]);
    let mut counts = [0usize; 4];
    for &seed in &seeds {
        let image = next_long_low(seed);
        let found = next_long_preimages(image);
        assert!(found.contains(&(seed & MASK)), "missed {seed}");
        for &p in &found {
            assert!((0..=MASK).contains(&p));
            assert_eq!(next_long_low(p), image, "spurious {p} for {seed}");
        }
        counts[found.len().min(3)] += 1;
    }
    assert_eq!(counts[0], 0);
    let mut values: Vec<i64> = (0..120).map(|_| (mix.next() & MASK as u64) as i64).collect();
    for high in [0u64, 0xFFFF, 0x8000] {
        for low in [0u64, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF, 0xFFFF_8000] {
            values.push(((high << 32) | low) as i64);
        }
    }
    values.extend(seeds.iter().take(40).map(|&s| next_long_low(s)));
    for &value in &values {
        let found = next_long_preimages(value);
        assert_eq!(found, brute_preimages(value), "value {value}");
        for &p in &found {
            assert_eq!(next_long_low(p), value & MASK);
        }
    }
}

#[test]
fn structure_seed_chain_inverts() {
    let mut mix = Mix(0x5EED_0002);
    for _ in 0..2000 {
        let world = mix.next() as i64;
        let structure = world & MASK;
        let floor = band_seed(Band::NetherFloor, world).unwrap();
        let roof = band_seed(Band::NetherRoof, world).unwrap();
        let from_floor = structure_seeds(Band::NetherFloor, floor);
        let from_roof = structure_seeds(Band::NetherRoof, roof);
        assert!(from_floor.contains(&structure));
        assert!(from_roof.contains(&structure));
        for &s in &from_floor {
            assert_eq!(band_seed(Band::NetherFloor, s), Some(floor));
        }
        for &s in &from_roof {
            assert_eq!(band_seed(Band::NetherRoof, s), Some(roof));
        }
        let both = shared_structure_seeds(Some(&[floor]), Some(&[roof]));
        assert!(both.contains(&structure));
        for &s in &both {
            assert_eq!(band_seed(Band::NetherFloor, s), Some(floor));
            assert_eq!(band_seed(Band::NetherRoof, s), Some(roof));
        }
        assert_eq!(shared_structure_seeds(Some(&[floor]), None), from_floor);
        assert_eq!(shared_structure_seeds(None, Some(&[roof])), from_roof);
    }
    assert!(shared_structure_seeds(None, None).is_empty());
    assert!(structure_seeds(Band::OverworldFloor, 5).is_empty());
}

fn reduced_cells(modulus: Modulus, seed: u64, kind: u64, mix: &mut Mix) -> Vec<Constraint> {
    let b = boundaries(modulus);
    match kind {
        0 => {
            let n = 6 + mix.below(25);
            (0..n).map(|_| narrow_true(modulus, seed, mix, None)).collect()
        }
        1 => {
            let n = 10 + mix.below(51);
            (0..n)
                .map(|_| true_side(modulus, seed, mix.next() & modulus.mask(), b[mix.below(4) as usize]))
                .collect()
        }
        2 => {
            let base = mix.next();
            let n = 6 + mix.below(25);
            (0..n).map(|_| narrow_true(modulus, seed, mix, Some(base))).collect()
        }
        3 => {
            let n = 8 + mix.below(20);
            let mut cells: Vec<Constraint> = (0..n).map(|_| narrow_true(modulus, seed, mix, None)).collect();
            let flipped = &mut cells[0];
            *flipped = if flipped.lo == 0 {
                Constraint::new(modulus, flipped.hash, flipped.hi..modulus.size())
            } else {
                Constraint::new(modulus, flipped.hash, 0..flipped.lo)
            };
            cells
        }
        _ => {
            let mut cells = planted(modulus, seed);
            let extra = mix.below(7);
            cells.extend((0..extra).map(|_| narrow_true(modulus, seed, mix, None)));
            mix.shuffle(&mut cells);
            cells
        }
    }
}

#[test]
fn reduced_width_search_equals_brute_force() {
    let modulus = Modulus::reduced(20);
    let mut mix = Mix(0x5EED_0003);
    let mut pair_tests = 0;
    let mut nonempty = 0;
    for kind in 0..5 {
        for trial in 0..40 {
            let seed = mix.next() & modulus.mask();
            let cells = reduced_cells(modulus, seed, kind, &mut mix);
            let search = Search::new(modulus, &cells);
            pair_tests += search.pair_test_count();
            let expected = brute_force(modulus, &cells);
            let scalar = scan_all(&search, Lanes::Scalar);
            assert_eq!(scalar, expected, "kind {kind} trial {trial}");
            assert_eq!(scan_all(&search, Lanes::Widest), expected, "kind {kind} trial {trial}");
            assert_eq!(scan_in_pieces(&search, &mut mix, 37, Lanes::Widest), expected, "kind {kind} trial {trial}");
            assert_eq!(scan_in_pieces(&search, &mut mix, 11, Lanes::Scalar), expected, "kind {kind} trial {trial}");
            if kind != 3 {
                assert!(expected.contains(&seed));
            }
            nonempty += usize::from(!expected.is_empty());
        }
    }
    assert!(pair_tests > 1000, "{pair_tests}");
    assert!(nonempty >= 160);
}

#[test]
fn boundary_planted_seed_is_always_returned() {
    let mut mix = Mix(0x5EED_0004);
    for bits in [16, 20, 24] {
        let modulus = Modulus::reduced(bits);
        for _ in 0..12 {
            let seed = mix.next() & modulus.mask();
            let cells = reduced_cells(modulus, seed, 4, &mut mix);
            let search = Search::new(modulus, &cells);
            let found = scan_all(&search, Lanes::Widest);
            assert!(found.contains(&seed));
            assert_eq!(scan_all(&search, Lanes::Scalar), found);
            if bits <= 20 {
                assert_eq!(found, brute_force(modulus, &cells));
            }
        }
    }
    let modulus = Modulus::JAVA;
    let seed = mix.next() & modulus.mask();
    let mut cells = planted(modulus, seed);
    cells.extend((0..70).map(|_| narrow_true(modulus, seed, &mut mix, None)));
    let search = Search::new(modulus, &cells);
    let found = scan_all(&search, Lanes::Widest);
    assert!(found.contains(&seed));
    assert!(found.iter().all(|&s| cells.iter().all(|c| c.holds(modulus, s))));
}

#[test]
fn full_width_floor_search_finds_the_known_band_seed() {
    let truth = band_seed(Band::NetherFloor, REAL_WORLD_SEED).unwrap();
    let cells = floor_cells_around(truth, 7_654_320, -3_210_000, 3);
    assert!(cells.iter().all(BedrockCell::narrow));
    assert!(cells.len() >= 2 * MIN_NARROW_CELLS);
    let built = Instant::now();
    let search = BandSearch::new(Band::NetherFloor, &cells).unwrap();
    let plan_time = built.elapsed();
    assert_eq!(search.total(), 1 << 24);

    let started = Instant::now();
    let mut scalar = Vec::new();
    search.scan_with(0..search.total(), &mut scalar, Lanes::Scalar).unwrap();
    let scalar_time = started.elapsed();
    let started = Instant::now();
    let mut widest = Vec::new();
    search.scan_with(0..search.total(), &mut widest, Lanes::Widest).unwrap();
    let widest_time = started.elapsed();
    eprintln!(
        "full width: {} cells, {} pair tests, plan {plan_time:?}, scalar {scalar_time:?}, widest {widest_time:?} (avx512 {}), {} survivors",
        cells.len(),
        search.search().pair_test_count(),
        crate::bedrock::wide_lanes_available(),
        scalar.len()
    );
    assert!(scalar.contains(&truth));
    assert_eq!(scalar, widest);
    assert!(scalar.windows(2).all(|w| w[0] < w[1]));

    let mut mix = Mix(0x5EED_0005);
    let total = search.total();
    let mut cuts: Vec<u64> = (0..2000).map(|_| mix.below(total + 1)).collect();
    cuts.extend([0, total]);
    cuts.sort_unstable();
    let mut ranges: Vec<(u64, u64)> = cuts.windows(2).map(|w| (w[0], w[1])).collect();
    mix.shuffle(&mut ranges);
    let mut pieces = Vec::new();
    for (start, end) in ranges {
        search.scan(start..end, &mut pieces).unwrap();
    }
    pieces.sort_unstable();
    assert_eq!(pieces, scalar);

    for &s in &scalar {
        assert!(cells.iter().all(|c| c.holds_for_band_seed(s)));
    }
    let found: BTreeSet<i64> = scalar.iter().copied().collect();
    let mut checked = 0;
    for d in 0..1i64 << 20 {
        for candidate in [truth ^ d, (truth + d - (1 << 19)) & MASK] {
            if cells.iter().all(|c| c.holds_for_band_seed(candidate)) {
                assert!(found.contains(&candidate), "missed {candidate}");
                checked += 1;
            }
        }
    }
    assert!(checked >= 2);

    let mut shuffled = cells.clone();
    mix.shuffle(&mut shuffled);
    shuffled.extend(cells.iter().take(20).copied());
    shuffled.push(BedrockCell::new(Band::NetherRoof, 1, 124, 1, true).unwrap());
    let rebuilt = BandSearch::new(Band::NetherFloor, &shuffled).unwrap();
    assert_eq!(format!("{:?}", rebuilt.search()), format!("{:?}", search.search()));

    let many = floor_cells_around(truth, -7_654_400, 3_210_000, 12);
    let built = Instant::now();
    let large = BandSearch::new(Band::NetherFloor, &many).unwrap();
    let plan_time = built.elapsed();
    let started = Instant::now();
    let mut found = Vec::new();
    large.scan(0..large.total(), &mut found).unwrap();
    eprintln!("full width: {} cells, plan {plan_time:?}, widest {:?}, {} survivors", many.len(), started.elapsed(), found.len());
    assert!(found.contains(&truth));
}

#[test]
fn band_search_needs_enough_narrow_cells() {
    let truth = band_seed(Band::NetherFloor, REAL_WORLD_SEED).unwrap();
    let cells = floor_cells_around(truth, 160, 160, 3);
    let narrow: Vec<BedrockCell> = cells.iter().copied().take(MIN_NARROW_CELLS).collect();
    assert!(BandSearch::new(Band::NetherFloor, &narrow).is_some());
    assert!(BandSearch::new(Band::NetherFloor, &narrow[1..]).is_none());
    assert!(BandSearch::new(Band::NetherRoof, &cells).is_none());
    assert!(BandSearch::new(Band::OverworldFloor, &cells).is_none());
    let mut wide = narrow[1..].to_vec();
    wide.extend((0..200).map(|i| BedrockCell::new(Band::NetherFloor, i, 2, 0, i % 2 == 0).unwrap()));
    assert!(BandSearch::new(Band::NetherFloor, &wide).is_none());
}

#[test]
fn overflow_is_reported_not_swallowed() {
    let modulus = Modulus::reduced(20);
    let mut mix = Mix(0x5EED_0006);
    let seed = mix.next() & modulus.mask();
    let cells: Vec<Constraint> = (0..2).map(|_| narrow_true(modulus, seed, &mut mix, None)).collect();
    let search = Search::new(modulus, &cells);
    let everything = brute_force(modulus, &cells);
    assert!(everything.len() > MAX_SURVIVORS);
    for lanes in [Lanes::Scalar, Lanes::Widest] {
        let mut out = vec![7u64, 9];
        assert_eq!(search.scan(0..search.total(), &mut out, MAX_SURVIVORS, lanes), Err(Overflow));
        assert_eq!(out, vec![7, 9]);
        assert_eq!(search.scan(0..search.total(), &mut out, everything.len(), lanes), Ok(()));
        assert_eq!(out[2..], everything[..]);
        out.truncate(2);
        assert_eq!(search.scan(0..search.total(), &mut out, everything.len() - 1, lanes), Err(Overflow));
        assert_eq!(out, vec![7, 9]);
    }
}

#[test]
fn end_to_end_recovers_the_world() {
    let world: i64 = -5_871_209_337_046_120_381;
    let structure = world & MASK;
    let floor_truth = band_seed(Band::NetherFloor, world).unwrap();
    let roof_truth = band_seed(Band::NetherRoof, world).unwrap();
    let floor_cells = floor_cells_around(floor_truth, -812_352, 40_016, 2);
    let roof_cells = roof_cells_around(roof_truth, 1_000_000, -999_984, 2);
    let mut all = floor_cells.clone();
    all.extend(&roof_cells);
    let mut floor = Vec::new();
    let floor_search = BandSearch::new(Band::NetherFloor, &all).unwrap();
    floor_search.scan(0..floor_search.total(), &mut floor).unwrap();
    let mut roof = Vec::new();
    let roof_search = BandSearch::new(Band::NetherRoof, &all).unwrap();
    roof_search.scan(0..roof_search.total(), &mut roof).unwrap();
    assert!(floor.contains(&floor_truth));
    assert!(roof.contains(&roof_truth));
    assert_eq!(shared_structure_seeds(Some(&floor), Some(&roof)), vec![structure]);
    assert!(shared_structure_seeds(Some(&floor), None).contains(&structure));
    assert!(shared_structure_seeds(None, Some(&roof)).contains(&structure));

    let overworld = OverworldFloor::new(&overworld_cells_around(world, 30, -77, 12));
    assert_eq!(overworld.len(), 48);
    assert!(overworld.test(world));
    assert_eq!(overworld.world_seeds(structure), vec![world]);
}
