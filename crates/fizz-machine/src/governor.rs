// The frame-rate governor: how often the DOS build draws, and (in Auto) how it picks 30 Hz or 60 Hz from what the frames cost.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: the policy is adapted from Fizz 1's DOS port (fizz-dos, `m4/c/main.c`, read only): at 30 Hz the screen is drawn on every
// second tick, and Auto averages the cost of a drawn frame over a window of 32 frames on the 560 Hz clock, as a percentage of a 60 Hz
// frame (`spent * 60 * 100 / (32 * 560)`). From 101% to 170% it chooses 30 Hz, where a steady half rate beats a ragged one; above 185%
// (dearer than that a cap would only throw frames away) or below 67% it chooses 60 Hz; in between it keeps the rate it has. The
// numbers are Fizz 1's; Fizz 2 has no detail setting (it has no backdrop layers), so only the rate half is taken.
// The code is written for this repository.
//
// The governor only decides when to draw. The simulation always runs at 60 Hz (the integer `Pacer`), so nothing it produces depends on
// the rate: the transcript, the audio and the input are the same at either.
use crate::settings::FrameRate;

/// Frames in one measuring window.
pub const WINDOW: u32 = 32;
/// The audio service's clock: a sixtieth of a second is 9.33 of its ticks.
pub const CLOCK_HZ: u32 = 560;
/// Above this percentage of a 60 Hz frame (and up to `CHOOSE_30_UP_TO`) Auto chooses 30 Hz.
pub const CHOOSE_30_ABOVE: u32 = 100;
/// The top of the band that chooses 30 Hz.
pub const CHOOSE_30_UP_TO: u32 = 170;
/// Above this percentage Auto chooses 60 Hz again: a cap would only throw frames away.
pub const CHOOSE_60_ABOVE: u32 = 185;
/// Below this percentage Auto chooses 60 Hz.
pub const CHOOSE_60_BELOW: u32 = 67;
/// With the sky on Auto (and the frame rate on Auto), a window of Light frames averaging below this percentage of a 60 Hz frame
/// chooses the Full sky. Full's worst frame costs about one and a half times Light's average frame (16.1 ms against 10.5 ms of
/// drawing and copy on the 486DX2-66 floor), so a machine that draws Light this cheaply draws Full's worst frame in about 70% of a
/// frame. The floor's Light windows are about 63%, so there Auto stays Light.
pub const SKY_FULL_BELOW: u32 = 45;
/// A window of Full frames averaging above this percentage goes back to the Light sky, before the frames come near a step.
pub const SKY_LIGHT_ABOVE: u32 = 75;

/// Draws at 60 Hz, at 30 Hz or (Auto) at whichever the frames' cost calls for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameGovernor {
    mode: FrameRate,
    hz: u32,
    /// Whether the sky is on Auto, which this governor then decides, and whether it chose Full.
    auto_sky: bool,
    full_sky: bool,
    /// The tick of the last frame drawn.
    drawn: u64,
    spent: u64,
    frames: u32,
}

impl FrameGovernor {
    /// Auto starts at 60 Hz; the fixed modes are their rate for good.
    pub const fn new(mode: FrameRate) -> Self {
        FrameGovernor {
            mode,
            hz: if matches!(mode, FrameRate::Hz30) {
                30
            } else {
                60
            },
            auto_sky: false,
            full_sky: false,
            drawn: 0,
            spent: 0,
            frames: 0,
        }
    }

    /// Lets this governor decide the sky (the Sky setting on Auto), starting from Light; `false` leaves the sky to the setting.
    /// Fizz 1's DOS port judges its detail level the same way, in the frame-rate window and only while the frame rate is Auto,
    /// so with a fixed rate an Auto sky stays Light.
    pub const fn with_auto_sky(mut self, auto: bool) -> Self {
        self.auto_sky = auto;
        self.full_sky = false;
        self
    }

    /// Whether an Auto sky is Full now (always `false` when the sky is not on Auto).
    pub const fn full_sky(&self) -> bool {
        self.full_sky
    }

    /// The mode it was made with.
    pub const fn mode(&self) -> FrameRate {
        self.mode
    }

    /// The rate in use now: 60 or 30.
    pub const fn rate(&self) -> u32 {
        self.hz
    }

    /// Whether to draw after the simulation reached `tick`. At 60 Hz always; at 30 Hz once two ticks have passed since the last
    /// frame drawn. Call it once for each pass of the loop that stepped, and draw (and call `record_frame`) when it says yes.
    pub fn draw_this_tick(&mut self, tick: u64) -> bool {
        let yes = self.hz == 60 || tick >= self.drawn + 2;
        if yes {
            self.drawn = tick;
        }
        yes
    }

