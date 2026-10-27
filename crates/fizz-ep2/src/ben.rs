// Ben's movement: walk, jump, pogo and physics, ported line for line from Fizz 1's `tick_ben_move`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: ported from Fizz 1 `episodes/episode-1/game/src/world.rs` at commit `4bce036`: `Player::level`,
// `tick_ben` (the alive and dead branches), `tick_ben_move`, `tick_head_bump` and `fire`.
// `fire`. Left out on purpose: mantle (automatic and in progress), ladders and vines, wall kicks, moving platforms under
// Ben, jokes and counters, captions and sound, and the camera. What Fizz 1's `tick_ben_contacts` does with tiles, items
// and enemies lives in `world.rs`. Departure: velocities below 2^-1000 snap to zero.

use crate::shot::Shot;
use alloc::vec::Vec;
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
/// Ben dies when his body's bottom is below this height: he has fallen out of the map. The world applies it.
pub const FALL_DEATH_Y: f64 = -1.5;
/// The speed of a Fizz Blaster shot in tiles per second.
pub const SHOT_SPEED: f64 = 16.0;
/// Seconds a shot flies before it fizzles.
pub const SHOT_LIFE: f64 = 1.1;
/// Seconds of the shooting pose after a shot.
pub const SHOOT_POSE: f64 = 0.25;
/// Seconds Up or Down must be held before the camera starts to look (Fizz 1's `look_up > 0.2` test).
pub const LOOK_DELAY: f64 = 0.2;
/// Seconds the look takes to rise from nothing to its full reach once the delay has passed. Fizz 1's camera chases its aim
/// point exponentially at 8 per second, which covers 95% of the distance in `ln(20) / 8` = 0.37 s; a straight ramp of 0.35 s
/// covers the same distance in about the same time without an `exp`.
pub const LOOK_RAMP: f64 = 0.35;
/// How far the view looks up or down in tiles, measured from the middle of Ben's body (Fizz 1's 3.6, which it measures from
/// his feet).
pub const LOOK_REACH: f64 = 3.6;
/// How fast the look offset may change in tiles per second. It is a little above the ramp's own speed (`LOOK_REACH /
/// LOOK_RAMP` = 10.3), so while a button is held the offset equals the ramp, and after a release it returns to nothing in
/// 0.3 s instead of snapping back when the timer resets.
pub const LOOK_RATE: f64 = 12.0;
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
    /// Seconds Up has been held alone while Ben is idle (Fizz 1's `Player::look_up`). Zero otherwise.
    pub look_up: f64,
    /// Seconds Down has been held alone while Ben is idle (Fizz 1's `Player::look_down`). Zero otherwise.
    pub look_down: f64,
    /// The direction of Ben's last shot: 1 up, -1 down, 0 level (Fizz 1's `Player::aim`). It keeps its value until the next
    /// shot, and the shooting pose reads it while `shoot_t` runs.
    pub aim: i8,
    /// How far the view is looking, in tiles, positive up: the stateless camera's memory. It follows [`Ben::look_target`] at
    /// no more than [`LOOK_RATE`] tiles a second. Fizz 1 has no such field; it keeps the same memory in its camera position.
    pub look_offset: f64,
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
            look_up: 0.0,
            look_down: 0.0,
            aim: 0,
            look_offset: 0.0,
        }
    }

    /// Where the look offset is heading: nothing until a look timer passes [`LOOK_DELAY`], then a straight ramp over
    /// [`LOOK_RAMP`] seconds to [`LOOK_REACH`] tiles up or down. It is a function of the timers alone.
    pub fn look_target(&self) -> f64 {
        let (held, sign) = if self.look_up > 0.0 {
            (self.look_up, 1.0)
        } else {
            (self.look_down, -1.0)
        };
        let share = ((held - LOOK_DELAY) / LOOK_RAMP).clamp(0.0, 1.0);
        if share == 0.0 {
            0.0
        } else {
            sign * LOOK_REACH * share
        }
    }

    /// Advances Ben by one tick (Fizz 1's `tick_ben`, alive and dead branches). A shot Ben fires is pushed onto `shots`
    /// and costs one of `ammo`.
    pub fn tick(
        &mut self,
        map: &TileMap,
        plats: &[Platform],
        input: InputFrame,
        ammo: &mut u32,
        shots: &mut Vec<Shot>,
    ) {
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
        self.tick_move(map, plats, input, ammo, shots, dt);
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
    fn tick_move(
        &mut self,
        map: &TileMap,
        plats: &[Platform],
        input: InputFrame,
        ammo: &mut u32,
        shots: &mut Vec<Shot>,
        dt: f64,
    ) {
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
            let idle = self.b.on_ground && !self.pogo && ax == 0.0;
            self.look_down = if h.contains(Buttons::DOWN) && !h.contains(Buttons::UP) && idle {
                self.look_down + dt
            } else {
                0.0
            };
            self.look_up = if h.contains(Buttons::UP) && !h.contains(Buttons::DOWN) && idle {
                self.look_up + dt
            } else {
                0.0
            };
            let (target, step) = (self.look_target(), LOOK_RATE * dt);
            let gap = target - self.look_offset;
            self.look_offset = if gap.abs() <= step {
                target
            } else if gap > 0.0 {
                self.look_offset + step
            } else {
                self.look_offset - step
            };
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
        if e.contains(Buttons::FIRE) {
            self.fire(h, ammo, shots);
        }
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

    /// Fires the Fizz Blaster (Fizz 1's `fire`). With no ammo nothing happens. Up fires straight up; Down fires straight
    /// down while Ben is in the air; otherwise the shot leaves from the front of his body, level.
    fn fire(&mut self, h: Buttons, ammo: &mut u32, shots: &mut Vec<Shot>) {
        if *ammo == 0 {
            return;
        }
        *ammo -= 1;
        let b = &self.b;
        let (mut vx, mut vy) = (self.face * SHOT_SPEED, 0.0);
        let (mut sx, mut sy) = (b.x + b.w / 2.0 + self.face * 0.6, b.y + 0.85);
        let mut aim = 0;
        if h.contains(Buttons::UP) {
            (vx, vy, sx, sy) = (0.0, SHOT_SPEED, b.x + b.w / 2.0, b.y + 1.5);
            aim = 1;
        } else if h.contains(Buttons::DOWN) && !b.on_ground {
            (vx, vy, sx, sy) = (0.0, -SHOT_SPEED, b.x + b.w / 2.0, b.y);
            aim = -1;
        }
        self.aim = aim;
        shots.push(Shot {
            x: sx,
            y: sy,
            vx,
            vy,
            life: SHOT_LIFE,
        });
        self.shoot_t = SHOOT_POSE;
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

    /// A flat floor of solid tiles under row 1, a ceiling-free arena and Ben standing on it.
    fn arena() -> (TileMap, Ben) {
        let mut map = TileMap::new(24, 14, fizz_phys::TileProps::default());
        map.props.set_flags(1, fizz_phys::tilemap::SOLID);
        for x in 0..24 {
            map.set(x, 0, 1);
            map.set(x, 1, 1);
        }
        (map, Ben::new(10.0, 2.0))
    }

    /// Holds `held` for `ticks` ticks (the first has the press edges), returning the shots fired.
    fn hold(
        map: &TileMap,
        ben: &mut Ben,
        prev: &mut Buttons,
        held: Buttons,
        ticks: u32,
        ammo: &mut u32,
    ) -> Vec<Shot> {
        let mut shots = Vec::new();
        for _ in 0..ticks {
            ben.tick(
                map,
                &[],
                InputFrame::from_transition(*prev, held),
                ammo,
                &mut shots,
            );
            *prev = held;
        }
        shots
    }

    #[test]
    fn a_look_timer_starts_while_idle_and_resets_on_release() {
        let (map, mut ben) = arena();
        let (mut prev, mut ammo) = (Buttons::NONE, 5);
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 5, &mut ammo);
        hold(&map, &mut ben, &mut prev, Buttons::UP, 60, &mut ammo);
        assert!((ben.look_up - 1.0).abs() < 1e-9 && ben.look_down == 0.0);
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 1, &mut ammo);
        assert_eq!(ben.look_up, 0.0);
        hold(&map, &mut ben, &mut prev, Buttons::DOWN, 30, &mut ammo);
        assert!((ben.look_down - 0.5).abs() < 1e-9 && ben.look_up == 0.0);
    }

    #[test]
    fn a_look_does_not_start_when_moving_airborne_on_the_pogo_or_with_both_held() {
        let (map, mut ben) = arena();
        let (mut prev, mut ammo) = (Buttons::NONE, 5);
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 5, &mut ammo);
        hold(
            &map,
            &mut ben,
            &mut prev,
            Buttons::UP.union(Buttons::DOWN),
            20,
            &mut ammo,
        );
        assert_eq!((ben.look_up, ben.look_down), (0.0, 0.0));
        hold(
            &map,
            &mut ben,
            &mut prev,
            Buttons::UP.union(Buttons::RIGHT),
            20,
            &mut ammo,
        );
        assert_eq!(ben.look_up, 0.0);
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 40, &mut ammo);
        hold(
            &map,
            &mut ben,
            &mut prev,
            Buttons::UP.union(Buttons::JUMP),
            1,
            &mut ammo,
        );
        assert!(!ben.b.on_ground);
        hold(&map, &mut ben, &mut prev, Buttons::UP, 10, &mut ammo);
        assert_eq!(ben.look_up, 0.0, "airborne");
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 60, &mut ammo);
        assert!(ben.b.on_ground);
        hold(&map, &mut ben, &mut prev, Buttons::POGO, 1, &mut ammo);
        hold(&map, &mut ben, &mut prev, Buttons::DOWN, 20, &mut ammo);
        assert!(ben.pogo);
        assert_eq!(ben.look_down, 0.0, "on the pogo");
    }

    #[test]
    fn up_fires_straight_up_and_down_fires_straight_down_only_in_the_air() {
        let (map, mut ben) = arena();
        let (mut prev, mut ammo) = (Buttons::NONE, 5);
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 5, &mut ammo);
        let (cx, y) = (ben.b.x + ben.b.w / 2.0, ben.b.y);
        let s = hold(&map, &mut ben, &mut prev, Buttons::FIRE, 1, &mut ammo);
        assert_eq!(
            (s[0].vx, s[0].vy, s[0].x, s[0].y),
            (16.0, 0.0, cx + 0.6, y + 0.85)
        );
        assert_eq!(ben.aim, 0);
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 1, &mut ammo);
        let s = hold(
            &map,
            &mut ben,
            &mut prev,
            Buttons::UP.union(Buttons::FIRE),
            1,
            &mut ammo,
        );
        assert_eq!((s[0].vx, s[0].vy, s[0].x, s[0].y), (0.0, 16.0, cx, y + 1.5));
        assert_eq!(ben.aim, 1);
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 1, &mut ammo);
        let s = hold(
            &map,
            &mut ben,
            &mut prev,
            Buttons::DOWN.union(Buttons::FIRE),
            1,
            &mut ammo,
        );
        assert_eq!(
            (s[0].vx, s[0].vy),
            (16.0, 0.0),
            "on the ground Down stays level"
        );
        assert_eq!(ben.aim, 0);
        hold(&map, &mut ben, &mut prev, Buttons::JUMP, 1, &mut ammo);
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 5, &mut ammo);
        let (y, vy) = (ben.b.y, ben.b.vy);
        assert!(!ben.b.on_ground && vy != 0.0);
        let s = hold(
            &map,
            &mut ben,
            &mut prev,
            Buttons::DOWN.union(Buttons::FIRE),
            1,
            &mut ammo,
        );
        assert_eq!((s[0].vx, s[0].vy, s[0].x, s[0].y), (0.0, -16.0, cx, y));
        assert_eq!(ben.aim, -1);
        assert_eq!(ammo, 1);
    }

    #[test]
    fn a_shot_with_no_ammo_leaves_the_aim_alone() {
        let (map, mut ben) = arena();
        let (mut prev, mut ammo) = (Buttons::NONE, 0);
        ben.aim = 1;
        hold(
            &map,
            &mut ben,
            &mut prev,
            Buttons::DOWN.union(Buttons::FIRE),
            1,
            &mut ammo,
        );
        assert_eq!(ben.aim, 1);
    }

    #[test]
    fn the_look_target_ramps_after_the_delay_and_the_offset_follows_and_returns() {
        let mut ben = Ben::new(0.0, 0.0);
        let mut last = 0.0;
        for n in 0..90 {
            ben.look_up = f64::from(n) * STEP;
            let t = ben.look_target();
            assert!(t >= last, "monotone");
            if ben.look_up <= LOOK_DELAY {
                assert_eq!(t, 0.0);
            }
            last = t;
        }
        assert_eq!(last, LOOK_REACH);
        ben.look_up = 0.0;
        ben.look_down = LOOK_DELAY + LOOK_RAMP / 2.0;
        assert!(
            (ben.look_target() + LOOK_REACH / 2.0).abs() < 1e-12,
            "down is negative"
        );
        ben.look_down = 0.0;
        assert_eq!(ben.look_target(), 0.0);
        assert!(ben.look_target().is_sign_positive());
    }

    #[test]
    fn the_look_offset_rises_to_the_reach_and_returns_to_nothing_after_release() {
        let (map, mut ben) = arena();
        let (mut prev, mut ammo) = (Buttons::NONE, 5);
        hold(&map, &mut ben, &mut prev, Buttons::NONE, 5, &mut ammo);
        let mut last = 0.0;
        for _ in 0..60 {
            hold(&map, &mut ben, &mut prev, Buttons::UP, 1, &mut ammo);
            assert!(ben.look_offset >= last && ben.look_offset <= LOOK_REACH);
            last = ben.look_offset;
        }
        assert_eq!(ben.look_offset, LOOK_REACH);
        let mut ticks = 0;
        while ben.look_offset != 0.0 {
            let before = ben.look_offset;
            hold(&map, &mut ben, &mut prev, Buttons::NONE, 1, &mut ammo);
            assert!(
                ben.look_offset < before && ben.look_offset >= 0.0,
                "monotone return"
            );
            ticks += 1;
        }
        assert!((17..=19).contains(&ticks), "about 0.3 s, took {ticks}");
        for _ in 0..60 {
            hold(&map, &mut ben, &mut prev, Buttons::DOWN, 1, &mut ammo);
        }
        assert_eq!(ben.look_offset, -LOOK_REACH);
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
            ben.tick(
                &map,
                &[],
                InputFrame::from_transition(prev, held),
                &mut 0,
                &mut Vec::new(),
            );
            prev = held;
            for v in [ben.b.vx, ben.b.vy] {
                assert!(v == 0.0 || v.abs() >= SNAP, "tick {tick}: {v:e}");
            }
        }
    }
}
