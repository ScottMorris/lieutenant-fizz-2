// Ben's movement: walk, jump, pogo and physics, ported line for line from Fizz 1's `tick_ben_move`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: ported from Fizz 1 `episodes/episode-1/game/src/world.rs` at commit `4bce036`: `Player::level`,
// `tick_ben` (the alive and dead branches), `tick_ben_move`, `tick_head_bump` and the fall check of `tick_ben_contacts`.
// Left out on purpose: mantle (automatic and in progress), ladders and vines, wall kicks, moving platforms under Ben,
// jokes and counters, captions and sound, and the camera. Departure: velocities below 2^-1000 snap to zero.

use fizz_phys::{Body, GRAVITY, Platform, STEP, TileMap};
use fizz_sim::{Buttons, InputFrame};

/// The pogo bounce height in tiles when Jump is held, which gives the high bounce its launch speed (Fizz 1's default).
pub const POGO_HEIGHT: f64 = 6.6;
/// How tall Ben is drawn on the pogo stick, against his 1.4-tile body. The taller head box uses it.
pub const POGO_DRAWN_HEIGHT: f64 = 2.0;
/// How fast Ben must be rising for his head meeting a ceiling to count as a bump.
pub const HEAD_BUMP_MIN_SPEED: f64 = 1.0;
/// How far Ben must have risen above his last footing for a bump to count.
pub const HEAD_BUMP_MIN_RISE: f64 = 0.05;
/// Ben dies when his body's bottom is below this height: he has fallen out of the map.
pub const FALL_DEATH_Y: f64 = -1.5;
/// Below this magnitude a velocity snaps to zero, so an idle pogo's drag never reaches the subnormal range, where x87 at
/// 53 bits and SSE2 round differently. This is a departure from Fizz 1, which lets the value decay. It is 2^-1000.
const SNAP: f64 = f64::from_bits((1023 - 1000) << 52);

fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn snap(v: f64) -> f64 {
    if v.abs() < SNAP { 0.0 } else { v }
}

/// Ben: the body and the state that drives his movement. Fields keep Fizz 1's `Player` names.
///
/// Mantle is out of scope. Fizz 1 starts a mantle automatically when Ben, airborne and not on the pogo, runs into a wall
/// whose lip is within reach; this port never does, so a level or scenario must keep Ben clear of such ledges. The
/// trace tool refuses scenarios that trigger it.
#[derive(Clone, Debug)]
pub struct Ben {
    pub b: Body,
    /// Facing: -1 or 1.
    pub face: f64,
    pub pogo: bool,
    /// Whether the current jump can still be cut short by releasing Jump.
    pub cut: bool,
    /// Seconds left of the shooting pose.
    pub shoot_t: f64,
    /// 0 while alive, otherwise seconds since death.
    pub dead: f64,
    /// Seconds of invulnerability left.
    pub inv: f64,
    /// Seconds left of the squash after a pogo bounce. It runs slightly below zero on the tick it expires and is never
    /// clamped, as in Fizz 1.
    pub squash: f64,
    /// Whether this rise has already bumped a ceiling.
    pub bumped: bool,
    /// Where the current rise began.
    pub rise_y: f64,
    /// Seconds before Ben may kick off a wall again. Wall kicks are not ported; the timer only counts down.
    pub wall_t: f64,
}

impl Ben {
    /// Ben at rest with the lower-left corner of his 0.7 by 1.4 body at `(x, y)`, facing right.
    pub fn new(x: f64, y: f64) -> Self {
        Ben {
            b: Body::new(x, y, 0.7, 1.4),
            face: 1.0,
            pogo: false,
            cut: false,
            shoot_t: 0.0,
            dead: 0.0,
            inv: 0.0,
            squash: 0.0,
            bumped: false,
            rise_y: 0.0,
            wall_t: 0.0,
        }
    }

    /// Advances Ben by one tick (Fizz 1's `tick_ben`, alive and dead branches).
    pub fn tick(&mut self, map: &TileMap, plats: &[Platform], input: InputFrame) {
        let dt = STEP;
        self.b.px = self.b.x;
        self.b.py = self.b.y;
        if self.dead > 0.0 {
            self.dead += dt;
            self.b.vy -= 30.0 * dt;
            self.b.x += self.b.vx * dt;
            self.b.y += self.b.vy * dt;
            return;
        }
        if self.inv > 0.0 {
            self.inv -= dt;
        }
        if self.shoot_t > 0.0 {
            self.shoot_t -= dt;
        }
        if self.wall_t > 0.0 {
            self.wall_t -= dt;
        }
        self.tick_move(map, plats, input, dt);
        self.tick_contacts();
    }

    /// Kills Ben as Fizz 1's `kill_by` does, minus the shake and the sound.
    pub fn kill(&mut self) {
        if self.dead > 0.0 {
            return;
        }
        self.dead = 0.001;
        self.b.vy = 14.0;
        self.b.vx = -self.face * 2.0;
        self.pogo = false;
    }

