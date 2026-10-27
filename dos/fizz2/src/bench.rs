// The /BENCH mode: what the slice costs per tick on this machine, split into the step, the drawing and the copy to video memory.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository.
//
// The stopwatch is PIT channel 0 (`timer::clocks`): a 486 has no `rdtsc`. One clock is 0.838 us, so a single reading is fine
// enough for the sections measured here (tens of microseconds to milliseconds), and each reading costs a few microseconds of port
// I/O. That cost is measured first and taken off every section, so it does not inflate the small ones. The run is not paced:
// ticks follow each other as fast as the machine can go, which is what a budget needs. The 60 Hz interrupt still fires and shows
// up in the maxima (a few microseconds on real hardware).
use fizz_dos_rt::timer;
use fizz_ep2::{SLICE_SCRIPT, World};
use fizz_present_dos::{DIGEST_TICKS, Frame, frame_digest, present, render_slice};
use fizz_sim::{Buttons, InputFrame};

use crate::hal::DosDisplay;

/// PIT input clocks per second.
const PIT_HZ: u64 = 1_193_182;
/// Back-to-back readings used to measure what one reading costs.
const OVERHEAD_SAMPLES: u64 = 256;
/// Microseconds in one 60 Hz tick: the budget the three sections share.
pub const BUDGET_US: u64 = 16_667;

/// A monotonic reading of `timer::clocks`, repairing the one-period dip its documentation describes.
struct Stopwatch {
    last: u64,
}

impl Stopwatch {
    fn new() -> Self {
        Stopwatch { last: 0 }
    }

    fn now(&mut self) -> u64 {
        let mut t = timer::clocks();
        if t < self.last {
            t += u64::from(timer::PIT_DIVISOR_60HZ);
        }
        self.last = t;
        t
    }
}

/// The sum, count and largest of one section's samples, in PIT clocks.
#[derive(Clone, Copy, Default)]
pub struct Stat {
    sum: u64,
    max: u64,
    count: u64,
}

impl Stat {
    fn add(&mut self, clocks: u64) {
        self.sum += clocks;
        self.max = self.max.max(clocks);
        self.count += 1;
    }

    pub fn avg_us(&self) -> u64 {
        to_us(self.sum, self.count)
    }

    pub fn max_us(&self) -> u64 {
        to_us(self.max, 1)
    }
}

fn to_us(clocks: u64, n: u64) -> u64 {
    if n == 0 { 0 } else { clocks * 1_000_000 / (PIT_HZ * n) }
}

/// Everything `run` measured.
pub struct Report {
    pub ticks: u64,
    pub overhead_clocks: u64,
    pub step: Stat,
    pub draw: Stat,
    pub present: Stat,
    pub total: Stat,
    /// `frame_digest` of the picture at each of `DIGEST_TICKS` the run reached, as (tick, digest); `frame_count` are filled.
    pub frames: [(u64, u64); DIGEST_TICKS.len()],
    pub frame_count: usize,
}

/// Measures the cost of one reading: the average gap between readings taken one straight after the other.
fn overhead(sw: &mut Stopwatch) -> u64 {
    let first = sw.now();
    let mut last = first;
    for _ in 0..OVERHEAD_SAMPLES {
        last = sw.now();
    }
    (last - first) / OVERHEAD_SAMPLES
}

/// Plays the slice script once from a fresh level, timing each tick's step, drawing (the picture and the HUD text into the
/// buffer) and copy to the screen, stopping after `limit` ticks (the whole script when it is `SLICE_TICKS` or more). The caller has hooked the timer and set mode 13h. Returns the final world with the report.
pub fn run(display: &mut DosDisplay, limit: u64) -> (World, Report) {
    let mut sw = Stopwatch::new();
    let cost = overhead(&mut sw);
    let mut world = fizz_ep2::level::world();
    let mut frame = Frame::new();
    let (mut step, mut draw, mut copy, mut total) = (Stat::default(), Stat::default(), Stat::default(), Stat::default());
    let mut prev = Buttons::NONE;
    let mut digests = [(0, 0); DIGEST_TICKS.len()];
    let mut frame_count = 0;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            if step.count >= limit {
                break;
            }
            let t0 = sw.now();
            fizz_ep2::step(&mut world, InputFrame::from_transition(prev, now));
            let t1 = sw.now();
            render_slice(&world, &mut frame);
            let t2 = sw.now();
            let _ = present(display, &frame);
            let t3 = sw.now();
            prev = now;
            // Outside the timed sections: the digest only proves what was drawn.
            if DIGEST_TICKS.contains(&world.tick.0) {
                digests[frame_count] = (world.tick.0, frame_digest(&frame));
                frame_count += 1;
            }
            step.add((t1 - t0).saturating_sub(cost));
            draw.add((t2 - t1).saturating_sub(cost));
            copy.add((t3 - t2).saturating_sub(cost));
            total.add((t3 - t0).saturating_sub(3 * cost));
        }
    }
    (
        world,
        Report {
            ticks: step.count,
            overhead_clocks: cost,
            step,
            draw,
            present: copy,
            total,
            frames: digests,
            frame_count,
        },
    )
}
