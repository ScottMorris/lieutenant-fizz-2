// The chicken: Fizz 1's Gloop, patrolling and stunnable, ported line for line.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: Fizz 1 `episodes/episode-1/game/src/world.rs` at commit `4bce036`: `init_ent` (the spawn position),
// `tick_ents`, `ai` (the stunned branch and `Kind::Gloop`) and `ground_ahead`; sizes from `ents.rs` `info(Kind::Gloop)`.
// Left out on purpose: the "stray" reset that sends an enemy home after three seconds on another floor (the slice's
// chicken cannot leave its floor), the animation clock and the hidden-room dormancy. The art is a placeholder, so the
// Gloop is called a chicken here; its cluck timer arrives with the sound package.

use crate::tiles::{SPIKE, is_liquid};
#[cfg_attr(test, allow(unused_imports))]
use fizz_phys::fmath::FloatMath;
use fizz_phys::{Body, STEP, TileMap};

/// The chicken's body width.
pub const CHICKEN_W: f64 = 0.9;
/// The chicken's body height.
pub const CHICKEN_H: f64 = 0.6;
/// How fast the chicken walks, in tiles per second.
pub const CHICKEN_SPEED: f64 = 1.5;
/// The fastest the chicken falls, in tiles per second.
pub const CHICKEN_TERMINAL: f64 = 20.0;
/// Seconds a stomp stuns the chicken.
pub const STOMP_STUN: f64 = 3.0;
/// Seconds a Fizz Blaster shot stuns the chicken.
pub const SHOT_STUN: f64 = 6.0;

/// One chicken. Fizz 1 calls the same thing a Gloop; its fields keep Fizz 1's `Ent` names.
#[derive(Clone, Debug)]
pub struct Chicken {
    pub b: Body,
    /// Walking direction: -1 or 1.
    pub dir: f64,
    /// Seconds of stun left. A stunned chicken stands still and is harmless.
    pub stun: f64,
    /// Whether Ben overlapped the chicken on the last contact check.
    pub touch: bool,
    /// Fizz 1 never sets this for a Gloop; the flag stays so the hash and the trace keep Fizz 1's shape.
    pub dead: bool,
}

impl Chicken {
    /// A chicken as Fizz 1's level builder spawns one at `(x, y)`: the body is centred on `x + 0.5` and stands on `y`.
    pub fn new(x: f64, y: f64, dir: f64) -> Self {
        Chicken {
            b: Body::new(x - CHICKEN_W / 2.0 + 0.5, y, CHICKEN_W, CHICKEN_H),
            dir,
            stun: 0.0,
            touch: false,
            dead: false,
        }
    }

    /// Advances the chicken by one tick (Fizz 1's `tick_ents` and `ai` for a Gloop).
    pub fn tick(&mut self, map: &TileMap) {
        let dt = STEP;
        self.b.px = self.b.x;
        self.b.py = self.b.y;
        if self.dead {
            return;
        }
        if self.stun > 0.0 {
            self.stun -= dt;
            self.b.vx = 0.0;
            self.b.fall(dt, CHICKEN_TERMINAL);
            self.b.phys(map, &[], dt);
            return;
        }
        self.b.vx = self.dir * CHICKEN_SPEED;
        self.b.fall(dt, CHICKEN_TERMINAL);
        self.b.phys(map, &[], dt);
        if self.b.hit_x || (self.b.on_ground && !self.ground_ahead(map)) {
            self.dir *= -1.0;
        }
    }

    /// Whether there is footing in front of the chicken (Fizz 1's `ground_ahead`): a floor or a slope, and no spikes or
    /// liquid.
    fn ground_ahead(&self, map: &TileMap) -> bool {
        let fx = if self.dir > 0.0 {
            self.b.x + self.b.w + 0.05
        } else {
            self.b.x - 0.05
        };
        let cx = fx.floor() as i32;
        let cy = (self.b.y - 0.1).floor() as i32;
        let t = map.get(cx, cy);
        let t2 = map.get(cx, self.b.y.floor() as i32);
        if t == SPIKE || is_liquid(t) || t2 == SPIKE {
            return false;
        }
        map.solid(cx, cy, true, self.b.y) || map.is_slope(t) || map.is_slope(t2)
    }

    /// The flags byte the state hash uses: on the ground, in contact with Ben, dead.
    pub fn flags(&self) -> u8 {
        u8::from(self.b.on_ground) | u8::from(self.touch) << 1 | u8::from(self.dead) << 2
    }
}
