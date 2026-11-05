// Ben's long fall as a sound cue: derived from his state, once per fall, with no rules event.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`FallWatch`] plays the slide whistle ([`EFFECT_WHISTLE`]) the moment Ben leaves a surface without jumping: walking, sliding or
//! being carried off an edge, or off the end of a pole if the game gets poles. It is a presenter's view state in the way
//! [`crate::Puffs`] is: each presenter keeps one, calls [`FallWatch::observe`] after every `fizz_ep2::step` and plays what it
//! returns. The rules do not know about it, so the state hash and the replay transcript cannot move, and the Web and DOS
//! presenters agree because they run the same code on the same world.
//!
//! The rule, read from Ben's state after a step:
//!
//! - **A stretch** starts when Ben leaves the ground and ends when he lands, dies or is on the pogo stick (each of which resets it).
//! - **The whistle** plays on the first step of a stretch that began by leaving a surface level or sinking, with no drop to wait for:
//!   a half-tile step off a platform whistles too, and a landing a moment later just ends the sound. It plays once for the stretch;
//!   the next one needs a landing first.
//! - **Silent stretches:** a stretch that began with a jump (Ben left the ground rising) never whistles, and neither does one in which
//!   the pogo stick was on at any point, including switching it off in the air. A death has its own cry ("whoa!") and plays nothing.
//! - **The place** is Ben, in the view the picture is drawn with, as for any sound.
//!
//! The cue raises the caption "wheee!" ([`CAP_WHEEE`]) above Ben with the sound, through [`EventCaptions::raise_at`], since no event
//! carries it. Like every caption it is gated only when drawn, and it is not rules state.

use crate::{DrawOptions, EventCaptions, camera_shaken};
use fizz_ep2::World;
use fizz_ep2::cues::{CAP_WHEEE, EFFECT_WHISTLE};
use fizz_sfx::place::{Placement, View, place};
use fizz_view::{screen_y, tile_px};

/// The fall cue's state: whether Ben stood on the last step observed, and whether this stretch can no longer whistle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FallWatch {
    /// Ben stood on the last step observed, so a step in the air is the first of a stretch.
    grounded: bool,
    /// The stretch began with a jump or had the pogo stick on, or the whistle has already played in it.
    spent: bool,
}

impl Default for FallWatch {
    fn default() -> Self {
        Self::new()
    }
}

impl FallWatch {
    /// A watch with no stretch under way.
    pub const fn new() -> Self {
        FallWatch {
            grounded: false,
            spent: false,
        }
    }

