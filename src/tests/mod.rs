mod vectors;

use std::collections::BTreeSet;

use crate::cracker::{Cracker, Event, Stage};
use crate::decorator::{DecoratorKind, DecoratorObservation};
use crate::hash;
use crate::lcg::{JavaRandom, MASK};
use crate::pillar::{self, PillarObservation};
use crate::placement::StructureSet;
use crate::version::{GenRange, Version};
use crate::world::{Observation, StructureObservation};
use crate::xoroshiro::WorldgenRandom;

fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

#[test]
fn java_random_matches() {
    for &(seed, v) in vectors::RANDOM {
        let mut r = JavaRandom::new(seed);
        assert_eq!(r.next_int(16) as i64, v[0], "seed {seed}");
        assert_eq!(r.next_int(24) as i64, v[1], "seed {seed}");
        assert_eq!(r.next_int(20) as i64, v[2], "seed {seed}");
        assert_eq!(r.next_int(7) as i64, v[3], "seed {seed}");
        assert_eq!(r.next_int_any() as i64, v[4], "seed {seed}");
        assert_eq!(r.next_long(), v[5], "seed {seed}");
        assert_eq!(r.next_float().to_bits() as i32 as i64, v[6], "seed {seed}");
        assert_eq!(r.next_double().to_bits() as i64, v[7], "seed {seed}");
    }
}

#[test]
fn xoroshiro_matches() {
    for &(seed, v) in vectors::XORO {
        let mut r = WorldgenRandom::new();
        r.set_seed(seed);
        assert_eq!(r.next_int(16) as i64, v[0], "seed {seed}");
        assert_eq!(r.next_int(320) as i64, v[1], "seed {seed}");
        assert_eq!(r.next_int(7) as i64, v[2], "seed {seed}");
        assert_eq!(r.next_long(), v[3], "seed {seed}");
        assert_eq!(r.next_float().to_bits() as i32 as i64, v[4], "seed {seed}");
        assert_eq!(r.set_decoration_seed(seed, 160, -320), v[5], "seed {seed}");
    }
}

#[test]
fn every_set_is_in_the_table() {
    let names: BTreeSet<&str> = vectors::POTENTIAL.iter().map(|r| r.0).collect();
    assert_eq!(names.len(), StructureSet::ALL.len());
    for name in names {
        assert!(StructureSet::from_name(name).is_some(), "{name}");
    }
}

#[test]
fn potential_chunks_match() {
    for &(name, seed, cx, cz, px, pz) in vectors::POTENTIAL {
        let set = StructureSet::from_name(name).unwrap();
        assert_eq!(set.placement().potential_chunk(seed, cx, cz), (px, pz), "{name} seed {seed} chunk {cx},{cz}");
    }
}

#[test]
fn structure_chunks_match() {
    for &(name, seed, w, want) in vectors::STRUCTURE_CHUNKS {
        let p = StructureSet::from_name(name).unwrap().placement();
        let mut got = Vec::new();
        for x in -w..w {
            for z in -w..w {
                if p.is_structure_chunk(seed, x, z) {
                    got.push((x, z));
                }
            }
        }
        assert_eq!(got, want, "{name} seed {seed}");
    }
}

#[test]
fn pillars_match() {
    for &(seed, p, h) in vectors::PILLARS {
        assert_eq!(pillar::pillar_seed(seed), p, "seed {seed}");
        assert_eq!(pillar::heights(p), h, "pillar seed {p}");
        assert!(PillarObservation { heights: h.map(Some) }.candidates().contains(&p));
    }
}

#[test]
fn time_machine_inverts_pillar_seed() {
    for &(seed, p, _) in vectors::PILLARS {
        let s = seed & MASK;
        let mut r = JavaRandom::new(s);
        r.next(32);
        r.next(32);
        let state = r.state();
        assert_eq!(((state >> 16) & 0xFFFF) as u16, p);
        let partial = ((((state >> 32) & 0xFFFF) << 16) | (state & 0xFFFF)) as u32;
        assert_eq!(pillar::structure_seed_from_pillar(partial, p), s, "seed {seed}");
    }
}

#[test]
fn hash_matches_guava() {
    for &(seed, h) in vectors::HASH {
        assert_eq!(hash::obfuscate(seed), h, "seed {seed}");
        assert_eq!(hash::world_seeds_from_hash(seed & MASK, h), vec![seed]);
    }
}

#[test]
fn dungeon_attempts_match() {
    let range = GenRange::OVERWORLD;
    let mut rand = WorldgenRandom::new();
    for &(world, _, _, rooms, deep) in vectors::DUNGEONS {
        for (kind, list) in [(DecoratorKind::Dungeon, &rooms[..]), (DecoratorKind::DeepDungeon, &deep[..])] {
            for &(x, y, z) in list {
                let o = DecoratorObservation { kind, x, y, z, height_above_surface: None };
                assert!(o.test(world, range, &mut rand), "{kind:?} world {world} at {x},{y},{z}");
                let far = DecoratorObservation { y: 1000, ..o };
                assert!(!far.test(world, range, &mut rand));
            }
        }
    }
}