    /// Ben's own movement for one tick: input, jumps, pogo and physics (Fizz 1's `tick_ben_move`).
    fn tick_move(&mut self, map: &TileMap, plats: &[Platform], input: InputFrame, dt: f64) {
        let (h, e) = (input.held, input.pressed);
        if e.contains(Buttons::POGO) {
            self.pogo = !self.pogo;
        }
        let ax = f64::from(u8::from(h.contains(Buttons::RIGHT)))
            - f64::from(u8::from(h.contains(Buttons::LEFT)));
        let jump_held = h.contains(Buttons::JUMP);
        {
            if ax != 0.0 {
                self.face = ax;
            }
            let b = &mut self.b;
            if b.on_ground && !self.pogo {
                if ax != 0.0 {
                    b.vx += ax * if sign(b.vx) == -ax { 90.0 } else { 55.0 } * dt;
                } else {
                    let f = 60.0 * dt;
                    b.vx = if b.vx.abs() <= f {
                        0.0
                    } else {
                        b.vx - sign(b.vx) * f
                    };
                }
            } else {
                b.vx += ax * 32.0 * dt;
                if ax == 0.0 {
                    b.vx *= 1.0 - 1.5 * dt;
                }
            }
            b.vx = b.vx.clamp(-7.0, 7.0);
        }
        if self.b.on_ground {
            if self.pogo {
                self.b.vy = if jump_held {
                    libm::sqrt(2.0 * GRAVITY * POGO_HEIGHT)
                } else {
                    14.0
                };
                self.b.on_ground = false;
                self.squash = 0.12;
            } else if e.contains(Buttons::JUMP) {
                self.b.vy = 20.5;
                self.cut = true;
                self.b.on_ground = false;
            }
        }
        // Fizz 1 kicks off a wall here when Jump is pressed in the air against one; not ported.
        {
            if self.cut && !jump_held && self.b.vy > 0.0 {
                self.b.vy *= 0.45;
                self.cut = false;
            }
            if self.b.vy <= 0.0 {
                self.cut = false;
            }
            if self.squash > 0.0 {
                self.squash -= dt;
            }
            self.b.vy = (self.b.vy - GRAVITY * dt).max(-22.0);
        }
        // Fizz 1 fires a shot here on the Fire edge; the shots arrive with the Fizz Blaster.
        let rising = self.b.vy;
        // Only a rising pogo has the taller head: walking, falling and standing use the body alone.
        self.b.head = if self.pogo && rising > 0.0 {
            POGO_DRAWN_HEIGHT - self.b.h
        } else {
            0.0
        };
        self.b.phys(map, plats, dt);
        self.b.head = 0.0;
        if self.b.bonk && self.pogo {
            self.b.vy = 0.0;
        }
        self.tick_head_bump(rising);
        // Fizz 1's `tick_mantle` would start a mantle here; it is out of scope.
        self.b.vx = snap(self.b.vx);
        self.b.vy = snap(self.b.vy);
    }

    /// Tracks the rise so a ceiling bump counts once per rise (Fizz 1's `tick_head_bump`, without the sound).
    fn tick_head_bump(&mut self, rising: f64) {
        if self.b.vy <= 0.0 && !self.b.bonk {
            self.bumped = false;
        }
        if rising <= 0.0 || self.b.on_ground {
            self.rise_y = self.b.y;
        }
        if self.bumped
            || rising <= HEAD_BUMP_MIN_SPEED
            || !self.b.bonk
            || self.b.y - self.rise_y < HEAD_BUMP_MIN_RISE
        {
            return;
        }
        self.bumped = true;
    }

    /// The part of Fizz 1's `tick_ben_contacts` that involves only Ben: falling out of the map. Hazard tiles, doors,
    /// items and enemies belong to the level and the entities.
    fn tick_contacts(&mut self) {
        if self.b.y < FALL_DEATH_Y {
            self.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_threshold_is_two_to_the_minus_1000() {
        assert_eq!(SNAP, 9.332_636_185_032_189e-302);
        assert_eq!(snap(SNAP), SNAP);
        assert_eq!(snap(-SNAP / 2.0), 0.0);
        assert_eq!(snap(1e-308), 0.0);
        assert_eq!(snap(-0.5), -0.5);
    }

    #[test]
    fn an_idle_pogo_never_reaches_subnormal_velocities() {
        let mut map = TileMap::new(8, 4, fizz_phys::TileProps::default());
        map.props.set_flags(1, fizz_phys::tilemap::SOLID);
        for x in 0..8 {
            map.set(x, 0, 1);
        }
        let mut ben = Ben::new(3.0, 1.0);
        let mut prev = Buttons::NONE;
        for tick in 0..30_000 {
            let held = if tick == 0 {
                Buttons::POGO
            } else {
                Buttons::NONE
            };
            ben.tick(&map, &[], InputFrame::from_transition(prev, held));
            prev = held;
            for v in [ben.b.vx, ben.b.vy] {
                assert!(v == 0.0 || v.abs() >= SNAP, "tick {tick}: {v:e}");
            }
        }
    }
}