    /// Forgets the stretch (a presenter restarting the level).
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// Reads Ben's state after the step that just ran. Returns the effect id and where it sounds, in a `w` by `h` view drawn with
    /// `options`, on the step that crosses the threshold and `None` on every other. Pure apart from the watch's own two fields.
    pub fn observe(
        &mut self,
        world: &World,
        w: usize,
        h: usize,
        options: &DrawOptions,
        captions: &mut EventCaptions,
    ) -> Option<(u8, Placement)> {
        let ben = &world.ben;
        if ben.pogo || ben.dead > 0.0 {
            self.grounded = false;
            self.spent = true;
            return None;
        }
        if ben.b.on_ground {
            self.grounded = true;
            self.spent = false;
            return None;
        }
        if self.grounded {
            // The first step in the air: a jump leaves the ground rising, a step off an edge does not.
            self.grounded = false;
            self.spent = ben.b.vy > 0.0;
        }
        if self.spent {
            return None;
        }
        self.spent = true;
        // The word floats from Ben's head, like every caption, whatever the Captions setting says (the setting gates drawing only).
        let head = (
            tile_px(ben.b.x + ben.b.w * 0.5),
            screen_y(ben.b.y + ben.b.h, world.map.h),
        );
        captions.raise_at(world, CAP_WHEEE, head, w, h, options);
        let (cam_x, cam_y) = camera_shaken(world, w, h, options);
        let at = place(
            tile_px(ben.b.x + ben.b.w * 0.5),
            screen_y(ben.b.y + ben.b.h * 0.5, world.map.h),
            View {
                x: cam_x,
                y: cam_y,
                w: w as i32,
                h: h as i32,
            },
        );
        Some((EFFECT_WHISTLE, at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VIEW_H, VIEW_W};
    use fizz_ep2::level;
    use fizz_ep2::world::step;
    use fizz_sim::{Buttons, InputFrame};

    extern crate std;
    use std::vec::Vec;

    fn opts() -> DrawOptions {
        DrawOptions::default()
    }

    fn observe(watch: &mut FallWatch, w: &World) -> bool {
        watch
            .observe(w, VIEW_W, VIEW_H, &opts(), &mut EventCaptions::new())
            .is_some()
    }

    /// A world with Ben standing, observed on the ground, then put in the air with vertical speed `vy`: the first step of a stretch.
    fn leaves_the_ground(vy: f64) -> (World, FallWatch, bool) {
        let mut w = level::world();
        let mut watch = FallWatch::new();
        w.ben.b.on_ground = true;
        assert!(!observe(&mut watch, &w));
        w.ben.b.on_ground = false;
        w.ben.b.vy = vy;
        let played = observe(&mut watch, &w);
        (w, watch, played)
    }

    #[test]
    fn leaving_a_surface_level_or_sinking_whistles_at_once() {
        for vy in [0.0, -0.5, -8.0] {
            let (_, _, played) = leaves_the_ground(vy);
            assert!(played, "vy {vy}: the first step in the air");
        }
    }

    #[test]
    fn the_whistle_raises_wheee_above_ben_and_a_silent_stretch_raises_nothing() {
        for (vy, words) in [(-1.0, 1), (20.0, 0)] {
            let mut w = level::world();
            let mut watch = FallWatch::new();
            let mut captions = EventCaptions::new();
            w.ben.b.on_ground = true;
            watch.observe(&w, VIEW_W, VIEW_H, &opts(), &mut captions);
            w.ben.b.on_ground = false;
            w.ben.b.vy = vy;
            watch.observe(&w, VIEW_W, VIEW_H, &opts(), &mut captions);
            let live: Vec<_> = captions.views(&w, VIEW_W, VIEW_H, &opts()).collect();
            assert_eq!(live.len(), words, "vy {vy}");
            if let Some(c) = live.first() {
                assert_eq!(c.text, "wheee!");
                assert!(c.event);
            }
        }
    }

    #[test]
    fn a_small_step_down_whistles_too() {
        let (mut w, mut watch, played) = leaves_the_ground(-1.0);
        assert!(played);
        // Landing half a tile lower ends the stretch and plays nothing more.
        w.ben.b.y -= 0.5;
        w.ben.b.on_ground = true;
        assert!(!observe(&mut watch, &w));
    }

    #[test]
    fn it_plays_once_and_again_only_after_a_landing() {
        let (mut w, mut watch, played) = leaves_the_ground(-1.0);
        assert!(played);
        for i in 0..40 {
            w.ben.b.y -= 0.5;
            w.ben.b.vy = -8.0 - f64::from(i);
            assert!(!observe(&mut watch, &w), "once per stretch");
        }
        w.ben.b.on_ground = true;
        assert!(!observe(&mut watch, &w));
        w.ben.b.on_ground = false;
        w.ben.b.vy = -1.0;
        assert!(observe(&mut watch, &w), "the next ledge");
    }

    #[test]
    fn a_jump_never_plays_it_not_even_on_the_way_down() {
        let (mut w, mut watch, played) = leaves_the_ground(20.0);
        assert!(!played, "rising");
        for (y, vy) in [(13.0, 5.0), (13.5, 0.0), (12.0, -4.0), (4.0, -20.0)] {
            w.ben.b.y = y;
            w.ben.b.vy = vy;
            assert!(!observe(&mut watch, &w), "y {y}, vy {vy}");
        }
    }

    #[test]
    fn the_pogo_never_plays_it_even_when_it_is_switched_off_in_the_air() {
        let mut w = level::world();
        let mut watch = FallWatch::new();
        w.ben.pogo = true;
        w.ben.b.on_ground = false;
        w.ben.b.vy = -8.0;
        for _ in 0..10 {
            assert!(!observe(&mut watch, &w));
        }
        w.ben.pogo = false;
        for i in 0..40 {
            w.ben.b.y -= 0.5;
            w.ben.b.vy = -8.0 - f64::from(i);
            assert!(
                !observe(&mut watch, &w),
                "off in the air, still the pogo's stretch"
            );
        }
        w.ben.b.on_ground = true;
        assert!(!observe(&mut watch, &w));
        w.ben.b.on_ground = false;
        assert!(observe(&mut watch, &w), "a landing starts afresh");
    }

    #[test]
    fn a_death_never_plays_it() {
        let mut w = level::world();
        let mut watch = FallWatch::new();
        w.ben.b.on_ground = false;
        w.ben.b.vy = -8.0;
        w.ben.dead = 0.5;
        for i in 0..40 {
            w.ben.b.y -= 0.5;
            assert!(!observe(&mut watch, &w), "step {i}");
        }
    }

    #[test]
    fn a_stretch_that_starts_in_the_air_is_not_a_walk_off() {
        // A presenter that starts watching with Ben already falling (a level start) has seen no surface.
        let mut w = level::world();
        let mut watch = FallWatch::new();
        w.ben.b.on_ground = false;
        w.ben.b.vy = -4.0;
        assert!(
            observe(&mut watch, &w),
            "falling with no prior ground plays once"
        );
    }

    /// Runs `input` for `ticks` steps on `w`, returning the ticks at which the watch cued.
    fn run(w: &mut World, input: Buttons, ticks: u32) -> Vec<u64> {
        // The watch sees Ben standing first, as a presenter's does on the ticks before he moves.
        let mut watch = FallWatch::new();
        watch.observe(w, VIEW_W, VIEW_H, &opts(), &mut EventCaptions::new());
        let mut prev = Buttons::NONE;
        let mut out = Vec::new();
        for _ in 0..ticks {
            step(w, InputFrame::from_transition(prev, input));
            prev = input;
            if watch
                .observe(w, VIEW_W, VIEW_H, &opts(), &mut EventCaptions::new())
                .is_some()
            {
                out.push(w.tick.0);
            }
        }
        out
    }

    #[test]
    fn the_roof_of_the_exit_gallery_whistles_when_ben_walks_off_and_not_when_he_jumps_off() {
        // Ben stands on the grey slab (row 41, feet at y 42) and the gallery floor is at feet y 38: a net drop of 4.
        for (input, whistles) in [(Buttons::RIGHT, 1), (Buttons::RIGHT | Buttons::JUMP, 0)] {
            let mut w = level::world();
            w.ben.b.x = 27.0;
            w.ben.b.y = 42.0;
            w.ben.b.px = 27.0;
            w.ben.b.py = 42.0;
            w.ben.b.on_ground = true;
            let cues = run(&mut w, input, 90);
            assert_eq!(cues.len(), whistles, "{input:?}: {cues:?}");
            assert!(w.ben.b.y < 41.0, "Ben came down to the gallery floor");
        }
    }

    #[test]
    fn a_jump_on_level_ground_never_plays_it() {
        let mut w = level::world();
        let cues = run(&mut w, Buttons::JUMP, 60);
        assert!(cues.is_empty(), "{cues:?}");
        assert!(w.ben.b.on_ground, "and Ben has landed");
    }

    #[test]
    fn the_replay_plays_the_whistle_at_known_ticks() {
        // Every walk off a surface in the route, which the cue derives from Ben's state alone.
        let mut w = level::world();
        let mut watch = FallWatch::new();
        let mut prev = Buttons::NONE;
        let mut ticks = Vec::new();
        for &(frames, now) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..frames {
                step(&mut w, InputFrame::from_transition(prev, now));
                prev = now;
                if watch
                    .observe(&w, VIEW_W, VIEW_H, &opts(), &mut EventCaptions::new())
                    .is_some()
                {
                    ticks.push(w.tick.0);
                }
            }
        }
        // The tick after Ben walks off the two ledges in the route. The pogo ride's end at tick 1668 is a pogo stretch, and silent.
        assert_eq!(ticks, [308, 378]);
    }
}
