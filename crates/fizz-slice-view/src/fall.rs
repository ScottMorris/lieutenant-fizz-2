// Ben's long fall as a sound cue: derived from his state, once per fall, with no rules event.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`FallWatch`] plays the slide whistle ([`EFFECT_WHISTLE`]) when Ben's feet are [`FALL_TILES`] or more below where he left the
//! ground. It is a presenter's view state in the way [`crate::Puffs`] is: each presenter keeps one, calls [`FallWatch::observe`]
//! after every `fizz_ep2::step` and plays what it returns. The rules do not know about it, so the state hash and the replay
//! transcript cannot move, and the Web and DOS presenters agree because they run the same code on the same world.
//!
//! The rule, read from Ben's state after a step:
//!
//! - **A stretch** starts when Ben leaves the ground and ends when he lands, dies or is on the pogo stick. The height of his feet on
//!   the last step he stood is the takeoff. A pogo stick bounces from the ground all the time and has no falls, and a death has its
//!   own cry ("whoa!"), so neither plays the whistle, and both reset the stretch.
//! - **The whistle** plays on the first step on which Ben is falling and his feet are [`FALL_TILES`] or more under the takeoff. It plays once
//!   for the stretch; the next one needs a landing first. The measure is the net drop, not the fall from the top of a jump: a full
//!   jump (3.5 tiles up, then 3.5 down) comes back to its takeoff and is silent, a drop of under a tile (a half-tile
//!   step, the settling of a landing) is silent, landing on something higher than the takeoff is silent, and a step or jump off
//!   a ledge a tile or more high is not, which includes the four-tile drop from the exit gallery's stone roof to its wooden floor.
//! - **The place** is Ben, in the view the picture is drawn with, as for any sound.
//!
//! The cue has no caption: the whistle is the comic sound of a fall and, like the jump, wants no word.

use crate::{DrawOptions, camera_shaken};
use fizz_ep2::World;
use fizz_ep2::cues::EFFECT_WHISTLE;
use fizz_sfx::place::{Placement, View, place};
use fizz_view::{screen_y, tile_px};

/// How far under the takeoff Ben's feet must be, in tiles, for the whistle. A full jump rises and falls 3.5 tiles and lands at net 0,
/// so it never reaches it; a step down of a tile does, and so does the four-tile drop from the stone roof of the exit gallery.
pub const FALL_TILES: f64 = 1.0;
const FALL_SIXTEENTHS: i32 = 16;

