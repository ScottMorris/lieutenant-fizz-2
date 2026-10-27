// Web presenter for Milestone 0: paces the throwaway rules from browser time and exposes them to JavaScript.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The page pushes `performance.now()` into `App::frame`. A millisecond `WebClock` and the shared integer `Pacer` turn that into
// whole 60 Hz ticks, so the Web build paces exactly as the DOS build does from its PIT. Rules never see the browser clock.
use core::cell::Cell;
use fizz_hal::{Clock, Pacer, Ticks};
use fizz_hello::{World, hash, step};
use fizz_present_dos::marker_origin;
use fizz_sim::{Buttons, InputFrame, InputLatch, TICK_HZ};
use wasm_bindgen::prelude::*;

/// Most ticks run for one displayed frame. Time beyond this is dropped, so a hidden tab does not cause a burst on return.
const MAX_CATCH_UP: u32 = 5;

/// A clock of whole milliseconds fed by the time JavaScript pushes in.
#[derive(Debug, Default)]
pub struct WebClock {
    ms: Cell<u32>,
}

impl WebClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the reading from a millisecond timestamp. The fraction is rounded up so that a 60 Hz display, whose frames
    /// arrive 16.67 ms apart, never loses a tick to truncation. The counter wraps like the HAL's other clocks.
    pub fn set_ms(&self, now_ms: f64) {
        let whole = if now_ms.is_finite() && now_ms > 0.0 {
            now_ms.ceil() as u64
        } else {
            0
        };
        self.ms.set(whole as u32);
    }
}

impl Clock for WebClock {
    fn now(&self) -> Ticks {
        Ticks(self.ms.get())
    }

    /// One tick per millisecond is 1000 Hz, which is 1,000,000 millihertz.
    fn millihertz(&self) -> u32 {
        1_000_000
    }
}

