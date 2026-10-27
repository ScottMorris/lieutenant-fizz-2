// The chicken: Fizz 1's Gloop, patrolling and stunnable, ported line for line.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: Fizz 1 `episodes/episode-1/game/src/world.rs` at commit `4bce036`: `init_ent` (the spawn position),
// `tick_ents`, `ai` (the stunned branch and `Kind::Gloop`) and `ground_ahead`; sizes from `ents.rs` `info(Kind::Gloop)`.
// Left out on purpose: the "stray" reset that sends an enemy home after three seconds on another floor (the slice's
// chicken cannot leave its floor), the animation clock and the hidden-room dormancy. The art is a placeholder, so the
// Gloop is called a chicken here. The cluck timer is new in Fizz 2 (Fizz 1's Gloop is silent); see `Chicken::cluck_timer`.

use crate::tiles::{SPIKE, is_liquid};
#[cfg_attr(test, allow(unused_imports))]
use fizz_phys::fmath::FloatMath;
use fizz_phys::rng::hashf;
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

/// How many cluck variants the chicken has. A presenter maps `Event::Cluck { variant }` with `variant < CLUCK_VARIANTS` to
/// one sound.
pub const CLUCK_VARIANTS: u8 = 4;
/// The shortest wait between clucks, in ticks (1.5 s).
pub const CLUCK_MIN_TICKS: u32 = 90;
/// The longest wait between clucks, in ticks (6 s).
pub const CLUCK_MAX_TICKS: u32 = 360;

/// The value of [`Chicken::cluck_age`] before the chicken's first cluck, and the value it saturates at.
pub const CLUCK_NEVER: u32 = u32::MAX;

/// Separates the cluck stream from any other use of `hashf`, so the same tick, id and counter never collide with another
/// stream added later.
const CLUCK_SALT: i32 = 0x0c1c_0c1c;

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
    /// Which chicken this is: its index in the world's list. It names the chicken's cluck stream and is not hashed (the
    /// index fixes it).
    id: u32,
    /// Ticks until the next cluck, in [`CLUCK_MIN_TICKS`]`..=`[`CLUCK_MAX_TICKS`]. It counts down one per tick while the
    /// chicken is alive and not stunned, and holds still while it is stunned: a stun delays the next cluck by its length
    /// instead of letting the cluck fall due the moment the chicken wakes or, worse, during the stun.
    pub cluck_timer: u32,
    /// How many draws the chicken's cluck stream has made. Every draw takes the next number, so the counter is the
    /// stream's position and is hashed with the timer.
    pub cluck_draws: u32,
    /// Presentation state: ticks since this chicken last clucked, 0 on the tick of the cluck, [`CLUCK_NEVER`] before the
    /// first one. [`World::step`](crate::world::World::step) adds one every tick (saturating) and resets it on a cluck.
    /// It is not hashed: it only tells a presenter how far into a cluck the chicken is, and the hashed cluck timer that
    /// sets it would diverge first. It is not read by any rule.
    pub cluck_age: u32,
    /// Presentation state: the variant of the chicken's last cluck, below [`CLUCK_VARIANTS`]; 0 while
    /// [`cluck_age`](Self::cluck_age) is [`CLUCK_NEVER`]. Not hashed, for the same reason as `cluck_age`.
    pub cluck_variant: u8,
}

impl Chicken {
    /// A chicken as Fizz 1's level builder spawns one at `(x, y)`: the body is centred on `x + 0.5` and stands on `y`.
    ///
    /// `id` names the chicken's cluck stream and `tick` is the world tick it spawns on; the first cluck timer is the
    /// stream's first draw.
    pub fn new(x: f64, y: f64, dir: f64, id: u32, tick: u64) -> Self {
        let mut c = Chicken {
            b: Body::new(x - CHICKEN_W / 2.0 + 0.5, y, CHICKEN_W, CHICKEN_H),
            dir,
            stun: 0.0,
            touch: false,
            dead: false,
            id,
            cluck_timer: 0,
            cluck_draws: 0,
            cluck_age: CLUCK_NEVER,
            cluck_variant: 0,
        };
        c.cluck_timer = c.draw_timer(tick);
        c
    }

    /// The next number in this chicken's cluck stream, in [0, 1): Fizz 1's `hashf` over the tick, the chicken's id and
    /// the number of draws so far. It is a pure function of those three integers, so no platform random source and no
    /// floating-point operation beyond `hashf`'s own enters the rules, and the chicken's AI draws nothing, so Fizz 1's
    /// movement is untouched.
    fn draw(&mut self, tick: u64) -> f64 {
        let x = (tick as u32).wrapping_add(self.id.wrapping_mul(0x9e37_79b1)) as i32;
        let y = (self.cluck_draws as i32) ^ CLUCK_SALT;
        self.cluck_draws += 1;
        hashf(x, y)
    }

    /// Draws a wait, uniform over [`CLUCK_MIN_TICKS`]`..=`[`CLUCK_MAX_TICKS`].
    fn draw_timer(&mut self, tick: u64) -> u32 {
        let span = CLUCK_MAX_TICKS - CLUCK_MIN_TICKS + 1;
        CLUCK_MIN_TICKS + (self.draw(tick) * f64::from(span)) as u32
    }

    /// Draws a variant, uniform over `0..`[`CLUCK_VARIANTS`].
    fn draw_variant(&mut self, tick: u64) -> u8 {
        (self.draw(tick) * f64::from(CLUCK_VARIANTS)) as u8
    }

    /// Advances the chicken by one tick (Fizz 1's `tick_ents` and `ai` for a Gloop), then runs the cluck timer. `tick` is
    /// the world tick being run. Returns the variant of a cluck that falls due on this tick.
    ///
    /// A dead or stunned chicken returns before the timer: it neither clucks nor counts down. When the timer reaches zero
    /// the variant is the next draw, then the timer is redrawn.
    pub fn tick(&mut self, map: &TileMap, tick: u64) -> Option<u8> {
        let dt = STEP;
        self.b.px = self.b.x;
        self.b.py = self.b.y;
        if self.dead {
            return None;
        }
        if self.stun > 0.0 {
            self.stun -= dt;
            self.b.vx = 0.0;
            self.b.fall(dt, CHICKEN_TERMINAL);
            self.b.phys(map, &[], dt);
            return None;
        }
        self.b.vx = self.dir * CHICKEN_SPEED;
        self.b.fall(dt, CHICKEN_TERMINAL);
        self.b.phys(map, &[], dt);
        if self.b.hit_x || (self.b.on_ground && !self.ground_ahead(map)) {
            self.dir *= -1.0;
        }
        self.cluck_timer -= 1;
        if self.cluck_timer > 0 {
            return None;
        }
        let variant = self.draw_variant(tick);
        self.cluck_timer = self.draw_timer(tick);
        Some(variant)
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