/// The fall cue's state: the takeoff height, and whether the whistle has already played in this stretch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FallWatch {
    /// The height of Ben's feet (y up, in sixteenths of a tile, rounded down) on the last step he stood.
    takeoff: i32,
    /// The whistle has played in this stretch.
    played: bool,
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
            takeoff: 0,
            played: false,
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
    ) -> Option<(u8, Placement)> {
        let ben = &world.ben;
        let y = (ben.b.y * 16.0) as i32;
        if ben.b.on_ground || ben.pogo || ben.dead > 0.0 {
            self.takeoff = y;
            self.played = false;
            return None;
        }
        if self.played || ben.b.vy >= 0.0 || self.takeoff - y < FALL_SIXTEENTHS {
            return None;
        }
        self.played = true;
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

    /// A world with Ben standing at the top of a synthetic drop: observed on the ground, then put in the air and moved down one
    /// quarter tile at a time to `drop` tiles under the takeoff. Returns how many times the watch cued.
    fn plays_for_a_drop_of(drop: f64) -> usize {
        let mut w = level::world();
        let mut watch = FallWatch::new();
        let top = w.ben.b.y + 10.0;
        w.ben.b.y = top;
        w.ben.b.on_ground = true;
        assert!(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_none());
        w.ben.b.on_ground = false;
        w.ben.b.vy = -5.0;
        let mut plays = 0;
        let mut dy = 0.0;
        while dy <= drop {
            w.ben.b.y = top - dy;
            plays += usize::from(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_some());
            dy += 0.25;
        }
        plays
    }

    #[test]
    fn a_net_drop_of_a_tile_plays_and_a_shorter_one_does_not() {
        assert_eq!(plays_for_a_drop_of(0.5), 0, "a half-tile step down");
        assert_eq!(plays_for_a_drop_of(FALL_TILES - 0.25), 0);
        assert_eq!(plays_for_a_drop_of(1.0), 1, "a one-tile step down");
        assert_eq!(plays_for_a_drop_of(2.0), 1, "a two-tile hop down");
        assert_eq!(plays_for_a_drop_of(4.0), 1);
        assert_eq!(
            plays_for_a_drop_of(30.0),
            1,
            "a long fall plays exactly once"
        );
    }

    #[test]
    fn it_plays_again_only_after_a_landing() {
        let mut w = level::world();
        let mut watch = FallWatch::new();
        let mut plays = 0;
        for round in 1..=2 {
            w.ben.b.on_ground = true;
            assert!(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_none());
            w.ben.b.on_ground = false;
            w.ben.b.vy = -8.0;
            let top = w.ben.b.y;
            for i in 0..40 {
                w.ben.b.y = top - f64::from(i) * 0.5;
                plays += usize::from(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_some());
            }
            assert_eq!(plays, round);
            w.ben.b.y = top;
        }
    }

    #[test]
    fn a_jump_up_to_a_ledge_and_a_fall_to_the_old_level_nets_under_a_tile() {
        // Take off at 10, rise to 13.5 and come back down to 9.5: a net drop of half a tile, and no whistle. A landing on a ledge at
        // 12 resets the takeoff to 12, and dropping from it to 11.5 is again under a tile.
        let mut w = level::world();
        let mut watch = FallWatch::new();
        let mut plays = 0;
        w.ben.b.y = 10.0;
        w.ben.b.on_ground = true;
        watch.observe(&w, VIEW_W, VIEW_H, &opts());
        w.ben.b.on_ground = false;
        for (y, vy) in [
            (11.0, 5.0),
            (12.5, 2.0),
            (13.5, 0.5),
            (12.5, -2.0),
            (10.5, -4.0),
            (9.5, -6.0),
        ] {
            w.ben.b.y = y;
            w.ben.b.vy = vy;
            plays += usize::from(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_some());
        }
        assert_eq!(plays, 0, "up 3.5 and back down to half a tile under");
        w.ben.b.y = 12.0;
        w.ben.b.on_ground = true;
        watch.observe(&w, VIEW_W, VIEW_H, &opts());
        w.ben.b.on_ground = false;
        w.ben.b.vy = -4.0;
        w.ben.b.y = 11.5;
        assert!(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_none());
        w.ben.b.y = 10.9;
        assert_eq!(
            watch.observe(&w, VIEW_W, VIEW_H, &opts()).map(|c| c.0),
            Some(EFFECT_WHISTLE)
        );
    }

    #[test]
    fn landing_on_something_higher_than_the_takeoff_never_plays_it() {
        let mut w = level::world();
        let mut watch = FallWatch::new();
        w.ben.b.y = 10.0;
        w.ben.b.on_ground = true;
        watch.observe(&w, VIEW_W, VIEW_H, &opts());
        w.ben.b.on_ground = false;
        w.ben.b.vy = 5.0;
        for i in 0..8 {
            w.ben.b.y = 10.0 + f64::from(i);
            assert!(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_none());
        }
    }

    #[test]
    fn the_pogo_and_a_death_never_play_it() {
        for pogo in [true, false] {
            let mut w = level::world();
            let mut watch = FallWatch::new();
            w.ben.b.on_ground = false;
            w.ben.b.vy = -8.0;
            w.ben.pogo = pogo;
            w.ben.dead = if pogo { 0.0 } else { 0.5 };
            let top = w.ben.b.y;
            for i in 0..40 {
                w.ben.b.y = top - f64::from(i) * 0.5;
                assert!(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_none());
            }
        }
    }

    #[test]
    fn rising_never_plays_it() {
        let mut w = level::world();
        let mut watch = FallWatch::new();
        w.ben.b.on_ground = false;
        w.ben.b.vy = 10.0;
        let base = w.ben.b.y;
        for i in 0..40 {
            w.ben.b.y = base + f64::from(i);
            assert!(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_none());
        }
    }

    /// Runs `input` for `ticks` steps on `w`, returning the ticks at which the watch cued.
    fn run(w: &mut World, input: Buttons, ticks: u32) -> Vec<u64> {
        // The watch sees Ben standing first, as a presenter's does on the ticks before he moves.
        let mut watch = FallWatch::new();
        watch.observe(w, VIEW_W, VIEW_H, &opts());
        let mut prev = Buttons::NONE;
        let mut out = Vec::new();
        for _ in 0..ticks {
            step(w, InputFrame::from_transition(prev, input));
            prev = input;
            if watch.observe(w, VIEW_W, VIEW_H, &opts()).is_some() {
                out.push(w.tick.0);
            }
        }
        out
    }

    #[test]
    fn the_roof_of_the_exit_gallery_is_a_whistling_drop_whether_ben_walks_or_jumps_off() {
        // Ben stands on the grey slab (row 41, feet at y 42) and the gallery floor is at feet y 38: a net drop of 4.
        for input in [Buttons::RIGHT, Buttons::RIGHT | Buttons::JUMP] {
            let mut w = level::world();
            w.ben.b.x = 27.0;
            w.ben.b.y = 42.0;
            w.ben.b.px = 27.0;
            w.ben.b.py = 42.0;
            w.ben.b.on_ground = true;
            let cues = run(&mut w, input, 90);
            assert_eq!(
                cues.len(),
                1,
                "{input:?}: {cues:?} {} {} {}",
                w.ben.b.x,
                w.ben.b.y,
                w.ben.b.on_ground
            );
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
    fn the_full_jump_comes_back_to_its_takeoff() {
        // 20.5 is the ground jump's launch speed (`ben.rs`) and 60 is `fizz_phys::GRAVITY`: it rises and falls the same 3.5 tiles.
        let apex = 20.5f64 * 20.5 / (2.0 * 60.0);
        assert!(apex > 3.4 && apex < 3.6, "a jump rises {apex} tiles");
    }

    #[test]
    fn the_replay_plays_the_whistle_at_known_ticks() {
        // The route's falls that reach the threshold, which the cue derives from Ben's state alone.
        let mut w = level::world();
        let mut watch = FallWatch::new();
        let mut prev = Buttons::NONE;
        let mut ticks = Vec::new();
        for &(frames, now) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..frames {
                step(&mut w, InputFrame::from_transition(prev, now));
                prev = now;
                if watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_some() {
                    ticks.push(w.tick.0);
                }
            }
        }
        assert_eq!(ticks, [318, 388, 1682], "three drops of a tile or more");
    }
}
