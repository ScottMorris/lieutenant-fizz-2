// Web presenter: paces the rules from browser time and exposes them to JavaScript, as the Milestone 0 marker and as the slice.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The page pushes `performance.now()` into `App::frame`. A millisecond `WebClock` and the shared integer `Pacer` turn that into
// whole 60 Hz ticks, so the Web build paces exactly as the DOS build does from its PIT. Rules never see the browser clock.
use core::cell::Cell;
use fizz_hal::{Clock, Pacer, Ticks};
use fizz_hello::{World, hash, step};
use fizz_sim::{Buttons, InputFrame, InputLatch, TICK_HZ};
use fizz_slice_view::{VIEW_H, VIEW_W, draw_slice};
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

    /// The logical pixel column of the marker's left edge, from the rule every presenter shares.
    pub fn marker_px_x(&self) -> i32 {
        fizz_view::pixel(self.world.x)
    }

    /// The logical pixel row of the marker's top edge, from the rule every presenter shares.
    pub fn marker_px_y(&self) -> i32 {
        fizz_view::pixel(self.world.y)
    }

    pub fn dashes(&self) -> u32 {
        self.world.dashes
    }
}

/// The sixteen EGA colours as opaque RGBA, in standard order: the table `docs/SPRITES.md` lists, which the DOS presenter loads
/// as DAC values, so the canvas shows the colours the VGA screen shows.
const EGA_RGBA: [[u8; 4]; 16] = {
    const fn c(r: u8, g: u8, b: u8) -> [u8; 4] {
        [r, g, b, 0xFF]
    }
    [
        c(0x00, 0x00, 0x00),
        c(0x00, 0x00, 0xAA),
        c(0x00, 0xAA, 0x00),
        c(0x00, 0xAA, 0xAA),
        c(0xAA, 0x00, 0x00),
        c(0xAA, 0x00, 0xAA),
        c(0xAA, 0x55, 0x00),
        c(0xAA, 0xAA, 0xAA),
        c(0x55, 0x55, 0x55),
        c(0x55, 0x55, 0xFF),
        c(0x55, 0xFF, 0x55),
        c(0x55, 0xFF, 0xFF),
        c(0xFF, 0x55, 0x55),
        c(0xFF, 0x55, 0xFF),
        c(0xFF, 0xFF, 0x55),
        c(0xFF, 0xFF, 0xFF),
    ]
};

/// The slice as the page sees it: the same pacing and key latch as [`App`], over `fizz_ep2`'s world.
///
/// `frame` runs whole ticks. `render` draws the world into a 320 by 200 palette-indexed buffer with the routine the DOS
/// presenter also uses, then expands it to RGBA for the canvas. The page reads that buffer straight out of the module's memory
/// through [`SliceApp::rgba_ptr`].
#[wasm_bindgen]
pub struct SliceApp {
    world: fizz_ep2::World,
    latch: InputLatch,
    clock: WebClock,
    pacer: Option<Pacer>,
    held: Buttons,
    indexed: Vec<u8>,
    rgba: Vec<u8>,
}

impl Default for SliceApp {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl SliceApp {
    #[wasm_bindgen(constructor)]
    pub fn new() -> SliceApp {
        SliceApp {
            world: fizz_ep2::level::world(),
            latch: InputLatch::new(),
            clock: WebClock::new(),
            pacer: None,
            held: Buttons::NONE,
            indexed: vec![0; VIEW_W * VIEW_H],
            rgba: vec![0; VIEW_W * VIEW_H * 4],
        }
    }

    /// Records a key edge. `bits` are `fizz_sim::Buttons` bits; repeated downs are not new presses.
    pub fn key(&mut self, bits: u8, down: bool) {
        self.latch.set(Buttons(bits), down);
    }