#[test]
fn wells_and_gateways_match() {
    let range = GenRange::OVERWORLD;
    let mut rand = WorldgenRandom::new();
    for &(world, _, _, x, z) in vectors::WELLS {
        let o = DecoratorObservation { kind: DecoratorKind::DesertWell, x, y: 70, z, height_above_surface: None };
        assert!(o.test(world, range, &mut rand), "well world {world} at {x},{z}");
    }
    for &(world, _, _, x, z, dy) in vectors::GATEWAYS {
        let o = DecoratorObservation { kind: DecoratorKind::EndGateway, x, y: 70, z, height_above_surface: Some(dy) };
        assert!(o.test(world, range, &mut rand), "gateway world {world} at {x},{z}");
        assert!(DecoratorObservation { height_above_surface: None, ..o }.test(world, range, &mut rand));
        let wrong = if dy == 9 { 3 } else { dy + 1 };
        assert!(!DecoratorObservation { height_above_surface: Some(wrong), ..o }.test(world, range, &mut rand));
    }
}

const WORLD: i64 = -4_172_144_997_902_289_642;

fn liftable_sightings(world: i64) -> Vec<Observation> {
    let mut out = Vec::new();
    for set in [
        StructureSet::DesertPyramids,
        StructureSet::Igloos,
        StructureSet::JungleTemples,
        StructureSet::SwampHuts,
        StructureSet::Shipwrecks,
    ] {
        let p = set.placement();
        for (rx, rz) in [(0, 0), (1, 0), (-1, 2)] {
            let (cx, cz) = p.potential_chunk(world, rx * p.spacing, rz * p.spacing);
            out.push(Observation::Structure(StructureObservation { set, chunk_x: cx, chunk_z: cz }));
        }
    }
    out
}

fn run(cracker: &mut Cracker) -> Vec<Event> {
    let mut events = Vec::new();
    while cracker.step_parallel(1 << 26, threads()) {
        events.extend(cracker.drain_events());
    }
    events.extend(cracker.drain_events());
    events
}

#[test]
fn cracks_by_lifting_then_hash_or_dungeons() {
    let mut cracker = Cracker::new(Version::V26_2);
    for o in liftable_sightings(WORLD) {
        assert!(cracker.add(o));
    }
    let (_, _, _, outposts) = vectors::STRUCTURE_CHUNKS
        .iter()
        .find(|r| r.0 == "pillager_outposts" && r.1 == WORLD)
        .unwrap();
    for &(cx, cz) in outposts.iter().take(2) {
        cracker.add(Observation::Structure(StructureObservation { set: StructureSet::PillagerOutposts, chunk_x: cx, chunk_z: cz }));
    }
    assert!(cracker.structure_bits() >= crate::cracker::LIFT_BITS);

    let events = run(&mut cracker);
    assert!(events.contains(&Event::Started(Stage::LiftLow)));
    assert_eq!(cracker.structure_seeds(), Some(&[WORLD & MASK][..]), "{events:?}");

    let mut by_hash = cracker.clone();
    by_hash.add(Observation::HashedSeed(hash::obfuscate(WORLD)));
    run(&mut by_hash);
    assert_eq!(by_hash.world_seed(), Some(WORLD));

    let mut by_dungeons = cracker.clone();
    by_dungeons.add(Observation::HashedSeed(12345));
    for &(world, _, _, rooms, _) in vectors::DUNGEONS {
        if world == WORLD {
            let (x, y, z) = rooms[0];
            by_dungeons.add(Observation::Decorator(DecoratorObservation {
                kind: DecoratorKind::Dungeon,
                x,
                y,
                z,
                height_above_surface: None,
            }));
        }
    }
    let events = run(&mut by_dungeons);
    assert!(events.contains(&Event::HashMismatch), "{events:?}");
    assert_eq!(by_dungeons.world_seed(), Some(WORLD), "{events:?}");
}

#[test]
fn waits_for_data_and_drops_duplicates() {
    let mut cracker = Cracker::new(Version::V26_2);
    for o in liftable_sightings(WORLD).into_iter().take(LIFT_SMALL) {
        cracker.add(o);
    }
    assert!(!cracker.step(1));
    assert_eq!(cracker.stage(), Stage::Idle);
    let p = StructureSet::DesertPyramids.placement();
    let (cx, cz) = p.potential_chunk(WORLD, 5 * p.spacing, 5 * p.spacing);
    let o = Observation::Structure(StructureObservation { set: StructureSet::DesertPyramids, chunk_x: cx, chunk_z: cz });
    assert!(cracker.add(o));
    assert!(!cracker.add(o));
}

#[test]
fn wrong_sighting_finds_nothing() {
    let mut cracker = Cracker::new(Version::V26_2);
    let mut sightings = liftable_sightings(WORLD);
    if let Observation::Structure(s) = &mut sightings[0] {
        s.chunk_x += 1;
    }
    for o in sightings {
        cracker.add(o);
    }
    let events = run(&mut cracker);
    assert_eq!(cracker.structure_seeds(), Some(&[][..]), "{events:?}");
    assert!(events.contains(&Event::NoStructureSeed));
}

