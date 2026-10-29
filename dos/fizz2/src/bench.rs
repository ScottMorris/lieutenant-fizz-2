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
// up in the maxima (a few microseconds on real hardware). With `/AUDIO` the 560 Hz service runs instead (see `main`), and the stopwatch
// counts periods of the timer as it is, not of the 60 Hz one.
use fizz_dos_rt::timer;
use fizz_ep2::{SLICE_SCRIPT, World};
use fizz_present_dos::{DIGEST_TICKS, DrawOptions, Frame, VIEW_H, VIEW_W, ViewFx, frame_digest, present, render_slice_with};
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
            // One period of the timer as it is now: 60 Hz for /BENCH, 560 Hz when the audio service runs (/BENCH /AUDIO).
            t += u64::from(timer::divisor());
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

/// Loops per calibration chunk. A chunk is two instructions per loop, so it stays far shorter than one 60 Hz period (the
/// stopwatch's range) at any cycle setting the check uses.
const CALIBRATE_CHUNK: u32 = 2_000;
/// Chunks timed one after the other, each with its own reading cost taken off.
const CALIBRATE_CHUNKS: u32 = 100;
/// Instructions in one calibration loop: a `dec` and a `jnz`.
pub const CALIBRATE_INSTRUCTIONS_PER_LOOP: u64 = 2;

/// Times a loop of a known number of simple instructions with the same stopwatch, so the emulator's speed (instructions per
/// millisecond) can be read off and compared with the cycles per millisecond it was asked to run at. Returns the loops run and
/// the microseconds they took, reading cost taken off.
pub fn calibrate() -> (u64, u64) {
    let mut sw = Stopwatch::new();
    let cost = overhead(&mut sw);
    let mut clocks = 0u64;
    for _ in 0..CALIBRATE_CHUNKS {
        let t0 = sw.now();
        spin(CALIBRATE_CHUNK);
        let t1 = sw.now();
        clocks += (t1 - t0).saturating_sub(cost);
    }
    (u64::from(CALIBRATE_CHUNK) * u64::from(CALIBRATE_CHUNKS), to_us(clocks, 1))
}

/// Runs `n` iterations of `dec`/`jnz`: two register-only instructions each, no memory access.
#[inline(never)]
fn spin(n: u32) {
    // SAFETY: only a register is read and written; no memory, stack or flags state outlives the block.
    unsafe {
        core::arch::asm!("2:", "dec {n:e}", "jnz 2b", n = inout(reg) n => _, options(nomem, nostack));
    }
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

/// The tick the caption measurement stops the script at: Ben is in the snack nook, with a jump, a snack and a head-bump about to
/// fire, so the picture is a busy one (the sprites, the HUD and the puffs are all in view).
pub const CAPTION_BENCH_TICK: u64 = 1190;
/// Pictures timed for each of the two cases.
pub const CAPTION_BENCH_FRAMES: u64 = 120;

/// What `captions` measured: the drawing of one busy picture with no captions on show and with all six slots full.
pub struct CaptionsReport {
    pub tick: u64,
    pub frames: u64,
    /// `render_slice_with` alone, then with the copy to the screen, with no captions.
    pub none_draw: Stat,
    pub none_total: Stat,
    /// The same with the six widest words on show (`EventCaptions::show_six`).
    pub six_draw: Stat,
    pub six_total: Stat,
}

/// Times the drawing of the picture at `CAPTION_BENCH_TICK` with no captions, then with six on show, `CAPTION_BENCH_FRAMES` pictures
/// each, always drawing captions whatever the Captions setting says. The world does not move between pictures, so the only thing that
/// differs between the two cases is the six words. With `audio` an effect is requested every few pictures, cycling through the longest
/// ones (ta-da!, boing, whoa!, crunch, key), so the audio service has the chip busy while the pictures are drawn; the caller reads the
/// service's profile for what that cost.
pub fn captions(display: &mut DosDisplay, options: &DrawOptions, audio: bool) -> CaptionsReport {
    let mut sw = Stopwatch::new();
    let cost = overhead(&mut sw);
    let options = DrawOptions {
        captions: true,
        ..*options
    };
    let mut world = fizz_ep2::level::world();
    let mut prev = Buttons::NONE;
    'script: for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            if world.tick.0 >= CAPTION_BENCH_TICK {
                break 'script;
            }
            fizz_ep2::step(&mut world, InputFrame::from_transition(prev, now));
            prev = now;
        }
    }
    let mut frame = Frame::new();
    let mut run = |fx: &ViewFx| -> (Stat, Stat) {
        let (mut draw, mut total) = (Stat::default(), Stat::default());
        for i in 0..CAPTION_BENCH_FRAMES {
            if audio && i % 6 == 0 {
                const CYCLE: [u8; 5] = [8, 12, 7, 14, 9];
                fizz_dos_rt::audio::effect(CYCLE[(i / 6) as usize % CYCLE.len()], &fizz_present_dos::Placement::CENTRE);
            }
            let t0 = sw.now();
            render_slice_with(&world, &mut frame, &options, fx);
            let t1 = sw.now();
            let _ = present(display, &frame);
            let t2 = sw.now();
            draw.add((t1 - t0).saturating_sub(cost));
            total.add((t2 - t0).saturating_sub(2 * cost));
        }
        (draw, total)
    };
    let (none_draw, none_total) = run(&ViewFx::new());
    let mut fx = ViewFx::new();
    fx.captions.show_six(&world, VIEW_W, VIEW_H, &options);
    let (six_draw, six_total) = run(&fx);
    CaptionsReport {
        tick: world.tick.0,
        frames: CAPTION_BENCH_FRAMES,
        none_draw,
        none_total,
        six_draw,
        six_total,
    }
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
///
/// With `audio`, each tick's clucks go to the audio service afterwards (outside the timed sections), as `/REPLAY` does.
pub fn run(display: &mut DosDisplay, limit: u64, options: &DrawOptions, audio: bool) -> (World, Report) {
    let mut sw = Stopwatch::new();
    let cost = overhead(&mut sw);
    let mut world = fizz_ep2::level::world();
    let mut frame = Frame::new();
    let (mut step, mut draw, mut copy, mut total) = (Stat::default(), Stat::default(), Stat::default(), Stat::default());
    let mut puffs = ViewFx::new();
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
            puffs.observe(&world, VIEW_W, VIEW_H, options);
            let t1 = sw.now();
            render_slice_with(&world, &mut frame, options, &puffs);
            let t2 = sw.now();
            let _ = present(display, &frame);
            let t3 = sw.now();
            prev = now;
            if audio {
                crate::play_events(&world, options);
            }
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