/// The running game as the page sees it.
#[wasm_bindgen]
pub struct App {
    world: World,
    latch: InputLatch,
    clock: WebClock,
    pacer: Option<Pacer>,
    held: Buttons,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl App {
    #[wasm_bindgen(constructor)]
    pub fn new() -> App {
        App {
            world: World::new(),
            latch: InputLatch::new(),
            clock: WebClock::new(),
            pacer: None,
            held: Buttons::NONE,
        }
    }

    /// Records a key edge. `bits` are `fizz_sim::Buttons` bits; repeated downs are not new presses.
    pub fn key(&mut self, bits: u8, down: bool) {
        self.latch.set(Buttons(bits), down);
    }

    /// Advances the rules to browser time `now_ms` and returns how many ticks ran. The first call only starts the pacer.
    pub fn frame(&mut self, now_ms: f64) -> u32 {
        self.clock.set_ms(now_ms);
        let Some(pacer) = self.pacer.as_mut() else {
            self.pacer = Some(Pacer::new(&self.clock, TICK_HZ, MAX_CATCH_UP));
            return 0;
        };
        let due = pacer.due(&self.clock);
        for i in 0..due {
            let input = if i == 0 {
                let f = self.latch.frame();
                self.held = f.held;
                f
            } else {
                InputFrame {
                    held: self.held,
                    pressed: Buttons::NONE,
                }
            };
            step(&mut self.world, input);
        }
        due
    }

    pub fn tick(&self) -> f64 {
        self.world.tick.0 as f64
    }

    pub fn hash_hex(&self) -> String {
        format!("{:016x}", hash(&self.world).finish())
    }

    pub fn marker_x(&self) -> f64 {
        self.world.x
    }

    pub fn marker_y(&self) -> f64 {
        self.world.y
    }

    /// The screen column of the marker's left edge, from the rule the DOS presenter uses.
    pub fn marker_px_x(&self) -> i32 {
        marker_origin(&self.world).0
    }

    /// The screen row of the marker's top edge, from the rule the DOS presenter uses.
    pub fn marker_px_y(&self) -> i32 {
        marker_origin(&self.world).1
    }

    pub fn dashes(&self) -> u32 {
        self.world.dashes
    }
}

/// The Milestone 0 replay transcript, byte for byte what `fizz_hello::replay` writes.
#[wasm_bindgen]
pub fn replay_transcript() -> String {
    let mut out = String::new();
    fizz_hello::replay(&mut out).expect("writing to a String cannot fail");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use fizz_hello::M0_SCRIPT;

    const FRAME_MS: f64 = 1000.0 / 60.0;

    /// Expands the script to the held buttons of each tick.
    fn per_tick() -> Vec<Buttons> {
        M0_SCRIPT
            .iter()
            .flat_map(|&(n, b)| core::iter::repeat_n(b, usize::from(n)))
            .collect()
    }

    fn set_keys(app: &mut App, prev: Buttons, now: Buttons) {
        for bit in [1u8, 2, 4, 8, 16] {
            let (was, is) = (prev.contains(Buttons(bit)), now.contains(Buttons(bit)));
            if was != is {
                app.key(bit, is);
            }
        }
    }

    fn hex(w: &World) -> String {
        format!("{:016x}", hash(w).finish())
    }

    #[test]
    fn one_tick_per_frame_matches_the_rules_after_every_tick() {
        let mut app = App::new();
        let mut reference = World::new();
        let (mut prev, mut now_ms) = (Buttons::NONE, 0.0);
        assert_eq!(app.frame(now_ms), 0);
        for held in per_tick() {
            set_keys(&mut app, prev, held);
            now_ms += FRAME_MS;
            assert_eq!(app.frame(now_ms), 1);
            step(&mut reference, InputFrame::from_transition(prev, held));
            assert_eq!(app.hash_hex(), hex(&reference));
            prev = held;
        }
        assert_eq!(app.tick(), 600.0);
        assert_eq!(app.dashes(), reference.dashes);
        assert_eq!(app.marker_x(), reference.x);
        assert_eq!(app.marker_y(), reference.y);
    }

    /// The marker's top-left corner in a rendered DOS frame: the first yellow pixel in row-major order.
    fn dos_marker_corner(world: &World, frame: &mut fizz_present_dos::Frame) -> (i32, i32) {
        fizz_present_dos::render(world, frame);
        let at = frame
            .pixels()
            .iter()
            .position(|&p| p == fizz_present_dos::YELLOW)
            .expect("the marker is on screen");
        (
            (at % fizz_present_dos::WIDTH) as i32,
            (at / fizz_present_dos::WIDTH) as i32,
        )
    }

    #[test]
    fn the_marker_pixel_matches_the_dos_frame_after_every_tick() {
        let mut app = App::new();
        let mut frame = fizz_present_dos::Frame::new();
        let (mut prev, mut now_ms) = (Buttons::NONE, 0.0);
        app.frame(now_ms);
        for held in per_tick() {
            set_keys(&mut app, prev, held);
            now_ms += FRAME_MS;
            app.frame(now_ms);
            assert_eq!(
                (app.marker_px_x(), app.marker_px_y()),
                dos_marker_corner(&app.world, &mut frame),
                "tick {}",
                app.world.tick.0
            );
            prev = held;
        }
    }

    #[test]
    fn the_final_hash_equals_the_replay_transcript() {
        let mut app = App::new();
        let (mut prev, mut now_ms) = (Buttons::NONE, 0.0);
        app.frame(now_ms);
        for held in per_tick() {
            set_keys(&mut app, prev, held);
            now_ms += FRAME_MS;
            app.frame(now_ms);
            prev = held;
        }
        let transcript = replay_transcript();
        let last = transcript.lines().last().unwrap();
        assert_eq!(last, format!("FINAL TICK 600 HASH {}", app.hash_hex()));
    }

    #[test]
    fn a_slow_frame_runs_several_ticks_with_held_input_only() {
        let mut app = App::new();
        app.frame(0.0);
        app.key(8, true);
        // 50 ms is three ticks: the first sees the press, the others the held state.
        assert_eq!(app.frame(50.0), 3);
        let mut reference = World::new();
        step(
            &mut reference,
            InputFrame::from_transition(Buttons::NONE, Buttons::RIGHT),
        );
        let held = InputFrame {
            held: Buttons::RIGHT,
            pressed: Buttons::NONE,
        };
        step(&mut reference, held);
        step(&mut reference, held);
        assert_eq!(app.hash_hex(), hex(&reference));
    }

    #[test]
    fn a_long_stall_is_capped() {
        let mut app = App::new();
        app.frame(0.0);
        assert_eq!(app.frame(60_000.0), MAX_CATCH_UP);
        assert_eq!(app.frame(60_000.0), 0);
    }

    #[test]
    fn a_millisecond_clock_gives_sixty_ticks_per_second() {
        let clock = WebClock::new();
        assert_eq!(clock.millihertz(), 1_000_000);
        let mut pacer = Pacer::new(&clock, TICK_HZ, MAX_CATCH_UP);
        let mut total = 0;
        for ms in 1..=1000 {
            clock.set_ms(f64::from(ms));
            total += pacer.due(&clock);
        }
        assert_eq!(total, 60);
    }

    #[test]
    fn the_clock_ignores_nonsense_timestamps() {
        let clock = WebClock::new();
        clock.set_ms(f64::NAN);
        assert_eq!(clock.now(), Ticks(0));
        clock.set_ms(-5.0);
        assert_eq!(clock.now(), Ticks(0));
        clock.set_ms(10.2);
        assert_eq!(clock.now(), Ticks(11));
    }

    #[test]
    fn the_transcript_equals_the_golden_file() {
        assert_eq!(
            replay_transcript(),
            include_str!("../../fizz-hello/m0-replay.txt")
        );
    }
}