    /// Takes the cost of the frame just drawn, in ticks of the 560 Hz clock (draw, wait for the retrace if any, and copy). In Auto
    /// the window of 32 frames closes here and the rate may change; it returns the window's average as a percentage of a 60 Hz
    /// frame when it closes, otherwise `None`. The fixed modes never measure and never change.
    pub fn record_frame(&mut self, cost_ticks_560: u32) -> Option<u32> {
        if self.mode != FrameRate::Auto {
            return None;
        }
        self.spent += u64::from(cost_ticks_560);
        self.frames += 1;
        if self.frames < WINDOW {
            return None;
        }
        let pct = u32::try_from(self.spent * 60 * 100 / (u64::from(WINDOW) * u64::from(CLOCK_HZ)))
            .unwrap_or(u32::MAX);
        self.spent = 0;
        self.frames = 0;
        // The sky first: a window that changes it measured the other sky, so it does not decide the rate as well.
        if self.auto_sky {
            if !self.full_sky && pct < SKY_FULL_BELOW && self.hz == 60 {
                self.full_sky = true;
                return Some(pct);
            }
            if self.full_sky && pct > SKY_LIGHT_ABOVE {
                self.full_sky = false;
                return Some(pct);
            }
        }
        if (CHOOSE_30_ABOVE + 1..=CHOOSE_30_UP_TO).contains(&pct) {
            self.hz = 30;
        } else if !(CHOOSE_60_BELOW..=CHOOSE_60_ABOVE).contains(&pct) {
            self.hz = 60;
        }
        Some(pct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    /// The total cost, in 560 Hz ticks, of a window whose average is `pct` of a 60 Hz frame (the least total that gives `pct`).
    fn window_total(pct: u32) -> u64 {
        let t = (u64::from(pct) * 32 * 560).div_ceil(6000);
        assert_eq!(
            t * 6000 / (32 * 560),
            u64::from(pct),
            "{pct} is not reachable"
        );
        t
    }

    /// Feeds one window averaging `pct`, spread over its 32 frames, and returns what the last call returned. The first 31 must be `None`.
    fn feed(g: &mut FrameGovernor, pct: u32) -> Option<u32> {
        let total = window_total(pct);
        let mut last = None;
        for i in 0..32u64 {
            let share = total / 32 + u64::from(i < total % 32);
            last = g.record_frame(share as u32);
            assert!(i == 31 || last.is_none(), "window closed early at {i}");
        }
        last
    }

    #[test]
    fn an_auto_sky_starts_light_goes_full_on_cheap_frames_and_back_on_dear_ones() {
        let mut g = FrameGovernor::new(FrameRate::Auto).with_auto_sky(true);
        assert!(!g.full_sky());
        // The floor: Light frames at about 63% keep Light and 60 Hz.
        for _ in 0..3 {
            feed(&mut g, 63);
            assert!(!g.full_sky());
            assert_eq!(g.rate(), 60);
        }
        // A machine that draws Light under 45% gets Full, and keeps it while Full stays at or under 75%.
        feed(&mut g, 44);
        assert!(g.full_sky());
        feed(&mut g, 75);
        assert!(g.full_sky());
        // Over 75% Full goes back to Light, and that window does not change the rate, even dear enough for 30 Hz.
        feed(&mut g, 120);
        assert!(!g.full_sky());
        assert_eq!(g.rate(), 60);
        // The next window, measured with Light, decides the rate as before.
        feed(&mut g, 120);
        assert_eq!(g.rate(), 30);
        // At 30 Hz an Auto sky does not go Full, however cheap the frames.
        feed(&mut g, 70);
        assert_eq!(g.rate(), 30);
        assert!(!g.full_sky());
    }

    #[test]
    fn a_fixed_rate_or_a_fixed_sky_never_changes_the_sky() {
        let mut fixed_rate = FrameGovernor::new(FrameRate::Hz60).with_auto_sky(true);
        let mut fixed_sky = FrameGovernor::new(FrameRate::Auto);
        for _ in 0..3 {
            for _ in 0..32 {
                assert_eq!(fixed_rate.record_frame(1), None);
            }
            feed(&mut fixed_sky, 10);
        }
        assert!(!fixed_rate.full_sky());
        assert!(!fixed_sky.full_sky());
    }

    #[test]
    fn cheap_frames_stay_at_60() {
        let mut g = FrameGovernor::new(FrameRate::Auto);
        assert_eq!(g.rate(), 60);
        for _ in 0..4 {
            assert_eq!(feed(&mut g, 70), Some(70));
            assert_eq!(g.rate(), 60);
        }
    }

    #[test]
    fn a_120_percent_cost_goes_to_30() {
        let mut g = FrameGovernor::new(FrameRate::Auto);
        assert_eq!(feed(&mut g, 120), Some(120));
        assert_eq!(g.rate(), 30);
    }

    #[test]
    fn the_dead_bands_keep_the_rate() {
        // 180% is inside the band between 170% and 185%: a rate of 30 stays, and so does a rate of 60.
        let mut g = FrameGovernor::new(FrameRate::Auto);
        feed(&mut g, 120);
        feed(&mut g, 180);
        assert_eq!(g.rate(), 30);
        // 67% to 100% keeps 30 too.
        feed(&mut g, 80);
        assert_eq!(g.rate(), 30);
        let mut g = FrameGovernor::new(FrameRate::Auto);
        feed(&mut g, 180);
        assert_eq!(g.rate(), 60);
        feed(&mut g, 90);
        assert_eq!(g.rate(), 60);
    }

    #[test]
    fn a_200_percent_cost_goes_back_to_60() {
        let mut g = FrameGovernor::new(FrameRate::Auto);
        feed(&mut g, 120);
        assert_eq!(g.rate(), 30);
        feed(&mut g, 200);
        assert_eq!(g.rate(), 60);
    }

    #[test]
    fn below_67_percent_at_30_goes_to_60() {
        let mut g = FrameGovernor::new(FrameRate::Auto);
        feed(&mut g, 120);
        assert_eq!(g.rate(), 30);
        feed(&mut g, 60);
        assert_eq!(g.rate(), 60);
    }

    #[test]
    fn the_window_is_exactly_32_frames() {
        let mut g = FrameGovernor::new(FrameRate::Auto);
        for _ in 0..31 {
            assert_eq!(g.record_frame(12), None);
            assert_eq!(g.rate(), 60);
        }
        // 12 ticks a frame is 128%: the 32nd frame closes the window and the rate changes with it.
        assert_eq!(g.record_frame(12), Some(128));
        assert_eq!(g.rate(), 30);
        // The next window starts empty and is 32 frames long too.
        for _ in 0..31 {
            assert_eq!(g.record_frame(0), None);
        }
        assert_eq!(g.record_frame(0), Some(0));
        assert_eq!(g.rate(), 60);
    }

    #[test]
    fn the_edges_of_the_bands() {
        // 100% is not above 100; 170% is the last of the 30 band; 185% is not above 185; 67% is not below 67.
        let run = |pct: u32, start_at_30: bool| {
            let mut g = FrameGovernor::new(FrameRate::Auto);
            if start_at_30 {
                feed(&mut g, 120);
            }
            let before = g.rate();
            assert_eq!(feed(&mut g, pct), Some(pct));
            (before, g.rate())
        };
        assert_eq!(run(100, false), (60, 60));
        assert_eq!(run(101, false), (60, 30));
        assert_eq!(run(170, false), (60, 30));
        assert_eq!(run(185, true), (30, 30));
        assert_eq!(run(186, true), (30, 60));
        assert_eq!(run(67, true), (30, 30));
        assert_eq!(run(66, true), (30, 60));
    }

    #[test]
    fn fixed_modes_never_change() {
        for (mode, hz) in [(FrameRate::Hz60, 60), (FrameRate::Hz30, 30)] {
            let mut g = FrameGovernor::new(mode);
            assert_eq!(g.rate(), hz);
            for cost in [0, 5, 12, 20, 50] {
                for _ in 0..70 {
                    assert_eq!(g.record_frame(cost), None);
                }
                assert_eq!(g.rate(), hz);
            }
        }
    }

    #[test]
    fn sixty_draws_every_tick_and_thirty_every_second() {
        let mut g = FrameGovernor::new(FrameRate::Hz60);
        assert!((1..=10).all(|t| g.draw_this_tick(t)));
        let mut g = FrameGovernor::new(FrameRate::Hz30);
        let drawn: Vec<u64> = (1..=10).filter(|&t| g.draw_this_tick(t)).collect();
        assert_eq!(drawn, [2, 4, 6, 8, 10]);
        // A stall that skips ticks still draws at once, then waits two.
        let mut g = FrameGovernor::new(FrameRate::Hz30);
        assert!(g.draw_this_tick(5));
        assert!(!g.draw_this_tick(6));
        assert!(g.draw_this_tick(7));
    }

    #[test]
    fn auto_changes_the_drawing_cadence_at_the_window_end() {
        let mut g = FrameGovernor::new(FrameRate::Auto);
        assert!(g.draw_this_tick(1) && g.draw_this_tick(2));
        feed(&mut g, 120);
        assert_eq!(g.rate(), 30);
        assert!(!g.draw_this_tick(3));
        assert!(g.draw_this_tick(4));
    }
}
