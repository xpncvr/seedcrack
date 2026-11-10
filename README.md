# seedcrack

Recovers a Minecraft world seed from what a client can see in the world, for
1.18 and later. It has tables for 1.21.11, 26.1.1 and 26.2.

The search is a Rust port of the cracking pipeline in
[SeedcrackerX](https://github.com/19MisterX98/SeedcrackerX) by 19MisterX98,
which builds on KaptainWutax's original SeedCracker. Constants and placement
rules were checked against the game's own code and data rather than copied
from SeedcrackerX, so a few finders differ from it.

## Use

```rust
use seedcrack::{Cracker, Dimension, Observation, Version, finders};

let mut cracker = Cracker::new(Version::V26_2);
cracker.add(Observation::HashedSeed(hashed_seed));
for found in finders::scan_chunk(&world, Dimension::Overworld, chunk_x, chunk_z, |_| true) {
    cracker.add(found.observation);
}
while cracker.step(1 << 24) {}
println!("{:?}", cracker.world_seed());
```

`world` is anything that implements `seedcrack::World` (`block`, `biome`,
`min_y`, `max_y`). Each brute-force stage runs a slice at a time through
`Cracker::step`, so a caller can spread it over frames or worker threads.

## seedcrack-wasm

`seedcrack-wasm/` builds the search alone as a small wasm module for Web
Workers, with raw exports and no wasm-bindgen. A page running a `Cracker` hands
each worker a slice of the current job (`Cracker::next_slice`) as bytes
(`Task::to_bytes`), and the worker scans it with `seed_scan`.

## License

AGPL-3.0-or-later. See [LICENSE](LICENSE).

The parts ported from SeedcrackerX and the seedfinding libraries it uses are
MIT-licensed, Copyright (c) 2020 KaptainWutax. Their notice is kept in
[LICENSE-MIT](LICENSE-MIT).
