use std::time::Instant;

use super::{Mix, WORLD, floor_cells_around, liftable_sightings, overworld_cells_around, roof_cells_around};
use crate::bedrock::{Band, BedrockCell, band_seed};
use crate::cracker::{
    BEDROCK_TARGET, BedrockNeed, Cracker, Event, JobKind, Mode, OVERWORLD_BEDROCK_CAP, Slice, Stage, Task, Trust,
};
use crate::hash;
use crate::lcg::MASK;
use crate::version::{GenRange, Version};
use crate::placement::StructureSet;
use crate::world::{Observation, StructureObservation};

const OTHER_WORLD: i64 = -5_871_209_337_046_120_381;

fn floor_of(world: i64) -> Vec<BedrockCell> {
    floor_cells_around(band_seed(Band::NetherFloor, world).unwrap(), -812_352, 40_016, 4)
}

fn roof_of(world: i64) -> Vec<BedrockCell> {
    roof_cells_around(band_seed(Band::NetherRoof, world).unwrap(), 1_000_000, -999_984, 4)
}

fn overworld_of(world: i64, columns: i32) -> Vec<BedrockCell> {
    overworld_cells_around(world, 30, -77, columns)
}

fn feed(cracker: &mut Cracker, cells: &[BedrockCell]) -> usize {
    cells.iter().filter(|&&c| cracker.add(Observation::Bedrock(c))).count()
}

fn run(cracker: &mut Cracker) -> Vec<Event> {
    let mut events = Vec::new();
    while cracker.step(1 << 26) {
        events.extend(cracker.drain_events());
    }
    events.extend(cracker.drain_events());
    events
}

