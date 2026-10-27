// Implements the throwaway Milestone 0 rules: a marker moved and dashed by input, with a state hash.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![forbid(unsafe_code)]

use core::f64::consts::FRAC_1_SQRT_2;
use fizz_sim::{Buttons, InputFrame, StateHash, Tick};

/// Everything the Milestone 0 rules remember between ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct World {
    pub tick: Tick,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub heading: f64,
    pub dashes: u32,
}

impl World {
    pub const fn new() -> Self {
        Self {
            tick: Tick::ZERO,
            x: 160.0,
            y: 100.0,
            vx: 0.0,
            vy: 0.0,
            heading: 0.0,
            dashes: 0,
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

const MAX_X: f64 = 312.0;
const MAX_Y: f64 = 192.0;
const ACCEL: f64 = 0.25;
const DRAG: f64 = 0.875;
const DASH_SPEED: f64 = 3.0;
/// Velocities smaller than this snap to zero, which keeps subnormals out of the simulation.
/// Subnormals round differently on x87 (53-bit), SSE2 and WebAssembly.
const SNAP: f64 = 1.0 / 1024.0;

/// Advances the world by one tick. The order of operations is part of the determinism contract.
pub fn step(w: &mut World, f: InputFrame) {
    let b = f.held | f.pressed;
    let dx = axis(b, Buttons::RIGHT, Buttons::LEFT);
    let dy = axis(b, Buttons::DOWN, Buttons::UP);

    if dx != 0.0 || dy != 0.0 {
        w.heading = libm::atan2(dy, dx);
    }

    if f.pressed.contains(Buttons::ACTION) {
        w.vx += DASH_SPEED * libm::cos(w.heading);
        w.vy += DASH_SPEED * libm::sin(w.heading);
        w.dashes += 1;
    }

    let diagonal = dx != 0.0 && dy != 0.0;
    let mut ax = dx * ACCEL;
    let mut ay = dy * ACCEL;
    if diagonal {
        ax *= FRAC_1_SQRT_2;
        ay *= FRAC_1_SQRT_2;
    }

    w.vx = (w.vx + ax) * DRAG;
    w.vy = (w.vy + ay) * DRAG;

    if w.vx.abs() < SNAP {
        w.vx = 0.0;
    }
    if w.vy.abs() < SNAP {
        w.vy = 0.0;
    }

    w.x += w.vx;
    w.y += w.vy;

    if w.x < 0.0 {
        w.x = 0.0;
        w.vx = 0.0;
    } else if w.x > MAX_X {
        w.x = MAX_X;
        w.vx = 0.0;
    }
    if w.y < 0.0 {
        w.y = 0.0;
        w.vy = 0.0;
    } else if w.y > MAX_Y {
        w.y = MAX_Y;
        w.vy = 0.0;
    }

    w.tick = w.tick.next();
}

/// Hashes the world in a fixed field order: tick, x, y, vx, vy, heading, dashes.
pub fn hash(w: &World) -> StateHash {
    let mut h = StateHash::new();
    h.write_u64(w.tick.0);
    h.write_f64(w.x);
    h.write_f64(w.y);
    h.write_f64(w.vx);
    h.write_f64(w.vy);
    h.write_f64(w.heading);
    h.write_u32(w.dashes);
    h
}

const IDLE: Buttons = Buttons::NONE;
const R: Buttons = Buttons::RIGHT;
const L: Buttons = Buttons::LEFT;
const U: Buttons = Buttons::UP;
const D: Buttons = Buttons::DOWN;
const A: Buttons = Buttons::ACTION;

/// The Milestone 0 input script as `(frames, held buttons)` runs. It sums to 600 ticks.
///
/// An ACTION held for exactly one frame is a press edge, so each one-frame run containing
/// ACTION is a dash. The script covers idle, a right run, a diagonal, dashes while moving and
/// idle, a left run into the left clamp, an up run, and a long idle tail so velocity decays
/// to the snap-to-zero.
pub const M0_SCRIPT: &[(u16, Buttons)] = &[
    (30, IDLE),
    (60, R),
    (1, R.union(A)),
    (29, R),
    (60, R.union(D)),
    (1, R.union(D).union(A)),
    (19, R.union(D)),
    (20, IDLE),
    (1, A),
    (29, IDLE),
    (1, A),
    (1, IDLE),
    (1, A),
    (17, IDLE),
    (200, L),
    (60, U),
    (70, IDLE),
];

/// Number of ticks the transcript covers.
pub const M0_TICKS: u64 = 600;

/// Writes transcript v1 of the M0 script: a hash at tick 0, every 60th tick, and the final tick.
pub fn replay(out: &mut impl core::fmt::Write) -> core::fmt::Result {
    let mut w = World::new();
    let mut prev = Buttons::NONE;
    writeln!(out, "M0 REPLAY 1")?;
    writeln!(out, "TICK 0 HASH {:016x}", hash(&w).finish())?;
    for &(frames, now) in M0_SCRIPT {
        for _ in 0..frames {
            step(&mut w, InputFrame::from_transition(prev, now));
            prev = now;
            if w.tick.0.is_multiple_of(60) {
                writeln!(out, "TICK {} HASH {:016x}", w.tick.0, hash(&w).finish())?;
            }
        }
    }
    writeln!(
        out,
        "FINAL TICK {} HASH {:016x}",
        w.tick.0,
        hash(&w).finish()
    )
}

fn axis(b: Buttons, positive: Buttons, negative: Buttons) -> f64 {
    let p = if b.contains(positive) { 1.0 } else { 0.0 };
    let n = if b.contains(negative) { 1.0 } else { 0.0 };
    p - n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(b: Buttons) -> InputFrame {
        InputFrame {
            held: b,
            pressed: Buttons::NONE,
        }
    }

    #[test]
    fn a_new_world_starts_at_rest_in_the_middle() {
        let w = World::new();
        assert_eq!(
            (w.x, w.y, w.vx, w.vy, w.heading),
            (160.0, 100.0, 0.0, 0.0, 0.0)
        );
        assert_eq!((w.tick, w.dashes), (Tick::ZERO, 0));
    }

    #[test]
    fn the_first_three_ticks_moving_right_are_exact() {
        let mut w = World::new();
        step(&mut w, held(Buttons::RIGHT));
        assert_eq!((w.vx, w.x, w.tick), (0.21875, 160.21875, Tick(1)));
        step(&mut w, held(Buttons::RIGHT));
        assert_eq!((w.vx, w.x), (0.41015625, 160.62890625));
        step(&mut w, held(Buttons::RIGHT));
        assert_eq!((w.vx, w.x), (0.57763671875, 161.20654296875));
        assert_eq!((w.vy, w.y, w.heading), (0.0, 100.0, 0.0));
    }

    #[test]
    fn a_dash_adds_a_burst_along_the_heading() {
        let mut w = World::new();
        let tap = InputFrame {
            held: Buttons::RIGHT | Buttons::ACTION,
            pressed: Buttons::ACTION,
        };
        step(&mut w, tap);
        // (3.0 + 0.25) * 0.875
        assert_eq!((w.vx, w.x, w.dashes), (2.84375, 162.84375, 1));
    }

    #[test]
    fn a_dash_at_rest_uses_the_last_heading() {
        let mut w = World::new();
        w.heading = core::f64::consts::PI;
        let tap = InputFrame {
            held: Buttons::NONE,
            pressed: Buttons::ACTION,
        };
        step(&mut w, tap);
        assert!(w.vx < 0.0);
        assert_eq!(w.dashes, 1);
    }

    #[test]
    fn a_pressed_direction_counts_even_if_not_held() {
        let mut w = World::new();
        let f = InputFrame {
            held: Buttons::NONE,
            pressed: Buttons::DOWN,
        };
        step(&mut w, f);
        assert!(w.vy > 0.0);
    }

    #[test]
    fn diagonal_acceleration_is_scaled() {
        let mut w = World::new();
        step(&mut w, held(Buttons::RIGHT | Buttons::DOWN));
        let expected = (0.25 * FRAC_1_SQRT_2) * 0.875;
        assert_eq!(w.vx, expected);
        assert_eq!(w.vy, expected);
        assert_eq!(w.heading, libm::atan2(1.0, 1.0));
    }

    #[test]
    fn clamping_zeroes_velocity_on_that_axis_only() {
        let mut w = World::new();
        w.x = 0.5;
        w.vx = -3.0;
        w.vy = 1.0;
        step(&mut w, held(Buttons::LEFT));
        assert_eq!((w.x, w.vx), (0.0, 0.0));
        assert!(w.vy > 0.0);
        w.y = 191.9;
        w.vy = 2.0;
        step(&mut w, held(Buttons::NONE));
        assert_eq!((w.y, w.vy), (192.0, 0.0));
    }

    #[test]
    fn velocity_never_goes_subnormal() {
        let mut w = World::new();
        for _ in 0..10_000 {
            step(&mut w, held(Buttons::RIGHT | Buttons::UP));
            assert!(ok(w.vx) && ok(w.vy));
        }
        for _ in 0..10_000 {
            step(&mut w, held(Buttons::NONE));
            assert!(ok(w.vx) && ok(w.vy));
        }
        assert_eq!((w.vx, w.vy), (0.0, 0.0));
    }

    #[test]
    fn the_initial_hash_matches_an_independent_fnv1a_computation() {
        // Computed outside Rust: FNV-1a 64 over tick as 8 LE bytes, five doubles as 8 LE bytes
        // each (160.0, 100.0, 0.0, 0.0, 0.0), then dashes as 4 LE bytes.
        assert_eq!(hash(&World::new()).finish(), 0x0231_7fd0_a71a_d280);
    }

    #[test]
    fn hash_field_order_is_tick_x_y_vx_vy_heading_dashes() {
        let w = World {
            tick: Tick(7),
            x: 1.5,
            y: 2.5,
            vx: 0.25,
            vy: -0.5,
            heading: 0.75,
            dashes: 3,
        };
        assert_eq!(hash(&w).finish(), 0x26bd_4838_1388_449d);

        // Swapping any two doubles changes the digest.
        let swapped_xy = World {
            x: w.y,
            y: w.x,
            ..w
        };
        let swapped_v = World {
            vx: w.vy,
            vy: w.vx,
            ..w
        };
        assert_ne!(hash(&swapped_xy).finish(), hash(&w).finish());
        assert_ne!(hash(&swapped_v).finish(), hash(&w).finish());
    }

    extern crate std;
    use std::string::String;
    use std::vec::Vec;

    fn transcript() -> String {
        let mut s = String::new();
        replay(&mut s).unwrap();
        s
    }

    #[test]
    fn the_script_covers_exactly_600_ticks() {
        let total: u64 = M0_SCRIPT.iter().map(|&(n, _)| u64::from(n)).sum();
        assert_eq!(total, M0_TICKS);
    }

    #[test]
    fn the_transcript_has_the_v1_shape() {
        let t = transcript();
        assert!(t.ends_with('\n'));
        let lines: Vec<&str> = t.lines().collect();
        // Header, tick 0, ticks 60..=600 (ten lines), final.
        assert_eq!(lines.len(), 13);
        assert_eq!(lines[0], "M0 REPLAY 1");
        assert!(lines[1].starts_with("TICK 0 HASH "));
        assert!(lines[11].starts_with("TICK 600 HASH "));
        let tail = |l: &str| String::from(l.rsplit(' ').next().unwrap());
        assert_eq!(
            lines[12].strip_prefix("FINAL TICK 600 HASH "),
            Some(tail(lines[11]).as_str())
        );
        assert_eq!(tail(lines[11]).len(), 16);
    }

    #[test]
    fn the_transcript_matches_the_golden_file() {
        assert_eq!(transcript(), include_str!("../m0-replay.txt"));
    }

    #[test]
    fn the_script_exercises_dashes_and_both_clamps() {
        let mut w = World::new();
        let mut prev = Buttons::NONE;
        let mut min_x = w.x;
        let mut max_x = w.x;
        for &(frames, now) in M0_SCRIPT {
            for _ in 0..frames {
                step(&mut w, InputFrame::from_transition(prev, now));
                prev = now;
                min_x = min_x.min(w.x);
                max_x = max_x.max(w.x);
            }
        }
        assert_eq!(w.dashes, 5);
        assert_eq!(min_x, 0.0);
        assert!(max_x > 160.0);
        assert_eq!((w.vx, w.vy), (0.0, 0.0));
    }

    fn ok(v: f64) -> bool {
        v == 0.0 || v.is_normal()
    }
}