const LIFT_SMALL: usize = 3;

#[test]
fn merge_rejects_without_changing() {
    let mut held = PillarObservation::default();
    held.heights[0] = Some(76);
    let mut other = PillarObservation::default();
    other.heights[0] = Some(79);
    other.heights[1] = Some(82);
    let before = held;
    assert!(!held.merge(&other));
    assert_eq!(held, before);
}

#[test]
fn inconsistent_pillars_do_not_discard_lifted_seed() {
    let mut cracker = Cracker::new(Version::V26_2);
    cracker.add(Observation::Pillars(PillarObservation { heights: [Some(76); 10] }));
    for o in liftable_sightings(WORLD) {
        cracker.add(o);
    }
    let events = run(&mut cracker);
    assert!(events.contains(&Event::PillarsInconsistent), "{events:?}");
    assert_eq!(cracker.structure_seeds(), Some(&[WORLD & MASK][..]), "{events:?}");
}

#[test]
#[ignore]
fn cracks_by_pillars() {
    let mut cracker = Cracker::new(Version::V26_2);
    let heights = pillar::heights(pillar::pillar_seed(WORLD));
    cracker.add(Observation::Pillars(PillarObservation { heights: heights.map(Some) }));
    let p = StructureSet::EndCities.placement();
    for (rx, rz) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0)] {
        let (cx, cz) = p.potential_chunk(WORLD, rx * p.spacing, rz * p.spacing);
        cracker.add(Observation::Structure(StructureObservation { set: StructureSet::EndCities, chunk_x: cx, chunk_z: cz }));
    }
    for o in liftable_sightings(WORLD).into_iter().take(LIFT_SMALL) {
        cracker.add(o);
    }
    cracker.add(Observation::HashedSeed(hash::obfuscate(WORLD)));
    let events = run(&mut cracker);
    assert!(events.contains(&Event::Started(Stage::Pillars)), "{events:?}");
    assert_eq!(cracker.world_seed(), Some(WORLD), "{events:?}");
}

#[test]
fn hints_follow_the_stage() {
    use crate::finders::{FinderKind, Yields};

    let cracker = Cracker::new(Version::V26_2);
    let hints = cracker.hints();
    assert!(!hints.is_empty());
    let first = hints[0];
    assert_eq!(first.rank, 0);
    assert!(matches!(first.finder.yields(), Yields::Structure(s) if s.placement().liftable()));
    assert_eq!(first.finder, FinderKind::Outpost, "{hints:?}");
    for pair in hints.windows(2) {
        assert!((pair[0].rank, -pair[0].bits) <= (pair[1].rank, -pair[1].bits), "{hints:?}");
    }

    let mut cracker = Cracker::new(Version::V26_2);
    for o in liftable_sightings(WORLD) {
        cracker.add(o);
    }
    run(&mut cracker);
    assert!(cracker.hints().iter().all(|h| matches!(h.finder.yields(), Yields::Decorator(_))));
    cracker.add(Observation::HashedSeed(hash::obfuscate(WORLD)));
    assert!(cracker.hints().is_empty());
}

#[test]
fn slices_through_tasks_match_step() {
    use crate::cracker::{Slice, Task};

    let mut cracker = Cracker::new(Version::V26_2);
    for o in liftable_sightings(WORLD) {
        cracker.add(o);
    }
    cracker.add(Observation::Decorator(DecoratorObservation {
        kind: DecoratorKind::EndGateway,
        x: 1,
        y: 2,
        z: 3,
        height_above_surface: Some(4),
    }));
    cracker.add(Observation::HashedSeed(hash::obfuscate(WORLD)));

    let mut task: Option<Task> = None;
    let mut lost_one = false;
    loop {
        let mut out: Vec<Slice> = Vec::new();
        while out.len() < 8 {
            match cracker.next_slice(1 << 26) {
                Some(s) => out.push(s),
                None => break,
            }
        }
        if out.is_empty() {
            break;
        }
        if task.as_ref().is_none_or(|t| t.id() != out[0].job) {
            let bytes = cracker.task().unwrap().to_bytes();
            task = Some(Task::from_bytes(&bytes).expect("round trip"));
        }
        let t = task.as_ref().unwrap();
        for s in out.into_iter().rev() {
            if !lost_one {
                lost_one = true;
                cracker.requeue(s);
                continue;
            }
            let found = t.scan(s.range.clone());
            cracker.complete(s, &found);
        }
    }
    assert!(lost_one);
    assert_eq!(cracker.world_seed(), Some(WORLD));
}

#[test]
fn task_bytes_reject_garbage() {
    assert!(crate::cracker::Task::from_bytes(&[]).is_none());
    assert!(crate::cracker::Task::from_bytes(&[99, 0, 0]).is_none());
}