    /// Advances the rules to browser time `now_ms` and returns how many ticks ran. The first call only starts the pacer.
    ///
    /// The first tick of a call takes the latch's edges, so a press that began and ended between two calls still counts once;
    /// the others see only the held buttons.
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
            fizz_ep2::step(&mut self.world, input);
        }
        due
    }

    /// Starts the level again from its first tick. The pacer keeps running, so no time is lost or repeated. Every held key is
    /// forgotten, so the page releases its own keys when it calls this.
    pub fn reset(&mut self) {
        self.world = fizz_ep2::level::world();
        self.latch = InputLatch::new();
        self.held = Buttons::NONE;
    }

    /// Draws the current world into the frame buffers.
    pub fn render(&mut self) {
        draw_slice(
            &mut self.indexed,
            VIEW_W,
            VIEW_H,
            &self.world,
            self.world.tick.0 as u32,
        );
        let (pixels, _) = self.rgba.as_chunks_mut::<4>();
        for (px, &i) in pixels.iter_mut().zip(&self.indexed) {
            *px = EGA_RGBA[usize::from(i & 15)];
        }
    }

    /// The width of the frame in pixels.
    pub fn width(&self) -> u32 {
        VIEW_W as u32
    }

    /// The height of the frame in pixels.
    pub fn height(&self) -> u32 {
        VIEW_H as u32
    }

    /// The address of the RGBA frame in the module's memory: `rgba_len` bytes, valid until the next call that changes the app.
    /// Call [`SliceApp::render`] first.
    pub fn rgba_ptr(&self) -> *const u8 {
        self.rgba.as_ptr()
    }

    /// The length of the RGBA frame in bytes.
    pub fn rgba_len(&self) -> usize {
        self.rgba.len()
    }

    pub fn tick(&self) -> f64 {
        self.world.tick.0 as f64
    }

    pub fn hash_hex(&self) -> String {
        format!("{:016x}", fizz_ep2::hash(&self.world).finish())
    }

    /// Fizz Blaster shots left.
    pub fn ammo(&self) -> u32 {
        self.world.ammo
    }

    /// Whether Ben is carrying the red key.
    pub fn has_key(&self) -> bool {
        self.world.flags.contains(fizz_ep2::WorldFlags::KEY_RED)
    }

    pub fn deaths(&self) -> u32 {
        self.world.deaths
    }

    /// Whether the red door has been opened. It stays open through a death.
    pub fn door_open(&self) -> bool {
        self.world
            .flags
            .contains(fizz_ep2::WorldFlags::DOOR_RED_OPEN)
    }

    /// Whether Ben has reached the exit.
    pub fn exit_reached(&self) -> bool {
        self.world
            .flags
            .contains(fizz_ep2::WorldFlags::EXIT_REACHED)
    }
}