fn run_tasks(cracker: &mut Cracker, mix: &mut Mix, budget: u64) -> Vec<Event> {
    let mut events = Vec::new();
    let mut task: Option<Task> = None;
    let mut lost_one = false;
    loop {
        let mut out: Vec<Slice> = Vec::new();
        while out.len() < 8 {
            match cracker.next_slice(budget) {
                Some(s) => out.push(s),
                None => break,
            }
        }
        events.extend(cracker.drain_events());
        if out.is_empty() {
            break;
        }
        if task.as_ref().is_none_or(|t| t.id() != out[0].job) {
            let bytes = cracker.task().unwrap().to_bytes();
            task = Some(Task::from_bytes(&bytes).expect("round trip"));
        }
        mix.shuffle(&mut out);
        let t = task.as_ref().unwrap();
        for s in out {
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
    events.extend(cracker.drain_events());
    events
}

fn nether_cracker(world: i64) -> Cracker {
    let mut cracker = Cracker::new(Version::V26_2);
    assert_eq!(feed(&mut cracker, &floor_of(world)), BEDROCK_TARGET);
    assert_eq!(feed(&mut cracker, &roof_of(world)), BEDROCK_TARGET);
    cracker
}

#[test]
fn nether_bedrock_and_hash_find_the_world() {
    let mut stepped = nether_cracker(OTHER_WORLD);
    stepped.add(Observation::HashedSeed(hash::obfuscate(OTHER_WORLD)));
    let mut sliced = stepped.clone();

    let started = Instant::now();
    let events = run(&mut stepped);
    eprintln!("nether end to end through step: {:?}", started.elapsed());
    assert_eq!(stepped.world_seed(), Some(OTHER_WORLD), "{events:?}");
    assert_eq!(events.iter().filter(|e| **e == Event::Started(Stage::Bedrock)).count(), 2, "{events:?}");
    assert!(events.contains(&Event::Started(Stage::WorldByHash)), "{events:?}");
    for band in [Band::NetherFloor, Band::NetherRoof] {
        assert!(events.iter().any(|e| matches!(e, Event::BandSeeds(b, n) if *b == band && *n >= 1)), "{events:?}");
        assert!(stepped.bedrock_seeds(band).is_some());
    }
    assert_eq!(stepped.bedrock_structure_seeds(), Some(&[OTHER_WORLD & MASK][..]));
    assert_eq!(stepped.structure_seeds(), Some(&[OTHER_WORLD & MASK][..]));
    assert_eq!(stepped.stage(), Stage::Done);

    let started = Instant::now();
    let sliced_events = run_tasks(&mut sliced, &mut Mix(0x5EED_0101), 1 << 20);
    eprintln!("nether end to end through tasks: {:?}", started.elapsed());
    assert_eq!(sliced.world_seeds(), stepped.world_seeds(), "{sliced_events:?}");
    assert_eq!(sliced.structure_seeds(), stepped.structure_seeds());
    assert_eq!(sliced.bedrock_structure_seeds(), stepped.bedrock_structure_seeds());
    for band in Band::ALL {
        assert_eq!(sliced.bedrock_seeds(band), stepped.bedrock_seeds(band));
    }
    assert_eq!(sliced_events, events);
}

#[test]
fn floor_alone_gives_candidates_and_the_roof_narrows_them() {
    let truth = OTHER_WORLD & MASK;
    let mut cracker = Cracker::new(Version::V26_2);
    assert_eq!(cracker.bedrock_hint(), Some(BedrockNeed::VisitNether));
    let floor = floor_of(OTHER_WORLD);
    feed(&mut cracker, &floor[..10]);
    assert_eq!(cracker.bedrock_hint(), Some(BedrockNeed::Collecting));
    feed(&mut cracker, &floor);
    let events = run(&mut cracker);
    let candidates = cracker.structure_seeds().unwrap().to_vec();
    eprintln!("floor alone: {} band seeds, {} structure seeds", cracker.bedrock_seeds(Band::NetherFloor).unwrap(), candidates.len());
    assert!(candidates.contains(&truth), "{events:?}");
    assert_eq!(cracker.bedrock_seeds(Band::NetherRoof), None);
    assert_eq!(cracker.bedrock_hint(), Some(BedrockNeed::VisitOverworld));

    feed(&mut cracker, &roof_of(OTHER_WORLD));
    let events = run(&mut cracker);
    assert_eq!(cracker.structure_seeds(), Some(&[truth][..]), "{events:?}");
    if candidates.len() > 1 {
        assert!(events.contains(&Event::StructureSeeds(vec![truth])), "{events:?}");
    }
}

#[test]
fn overworld_bedrock_completes_the_world_without_a_hash() {
    let mut cracker = nether_cracker(OTHER_WORLD);
    assert_eq!(feed(&mut cracker, &overworld_of(OTHER_WORLD, 64)), 256);
    let events = run(&mut cracker);
    assert!(events.contains(&Event::Started(Stage::WorldByDecorators)), "{events:?}");
    assert_eq!(cracker.world_seed(), Some(OTHER_WORLD), "{events:?}");
    assert_eq!(cracker.bedrock_hint(), None);

    let mut waiting = nether_cracker(OTHER_WORLD);
    feed(&mut waiting, &overworld_of(OTHER_WORLD, 63));
    let events = run(&mut waiting);
    assert!(!events.contains(&Event::Started(Stage::WorldByDecorators)), "{events:?}");
    assert_eq!(waiting.world_seeds(), None);
    assert_eq!(waiting.bedrock_hint(), Some(BedrockNeed::VisitOverworld));
}

#[test]
fn few_overworld_cells_report_every_survivor_and_more_cells_narrow() {
    let mut cracker = nether_cracker(OTHER_WORLD);
    cracker.overworld_min = 64;
    let cells = overworld_cells_around(OTHER_WORLD, 270, -77, 128);
    assert_eq!(feed(&mut cracker, &cells[..64]), 64);
    let events = run(&mut cracker);
    let worlds = cracker.world_seeds().unwrap().to_vec();
    eprintln!("64 overworld cells leave {} world seeds", worlds.len());
    assert!(worlds.len() > 1, "{worlds:?}");
    assert!(worlds.contains(&OTHER_WORLD));
    assert!(events.contains(&Event::WorldSeeds(worlds.clone())));
    for &w in &worlds {
        assert_eq!(w & MASK, OTHER_WORLD & MASK);
    }

    feed(&mut cracker, &cells[64..]);
    let events = run(&mut cracker);
    assert_eq!(cracker.world_seed(), Some(OTHER_WORLD), "{events:?}");
    assert!(events.contains(&Event::WorldSeeds(vec![OTHER_WORLD])), "{events:?}");
}

#[test]
fn bedrock_agrees_with_structures() {
    let truth = WORLD & MASK;
    let mut together = Cracker::new(Version::V26_2);
    for o in liftable_sightings(WORLD) {
        together.add(o);
    }
    feed(&mut together, &floor_of(WORLD));
    together.add(Observation::HashedSeed(hash::obfuscate(WORLD)));
    let events = run(&mut together);
    assert!(!events.contains(&Event::Started(Stage::LiftLow)), "{events:?}");
    assert_eq!(together.structure_seeds(), Some(&[truth][..]), "{events:?}");
    assert_eq!(together.world_seed(), Some(WORLD), "{events:?}");

    let mut lifted = Cracker::new(Version::V26_2);
    for o in liftable_sightings(WORLD) {
        lifted.add(o);
    }
    let events = run(&mut lifted);
    let seeds = lifted.structure_seeds().unwrap().to_vec();
    assert!(seeds.len() > 1 && seeds.contains(&truth), "{events:?}");
    feed(&mut lifted, &floor_of(WORLD));
    let events = run(&mut lifted);
    assert!(events.contains(&Event::Started(Stage::Bedrock)), "{events:?}");
    assert!(!lifted.conflict());
    assert_eq!(lifted.structure_seeds(), Some(&[truth][..]), "{events:?}");
}

fn conflicted() -> (Cracker, Vec<i64>) {
    let mut cracker = Cracker::new(Version::V26_2);
    for o in liftable_sightings(WORLD) {
        cracker.add(o);
    }
    run(&mut cracker);
    let lifted = cracker.structure_seeds().unwrap().to_vec();
    assert!(!lifted.contains(&(OTHER_WORLD & MASK)));
    feed(&mut cracker, &floor_of(OTHER_WORLD));
    feed(&mut cracker, &roof_of(OTHER_WORLD));
    cracker.add(Observation::HashedSeed(hash::obfuscate(OTHER_WORLD)));
    let events = run(&mut cracker);
    assert!(events.contains(&Event::BedrockConflict), "{events:?}");
    assert!(!events.iter().any(|e| matches!(e, Event::Started(s) if *s != Stage::Bedrock)), "{events:?}");
    (cracker, lifted)
}

#[test]
fn conflict_holds_until_resolved() {
    let (mut cracker, lifted) = conflicted();
    assert!(cracker.conflict());
    assert_eq!(cracker.structure_seeds(), Some(&lifted[..]));
    assert_eq!(cracker.bedrock_structure_seeds(), Some(&[OTHER_WORLD & MASK][..]));
    assert_eq!(cracker.bedrock_cells(Band::NetherFloor), BEDROCK_TARGET);
    assert_eq!(cracker.structures().len(), liftable_sightings(WORLD).len());
    assert_eq!(cracker.world_seeds(), None);
    assert_eq!(cracker.stage(), Stage::Idle);
    assert!(!cracker.step(1 << 26));
    assert!(cracker.task().is_none());
    assert_eq!(cracker.bedrock_hint(), None);

    let mut by_bedrock = cracker.clone();
    by_bedrock.resolve_conflict(Trust::Bedrock);
    assert!(!by_bedrock.conflict());
    assert!(by_bedrock.structures().is_empty());
    let events = run(&mut by_bedrock);
    assert_eq!(by_bedrock.structure_seeds(), Some(&[OTHER_WORLD & MASK][..]), "{events:?}");
    assert_eq!(by_bedrock.world_seed(), Some(OTHER_WORLD), "{events:?}");

    let mut by_structures = cracker.clone();
    by_structures.resolve_conflict(Trust::Structures);
    assert!(!by_structures.conflict());
    assert_eq!(by_structures.structure_seeds(), Some(&lifted[..]));
    assert_eq!(by_structures.structures().len(), liftable_sightings(WORLD).len());
    for band in Band::ALL {
        assert_eq!(by_structures.bedrock_cells(band), 0);
        assert_eq!(by_structures.bedrock_seeds(band), None);
    }
    assert_eq!(by_structures.bedrock_structure_seeds(), None);
    assert!(!by_structures.bedrock_enabled());
    assert_eq!(feed(&mut by_structures, &floor_of(OTHER_WORLD)), 0);
    by_structures.set_bedrock(true);
    assert_eq!(feed(&mut by_structures, &floor_of(OTHER_WORLD)), 0);
    let events = run(&mut by_structures);
    assert!(events.contains(&Event::HashMismatch), "{events:?}");
    assert_eq!(by_structures.structure_seeds(), Some(&lifted[..]));
    by_structures.clear();
    assert!(by_structures.bedrock_enabled());
    assert_eq!(feed(&mut by_structures, &floor_of(OTHER_WORLD)), BEDROCK_TARGET);
}

#[test]
fn caps_duplicates_and_switches() {
    let mut cracker = Cracker::new(Version::V26_2);
    let floor = floor_of(WORLD);
    assert!(floor.len() > BEDROCK_TARGET);
    assert!(floor.iter().all(BedrockCell::narrow));
    for (i, &c) in floor.iter().enumerate() {
        assert_eq!(cracker.add(Observation::Bedrock(c)), i < BEDROCK_TARGET);
    }
    assert!(!cracker.add(Observation::Bedrock(floor[0])));
    assert_eq!(cracker.bedrock_cells(Band::NetherFloor), BEDROCK_TARGET);
    assert_eq!(cracker.bedrock_cells(Band::NetherRoof), 0);

    let wide = BedrockCell::new(Band::NetherRoof, 5, 124, 5, true).unwrap();
    assert!(!wide.narrow());
    assert!(!cracker.add(Observation::Bedrock(wide)));
    assert!(BedrockCell::new(Band::NetherRoof, 5, 4, 5, true).is_none());
    let roof = roof_of(WORLD);
    assert!(cracker.add(Observation::Bedrock(roof[0])));
    assert!(!cracker.add(Observation::Bedrock(roof[0])));
    assert_eq!(cracker.bedrock_cells(Band::NetherRoof), 1);
    assert_eq!(cracker.bedrock_cells(Band::NetherFloor), BEDROCK_TARGET);

    let overworld = overworld_of(WORLD, 200);
    assert!(overworld.iter().any(|c| !c.narrow()));
    assert_eq!(feed(&mut cracker, &overworld), OVERWORLD_BEDROCK_CAP);
    assert!(!cracker.add(Observation::Bedrock(overworld[0])));
    assert_eq!(cracker.bedrock_cells(Band::OverworldFloor), OVERWORLD_BEDROCK_CAP);

    cracker.set_bedrock(false);
    assert!(!cracker.bedrock_enabled());
    for band in Band::ALL {
        assert_eq!(cracker.bedrock_cells(band), 0);
    }
    assert_eq!(feed(&mut cracker, &floor), 0);
    assert_eq!(feed(&mut cracker, &overworld), 0);
    assert!(!cracker.step(1 << 26));
    cracker.set_bedrock(true);
    assert_eq!(feed(&mut cracker, &floor[..5]), 5);

    cracker.clear();
    assert!(cracker.bedrock_enabled());
    for band in Band::ALL {
        assert_eq!(cracker.bedrock_cells(band), 0);
    }
    cracker.set_bedrock(false);
    cracker.clear();
    assert!(!cracker.bedrock_enabled());
}

#[test]
fn task_bytes_round_trip_bedrock_and_overworld() {
    let range = GenRange { min_y: -64, height: 384 };
    let structures: Vec<_> = liftable_sightings(WORLD)
        .into_iter()
        .filter_map(|o| match o {
            Observation::Structure(s) => Some(s),
            _ => None,
        })
        .collect();
    let floor: Vec<BedrockCell> = floor_of(OTHER_WORLD).into_iter().take(BEDROCK_TARGET).collect();
    let roof: Vec<BedrockCell> = roof_of(OTHER_WORLD).into_iter().take(BEDROCK_TARGET).collect();
    let overworld = overworld_of(OTHER_WORLD, 64);
    let kinds = [
        (JobKind::bedrock(Band::NetherFloor, floor.clone(), 4096).unwrap(), band_seed(Band::NetherFloor, OTHER_WORLD).unwrap()),
        (JobKind::bedrock(Band::NetherRoof, roof.clone(), 4096).unwrap(), band_seed(Band::NetherRoof, OTHER_WORLD).unwrap()),
        (JobKind::world(vec![OTHER_WORLD & MASK], overworld.clone()), OTHER_WORLD),
    ];
    assert!(JobKind::bedrock(Band::NetherRoof, floor.clone(), 4096).is_none());
    for mode in [Mode::Normal, Mode::Fast] {
        for (i, (kind, truth)) in kinds.iter().enumerate() {
            let task = Task::with(70 + i as u64, kind.clone(), mode, structures.clone(), Vec::new(), range);
            let bytes = task.to_bytes();
            assert_eq!(bytes[0], 3);
            let back = Task::from_bytes(&bytes).expect("round trip");
            assert_eq!(back.id(), task.id());
            assert_eq!(back.stage(), task.stage());
            assert_eq!(format!("{:?}", back.parts().0), format!("{:?}", task.parts().0));
            assert_eq!(back.to_bytes(), bytes);
            let total = kind.total();
            let found = back.scan(0..total);
            assert_eq!(found, task.scan(0..total));
            assert!(found.contains(truth), "{found:?}");
            let mut format_two = bytes.clone();
            format_two[0] = 2;
            assert!(Task::from_bytes(&format_two).is_none());
            assert!(Task::from_bytes(&bytes[..bytes.len() - 1]).is_none());
            let first_flag = if i < 2 { 32 } else { 39 };
            assert!(bytes[first_flag] <= 1);
            let mut bad_flag = bytes.clone();
            bad_flag[first_flag] = 2;
            assert!(Task::from_bytes(&bad_flag).is_none());
            let mut flipped = bytes.clone();
            flipped[first_flag] ^= 1;
            assert!(Task::from_bytes(&flipped).is_some_and(|t| t.scan(0..total) != found));
        }
    }
    let task = Task::with(80, kinds[0].0.clone(), Mode::Normal, Vec::new(), Vec::new(), range);
    let mut bytes = task.to_bytes();
    assert_eq!(bytes[10], 5);
    bytes[11] = 2;
    assert!(Task::from_bytes(&bytes).is_none());
    bytes[11] = 1;
    assert!(Task::from_bytes(&bytes).is_none());
}

#[test]
fn overflow_raises_an_event_and_leaves_no_result() {
    for sliced in [false, true] {
        let mut cracker = Cracker::new(Version::V26_2);
        cracker.survivor_limit = 0;
        feed(&mut cracker, &floor_of(OTHER_WORLD));
        let events = if sliced { run_tasks(&mut cracker, &mut Mix(0x5EED_0102), 1 << 21) } else { run(&mut cracker) };
        assert!(events.contains(&Event::BedrockOverflow(Band::NetherFloor)), "{events:?}");
        assert!(!events.iter().any(|e| matches!(e, Event::BandSeeds(..) | Event::StructureSeeds(_))), "{events:?}");
        assert_eq!(cracker.bedrock_seeds(Band::NetherFloor), None);
        assert_eq!(cracker.bedrock_structure_seeds(), None);
        assert_eq!(cracker.structure_seeds(), None);
        assert!(!cracker.step(1 << 26));
    }
}

#[test]
fn fast_and_normal_agree_on_bedrock() {
    let mut results = Vec::new();
    for mode in [Mode::Normal, Mode::Fast] {
        let mut cracker = Cracker::new(Version::V26_2);
        cracker.set_mode(mode);
        feed(&mut cracker, &floor_of(OTHER_WORLD));
        let floor_events = run(&mut cracker);
        let floor_only = cracker.structure_seeds().map(<[i64]>::to_vec);
        feed(&mut cracker, &roof_of(OTHER_WORLD));
        feed(&mut cracker, &overworld_of(OTHER_WORLD, 64));
        let events = run(&mut cracker);
        results.push((floor_events, floor_only, events, cracker.world_seeds().map(<[i64]>::to_vec)));
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0].3, Some(vec![OTHER_WORLD]));
}


#[test]
fn a_hash_confirmed_seed_survives_a_sighting_that_does_not_fit() {
    let mut cracker = nether_cracker(OTHER_WORLD);
    cracker.add(Observation::HashedSeed(hash::obfuscate(OTHER_WORLD)));
    let events = run(&mut cracker);
    assert_eq!(cracker.confirmed_seed(), Some(OTHER_WORLD), "{events:?}");

    let structure_seed = OTHER_WORLD & MASK;
    let set = StructureSet::Shipwrecks;
    let p = set.placement();
    let (fit_x, fit_z) = (0..64)
        .map(|r| p.potential_chunk(structure_seed, r * p.spacing, 0))
        .find(|&(x, z)| p.is_structure_chunk(structure_seed, x, z))
        .expect("a shipwreck chunk");
    let fitting = StructureObservation { set, chunk_x: fit_x, chunk_z: fit_z };
    assert!(cracker.add(Observation::Structure(fitting)));
    assert!(!cracker.drain_events().iter().any(|e| matches!(e, Event::SightingIgnored(_))));

    let wrong_x = if p.region_of(fit_x + 1, fit_z) == p.region_of(fit_x, fit_z) { fit_x + 1 } else { fit_x - 1 };
    let wrong = StructureObservation { set, chunk_x: wrong_x, chunk_z: fit_z };
    assert!(!p.is_structure_chunk(structure_seed, wrong_x, fit_z));
    assert!(!cracker.add(Observation::Structure(wrong)));
    assert_eq!(cracker.drain_events(), vec![Event::SightingIgnored(wrong)]);
    let events = run(&mut cracker);
    assert_eq!(cracker.world_seed(), Some(OTHER_WORLD), "{events:?}");
    assert_eq!(cracker.structure_seeds(), Some(&[structure_seed][..]));
    assert_eq!(cracker.stage(), Stage::Done);
    assert!(!cracker.structures().contains(&wrong));

    let mut unconfirmed = nether_cracker(OTHER_WORLD);
    run(&mut unconfirmed);
    assert_eq!(unconfirmed.confirmed_seed(), None);
    assert!(unconfirmed.add(Observation::Structure(wrong)));
}
