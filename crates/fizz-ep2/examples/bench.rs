// Times the slice's simulation step natively: a wall-clock average, and a run that callgrind can count instructions over.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: cargo run --release -p fizz-ep2 --example bench [repetitions]
//
// Each repetition plays the whole slice script (1,681 ticks) from a fresh level, stepping only: no transcript, no hashing and
// no drawing, so the figure is the rules' cost. The default is 200 repetitions. Wall-clock time depends on the host and is an
// extra; the comparable figure is instructions per tick, which does not:
//
//   cargo build --release -p fizz-ep2 --example bench
//   valgrind --tool=callgrind --callgrind-out-file=/dev/null --toggle-collect='fizz_ep2::world::step' \
//       target/release/examples/bench 1
//
// divides the "Collected" count by 1,681 (`bench 1` is one repetition). Without `--toggle-collect` the count includes the level
// build. The DOS build's cost is measured by `FIZZ2.EXE /BENCH`; see docs/PLATFORM.md for how the two relate.
use fizz_ep2::{SLICE_SCRIPT, SLICE_TICKS, level, step};
use fizz_sim::{Buttons, InputFrame};
use std::hint::black_box;
use std::time::Instant;

fn play_once() -> u64 {
    let mut world = level::world();
    let mut prev = Buttons::NONE;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            step(&mut world, InputFrame::from_transition(prev, now));
            prev = now;
        }
    }
    black_box(&world);
    world.tick.0
}

fn main() {
    let reps: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(200)
        .max(1);
    let start = Instant::now();
    let mut ticks = 0u64;
    for _ in 0..reps {
        ticks += play_once();
    }
    let elapsed = start.elapsed();
    assert_eq!(ticks, u64::from(reps) * SLICE_TICKS);
    let ns = elapsed.as_nanos() / u128::from(ticks);
    println!("BENCH NATIVE TICKS {ticks} REPS {reps}");
    println!("BENCH NATIVE STEP avg {ns} ns per tick (wall clock, includes level build)");
}
