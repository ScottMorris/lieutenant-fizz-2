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

// The chicken's body fits its sprite (the oblique projection puts a wall's front face 4 px left of its collision edge, `docs/OBLIQUE.md` "Registration"). The art is `cluckzarg_idle`, `_walk`,
// `_walk2` and `_alert`, each 24 by 24 with its anchor at column 12, row 24, and `cluckzarg_beak`, a 4 by 4 overlay that lands
// on columns 8 to 11. Read from the pixel data (`fizz-view`'s `sprites_data.rs`), every one of the four body frames is opaque
// in columns 0 to 23 and in rows 0 to 22, so each covers 12 px left of the anchor and 12 px right of it (the pixels
// `anchor - 12 ..= anchor + 11`) and reaches from 1 px to 24 px above the feet line. The view flips the art by mirroring those
// 24 columns inside the same box, so both facings cover the same pixels, and the beak lies inside them.
//
// The rule asks the body to contain the opaque pixels with 4 px to spare on its right, where a wall's front face is drawn 4 px
// left of its collision edge (the near plane, `docs/OBLIQUE.md` "Registration"), and nothing to spare on its left, where a
// wall is drawn at its collision edge. So the body runs from the sprite's left edge to 4 px past its right edge:
// `x - 12 .. x + 16` px about the anchor, 28 px or 1.75 tiles wide, and the body's left edge is the sprite's left edge. The
// body is not centred on the sprite; the view draws the sprite with its anchor [`CHICKEN_ANCHOR_X`] (12 px) right of the
// body's left edge, a constant that does not depend on the frame or the facing.
//
// The height is the sprite's too: the opaque rows end 24 px above the feet line, which is 1.5 tiles. Fizz 1's Gloop is 0.6
// tiles tall under art that is 0.6 tiles tall; this placeholder art is 24 px, and a body of 9.6 px under it let Ben pass
// through the chicken's drawn head unhurt. The ledge the slice chicken patrols has two tiles of headroom, so the taller body
// never meets a ceiling there. A stomp still needs Ben's feet over the top 45% of the body (`World`'s contact check), a shot
// still hits the body (its box already reaches `max(h, 1.1)` tiles up) and the stomp and stun puffs sit 0.4 tiles over the
// body's top.

/// The chicken's body width: its sprite's 24 px (1.5 tiles) plus the 4 px right margin of the oblique rule, 1.75 tiles.
pub const CHICKEN_W: f64 = 1.75;
/// The chicken's body height: the sprite's 24 px, 1.5 tiles.
pub const CHICKEN_H: f64 = 1.5;
/// How far the sprite's anchor column lies right of the body's left edge, in pixels. The sprite spans the body's left 24 px,
/// so a presenter draws it at `tile_px(body.x) + CHICKEN_ANCHOR_X` whichever frame and facing it shows.
pub const CHICKEN_ANCHOR_X: i32 = 12;
/// The same distance in tiles; a chicken spawns with its sprite centred on the spawn tile (`x + 0.5`).
const CHICKEN_ANCHOR: f64 = CHICKEN_ANCHOR_X as f64 / 16.0;

/// A chicken's body: its size, and how far its spawn centre lies from its left edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub w: f64,
    pub h: f64,
    /// Distance from the body's left edge to the point that sits on `x + 0.5` at spawn.
    pub centre: f64,
}

