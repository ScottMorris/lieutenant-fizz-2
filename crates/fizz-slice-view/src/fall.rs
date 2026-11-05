// Ben's long fall as a sound cue: derived from his state, once per fall, with no rules event.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`FallWatch`] plays the slide whistle ([`EFFECT_WHISTLE`]) when Ben has dropped [`FALL_TILES`] or more below the highest point
//! of his current airborne stretch. It is a presenter's view state in the way [`crate::Puffs`] is: each presenter keeps one, calls
//! [`FallWatch::observe`] after every `fizz_ep2::step` and plays what it returns. The rules do not know about it, so the state
//! hash and the replay transcript cannot move, and the Web and DOS presenters agree because they run the same code on the same
//! world.
//!
//! The rule, read from Ben's state after a step:
//!
//! - **A stretch** starts when Ben leaves the ground and ends when he lands, dies or is on the pogo stick. The highest `y` seen in
//!   it is its peak. A pogo stick bounces from the ground all the time and has no falls, and a death has its own cry ("whoa!"),
//!   so neither plays the whistle, and both reset the stretch.
//! - **The whistle** plays on the first step on which Ben is falling and is [`FALL_TILES`] under the peak. It plays once for the
//!   stretch; the next one needs a landing first. A short drop, a hop, a full jump (3.5 tiles up, then the same down) and a stomp
//!   bounce never reach the threshold from their own peak, so the whistle is for a real drop: a ledge, a gap or a shaft.
//! - **The place** is Ben, in the view the picture is drawn with, as for any sound.
//!
//! The cue has no caption: the whistle is the comic sound of a fall and, like the jump, wants no word.

use crate::{DrawOptions, camera_shaken};
use fizz_ep2::World;
use fizz_ep2::cues::EFFECT_WHISTLE;
use fizz_sfx::place::{Placement, View, place};
use fizz_view::{screen_y, tile_px};

/// How far under the peak of an airborne stretch Ben must be, in tiles, for the whistle. A full jump rises 3.5 tiles
/// (`20.5^2 / (2 * 60)`), so a hop from the floor never reaches it, and a drop over the edge of a ledge of five tiles does.
pub const FALL_TILES: f64 = 5.0;
const FALL_SIXTEENTHS: i32 = 80;

/// The fall cue's state: the peak of the airborne stretch, and whether it has already played.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FallWatch {
    /// The highest `y` (y up, in sixteenths of a tile, rounded down) of Ben's body since he last stood, or his `y` while he stands.
    peak: i32,
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
            peak: 0,
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
            self.peak = y;
            self.played = false;
            return None;
        }
        if y > self.peak {
            self.peak = y;
        }
        if self.played || ben.b.vy >= 0.0 || self.peak - y < FALL_SIXTEENTHS {
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

    fn opts() -> DrawOptions {
        DrawOptions::default()
    }

    /// The first step on which the watch returns a cue while Ben is put `drop` tiles under a peak, one tile at a time.
    fn plays_for_a_drop_of(drop: f64) -> bool {
        let mut w = level::world();
        let mut watch = FallWatch::new();
        w.ben.b.on_ground = false;
        w.ben.b.vy = -5.0;
        let top = w.ben.b.y;
        let mut hit = false;
        let mut dy = 0.0;
        while dy <= drop {
            w.ben.b.y = top - dy;
            hit |= watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_some();
            dy += 0.25;
        }
        hit
    }

    #[test]
    fn a_drop_of_the_threshold_plays_and_a_shorter_one_does_not() {
        assert!(!plays_for_a_drop_of(FALL_TILES - 0.5));
        assert!(plays_for_a_drop_of(FALL_TILES));
        assert!(plays_for_a_drop_of(FALL_TILES + 3.0));
    }

    #[test]
    fn it_plays_once_for_a_fall_and_again_after_a_landing() {
        let mut w = level::world();
        let mut watch = FallWatch::new();
        w.ben.b.on_ground = false;
        w.ben.b.vy = -8.0;
        let top = w.ben.b.y;
        let mut plays = 0;
        for i in 0..40 {
            w.ben.b.y = top - f64::from(i) * 0.5;
            plays += usize::from(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_some());
        }
        assert_eq!(plays, 1, "once for the fall");
        w.ben.b.on_ground = true;
        assert!(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_none());
        w.ben.b.on_ground = false;
        let top = w.ben.b.y;
        for i in 0..40 {
            w.ben.b.y = top - f64::from(i) * 0.5;
            plays += usize::from(watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_some());
        }
        assert_eq!(plays, 2, "and again after he lands");
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

    #[test]
    fn the_full_jump_stays_under_the_threshold() {
        // 20.5 is the ground jump's launch speed (`ben.rs`) and 60 is `fizz_phys::GRAVITY`.
        let apex = 20.5f64 * 20.5 / (2.0 * 60.0);
        assert!(apex < FALL_TILES - 1.0, "a jump rises {apex} tiles");
    }

    #[test]
    fn the_replay_plays_the_whistle_at_known_ticks() {
        // The route drops down the shafts on its way to the exit; this pins where, and proves the cue is derived from Ben's state alone.
        let mut w = level::world();
        let mut watch = FallWatch::new();
        let mut prev = Buttons::NONE;
        let mut ticks = std::vec::Vec::new();
        for &(frames, now) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..frames {
                step(&mut w, InputFrame::from_transition(prev, now));
                prev = now;
                if watch.observe(&w, VIEW_W, VIEW_H, &opts()).is_some() {
                    ticks.push(w.tick.0);
                }
            }
        }
        assert_eq!(ticks, [331], "one long fall, at tick 331");
    }
}