/// The slice replay transcript, byte for byte what `fizz_ep2::replay` writes and `crates/fizz-ep2/slice-replay.txt` holds.
#[wasm_bindgen]
pub fn slice_replay_transcript() -> String {
    let mut out = String::new();
    fizz_ep2::replay(&mut out).expect("writing to a String cannot fail");
    out
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

    #[test]
    fn the_marker_sits_in_the_pixel_that_contains_its_position_after_every_tick() {
        let mut app = App::new();
        let (mut prev, mut now_ms) = (Buttons::NONE, 0.0);
        app.frame(now_ms);
        for held in per_tick() {
            set_keys(&mut app, prev, held);
            now_ms += FRAME_MS;
            app.frame(now_ms);
            let (px, py) = (f64::from(app.marker_px_x()), f64::from(app.marker_px_y()));
            let (x, y) = (app.marker_x(), app.marker_y());
            assert!(
                px <= x && x < px + 1.0,
                "tick {}: x {x} drawn at {px}",
                app.world.tick.0
            );
            assert!(
                py <= y && y < py + 1.0,
                "tick {}: y {y} drawn at {py}",
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

    /// `set_keys` for the slice's seven buttons.
    fn set_slice_keys(app: &mut SliceApp, prev: Buttons, now: Buttons) {
        for bit in [1u8, 2, 4, 8, 16, 32, 64] {
            let (was, is) = (prev.contains(Buttons(bit)), now.contains(Buttons(bit)));
            if was != is {
                app.key(bit, is);
            }
        }
    }

    #[test]
    fn the_slice_app_matches_the_rules_after_every_tick() {
        let mut app = SliceApp::new();
        let mut reference = fizz_ep2::level::world();
        let (mut prev, mut now_ms) = (Buttons::NONE, 0.0);
        app.frame(now_ms);
        for &(n, held) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..n {
                set_slice_keys(&mut app, prev, held);
                now_ms += FRAME_MS;
                assert_eq!(app.frame(now_ms), 1);
                fizz_ep2::step(&mut reference, InputFrame::from_transition(prev, held));
                assert_eq!(
                    app.hash_hex(),
                    format!("{:016x}", fizz_ep2::hash(&reference).finish())
                );
                prev = held;
            }
        }
        assert_eq!(app.tick(), fizz_ep2::SLICE_TICKS as f64);
        assert!(app.exit_reached());
        assert!(app.door_open());
        assert!(!app.has_key(), "the door spent the key");
        assert_eq!(app.deaths(), 1);
    }

    #[test]
    fn the_slice_app_ends_on_the_golden_final_hash() {
        let mut app = SliceApp::new();
        let (mut prev, mut now_ms) = (Buttons::NONE, 0.0);
        app.frame(now_ms);
        for &(n, held) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..n {
                set_slice_keys(&mut app, prev, held);
                now_ms += FRAME_MS;
                app.frame(now_ms);
                prev = held;
            }
        }
        let golden = include_str!("../../fizz-ep2/slice-replay.txt");
        let last = golden.lines().last().unwrap();
        assert_eq!(
            last,
            format!("FINAL TICK {} HASH {}", app.tick(), app.hash_hex())
        );
    }

    #[test]
    fn the_slice_transcript_equals_the_golden_file() {
        assert_eq!(
            slice_replay_transcript(),
            include_str!("../../fizz-ep2/slice-replay.txt")
        );
    }

    #[test]
    fn a_slow_slice_frame_runs_several_ticks_with_held_input_only() {
        let mut app = SliceApp::new();
        app.frame(0.0);
        app.key(8, true);
        assert_eq!(app.frame(50.0), 3);
        let mut reference = fizz_ep2::level::world();
        fizz_ep2::step(
            &mut reference,
            InputFrame::from_transition(Buttons::NONE, Buttons::RIGHT),
        );
        let held = InputFrame {
            held: Buttons::RIGHT,
            pressed: Buttons::NONE,
        };
        fizz_ep2::step(&mut reference, held);
        fizz_ep2::step(&mut reference, held);
        assert_eq!(
            app.hash_hex(),
            format!("{:016x}", fizz_ep2::hash(&reference).finish())
        );
    }

    #[test]
    fn a_press_released_between_frames_still_counts_once() {
        let mut app = SliceApp::new();
        app.frame(0.0);
        // Pressed and released before the next frame: the latch holds the press for one tick.
        app.key(16, true);
        app.key(16, false);
        assert_eq!(app.frame(17.0), 1);
        let mut reference = fizz_ep2::level::world();
        fizz_ep2::step(
            &mut reference,
            InputFrame {
                held: Buttons::NONE,
                pressed: Buttons::JUMP,
            },
        );
        assert_eq!(
            app.hash_hex(),
            format!("{:016x}", fizz_ep2::hash(&reference).finish())
        );
    }

    /// A fresh level stepped with no input for `n` ticks.
    fn level_world_after_idle_ticks(n: u32) -> fizz_ep2::World {
        let mut w = fizz_ep2::level::world();
        for _ in 0..n {
            fizz_ep2::step(
                &mut w,
                InputFrame::from_transition(Buttons::NONE, Buttons::NONE),
            );
        }
        w
    }

    #[test]
    fn reset_returns_to_the_first_tick_and_forgets_held_keys() {
        let mut app = SliceApp::new();
        let start = app.hash_hex();
        app.frame(0.0);
        app.key(8, true);
        app.frame(100.0);
        assert_ne!(app.hash_hex(), start);
        app.reset();
        assert_eq!(app.hash_hex(), start);
        assert_eq!(app.tick(), 0.0);
        // The held Right was forgotten: Ben stands still while time passes.
        let idle = level_world_after_idle_ticks(app.frame(200.0));
        assert_eq!(
            app.hash_hex(),
            format!("{:016x}", fizz_ep2::hash(&idle).finish())
        );
        assert_eq!(app.ammo(), fizz_ep2::world::START_AMMO);
        assert!(!app.has_key() && !app.door_open() && !app.exit_reached());
        assert_eq!(app.deaths(), 0);
    }

    #[test]
    fn the_rgba_frame_is_opaque_and_the_size_of_the_view() {
        let mut app = SliceApp::new();
        app.render();
        assert_eq!((app.width(), app.height()), (320, 200));
        assert_eq!(app.rgba_len(), 320 * 200 * 4);
        let (pixels, rest) = app.rgba.as_chunks::<4>();
        assert!(rest.is_empty());
        // Every pixel is one of the sixteen EGA colours, which are all opaque.
        assert!(pixels.iter().all(|p| EGA_RGBA.contains(p)));
        // The slice draws a whole picture, not a blank frame.
        assert!(pixels.iter().any(|p| p != &EGA_RGBA[0]));
    }

    #[test]
    fn the_palette_matches_the_sprite_document() {
        let hex = [
            "000000", "0000aa", "00aa00", "00aaaa", "aa0000", "aa00aa", "aa5500", "aaaaaa",
            "555555", "5555ff", "55ff55", "55ffff", "ff5555", "ff55ff", "ffff55", "ffffff",
        ];
        for (c, h) in EGA_RGBA.iter().zip(hex) {
            assert_eq!(format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2]), h);
        }
    }
}