impl Shape {
    /// The slice's chicken, fitted to its sprite.
    pub const SPRITE: Shape = Shape {
        w: CHICKEN_W,
        h: CHICKEN_H,
        centre: CHICKEN_ANCHOR,
    };
    /// Fizz 1's Gloop (`ents.rs` `info(Kind::Gloop)`: 0.9 by 0.6, centred on its spawn tile). The Fizz 1 trace tests build their
    /// chickens with it, so the proof that the port matches Fizz 1 bit for bit keeps its meaning.
    pub const FIZZ1: Shape = Shape {
        w: 0.9,
        h: 0.6,
        centre: 0.45,
    };
}

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
    /// A chicken as Fizz 1's level builder spawns one at `(x, y)`: the sprite is centred on `x + 0.5` (Fizz 1 centres the body there) and the body stands on `y`.
    ///
    /// `id` names the chicken's cluck stream and `tick` is the world tick it spawns on; the first cluck timer is the
    /// stream's first draw.
    pub fn new(x: f64, y: f64, dir: f64, id: u32, tick: u64) -> Self {
        Self::with_shape(x, y, dir, id, tick, Shape::SPRITE)
    }

    /// [`new`](Self::new) with an explicit body. [`Shape::FIZZ1`] gives Fizz 1's own Gloop, which the trace tests use.
    pub fn with_shape(x: f64, y: f64, dir: f64, id: u32, tick: u64, shape: Shape) -> Self {
        let mut c = Chicken {
            b: Body::new(x - shape.centre + 0.5, y, shape.w, shape.h),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level;
    use crate::tiles::{BLOCK, FILL, props};

    /// A hall nine tiles wide: walls at columns 0 and 10, a floor under row 1, and nothing else.
    fn hall() -> TileMap {
        let mut m = TileMap::new(11, 6, props());
        for x in 0..11 {
            m.set(x, 0, FILL);
        }
        for y in 1..6 {
            m.set(0, y, BLOCK);
            m.set(10, y, BLOCK);
        }
        m
    }

    #[test]
    fn the_body_is_the_sprites_24_px_plus_4_px_on_the_right() {
        assert_eq!(CHICKEN_W * 16.0, 28.0);
        assert_eq!(CHICKEN_H * 16.0, 24.0);
        assert_eq!(f64::from(CHICKEN_ANCHOR_X) / 16.0, 0.75);
        // The sprite spans the body's left 24 px, so 4 px of the 28 are the margin.
        assert_eq!(CHICKEN_W * 16.0 - 24.0, 4.0);
    }

    #[test]
    fn a_chicken_spawns_with_its_sprite_centred_on_the_spawn_tile() {
        let c = Chicken::new(8.0, 11.0, -1.0, 0, 0);
        assert_eq!(c.b.x + CHICKEN_ANCHOR, 8.5);
        assert_eq!((c.b.w, c.b.h), (CHICKEN_W, CHICKEN_H));
        let g = Chicken::with_shape(8.0, 11.0, -1.0, 0, 0, Shape::FIZZ1);
        assert_eq!((g.b.x, g.b.w, g.b.h), (8.05, 0.9, 0.6));
    }

    #[test]
    fn the_patrol_turns_at_a_wall_on_its_right_with_the_body_against_it() {
        let m = hall();
        let mut c = Chicken::new(5.0, 1.0, 1.0, 0, 0);
        for t in 1..=400 {
            c.tick(&m, t);
            if c.dir < 0.0 {
                break;
            }
        }
        assert_eq!(c.dir, -1.0);
        // The body's right edge is on the wall's collision edge (column 10) and the chicken now walks left.
        assert!((c.b.x + c.b.w - 10.0).abs() < 1e-5, "{}", c.b.x + c.b.w);
    }

    #[test]
    fn the_patrol_turns_at_a_wall_on_its_left_with_the_body_against_it() {
        let m = hall();
        let mut c = Chicken::new(5.0, 1.0, -1.0, 0, 0);
        for t in 1..=400 {
            c.tick(&m, t);
            if c.dir > 0.0 {
                break;
            }
        }
        assert_eq!(c.dir, 1.0);
        assert_eq!(c.b.x, 1.0, "the body's left edge is the wall's edge");
    }

    #[test]
    fn a_chicken_in_a_hall_walks_from_wall_to_wall_and_never_sticks() {
        let m = hall();
        let mut c = Chicken::new(3.0, 1.0, 1.0, 0, 0);
        let (mut lo, mut hi, mut turns) = (f64::MAX, f64::MIN, 0);
        let mut dir = c.dir;
        for t in 1..=3000 {
            c.tick(&m, t);
            lo = lo.min(c.b.x);
            hi = hi.max(c.b.x + c.b.w);
            if c.dir != dir {
                turns += 1;
                dir = c.dir;
            }
        }
        assert!(lo >= 1.0 - 1e-9 && hi <= 10.0, "{lo} {hi}");
        // 8.25 tiles of walking at 1.5 tiles a second is 330 ticks a lap.
        assert!(turns >= 8, "{turns}");
    }

    #[test]
    fn the_slice_chicken_patrols_its_ledge_from_the_edge_to_the_wall() {
        let w = level::world();
        let m = w.map.clone();
        let mut c = w.chickens[0].clone();
        let (mut lo, mut hi, mut turns) = (f64::MAX, f64::MIN, 0);
        let mut dir = c.dir;
        for t in 1..=3000 {
            c.tick(&m, t);
            assert!(
                c.b.on_ground && c.b.y == 11.0,
                "stays on the ledge at tick {t}"
            );
            lo = lo.min(c.b.x);
            hi = hi.max(c.b.x + c.b.w);
            if c.dir != dir {
                turns += 1;
                dir = c.dir;
            }
        }
        // The ledge's planks run from x = 2 to x = 15, where the wall begins. The body turns with its left edge on the
        // ledge's end (one step past it at most) and its right edge against the wall.
        assert!((2.0 - 0.05..2.05).contains(&lo), "{lo}");
        assert!(hi <= 15.0 && hi > 15.0 - 0.05, "{hi}");
        // 11.2 tiles of ledge at 1.5 tiles a second is 448 ticks a side: 6 or 7 turns in 3000.
        assert!(turns >= 6, "{turns}");
    }
}
