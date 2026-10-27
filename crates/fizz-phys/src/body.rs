// Axis-aligned body that resolves X then Y movement against tiles, slopes and platforms.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: Derived from Fizz 1 `crates/sim/src/body.rs` at commit `4bce036`; transformed as in fizz-dos `m2/patches/sim.patch`.
// Edits: `use crate::fmath::FloatMath;` added (unused in test builds, where `std` supplies `floor`); imports ordered by rustfmt for the 2024 edition.

#[cfg_attr(test, allow(unused_imports))]
use crate::fmath::FloatMath;

use crate::GRAVITY;
use crate::platform::Platform;
use crate::tilemap::{TileMap, slope_height};

/// Maximum ledge height a grounded body steps up without jumping (tiles).
pub const STEP_UP: f64 = 0.55;

/// An axis-aligned body. `(x, y)` is the bottom-left corner; y grows upwards.
#[derive(Clone, Debug, Default)]
pub struct Body {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub vx: f64,
    pub vy: f64,
    /// Position at the start of the tick, for render interpolation.
    pub px: f64,
    pub py: f64,
    pub on_ground: bool,
    /// Tile id of the slope the body stands on, or 0.
    pub on_slope: u8,
    /// Index of the platform the body rides, if any.
    pub on_plat: Option<usize>,
    pub hit_x: bool,
    pub bonk: bool,
    /// Extra reach above the body's top that only the ceiling check sees (a pogo stick's drawn head);
    /// 0 for everything else. The body is never pushed down by it.
    pub head: f64,
}

impl Body {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Body {
            x,
            y,
            w,
            h,
            px: x,
            py: y,
            ..Default::default()
        }
    }

    pub fn centre_x(&self) -> f64 {
        self.x + self.w / 2.0
    }

    pub fn overlaps(&self, bx: f64, by: f64, bw: f64, bh: f64) -> bool {
        self.x < bx + bw && self.x + self.w > bx && self.y < by + bh && self.y + self.h > by
    }

    /// Applies gravity, clamped to `terminal` downward speed.
    pub fn fall(&mut self, dt: f64, terminal: f64) {
        self.vy = (self.vy - GRAVITY * dt).max(-terminal);
    }

    /// Horizontal move with wall resolution. A grounded body steps up ledges of at most
    /// [`STEP_UP`] when the cell above the ledge is free. Returns true on a wall hit.
    pub fn move_x(&mut self, map: &TileMap, dx: f64, grounded: bool) -> bool {
        self.x += dx;
        let y0 = (self.y + 1e-4).floor() as i32;
        let y1 = (self.y + self.h - 1e-4).floor() as i32;
        let blocked = |cx: i32, y: f64| -> bool {
            for cy in y0..=y1 {
                if !map.solid(cx, cy, false, 0.0) {
                    continue;
                }
                if cy == y0
                    && grounded
                    && f64::from(cy) + 1.0 - y <= STEP_UP
                    && !map.solid(cx, cy + 1, false, 0.0)
                {
                    continue;
                }
                return true;
            }
            false
        };
        if dx > 0.0 {
            let cx = (self.x + self.w).floor() as i32;
            if blocked(cx, self.y) {
                self.x = f64::from(cx) - self.w - 1e-6;
                return true;
            }
        } else if dx < 0.0 {
            let cx = self.x.floor() as i32;
            if blocked(cx, self.y) {
                self.x = f64::from(cx) + 1.0;
                return true;
            }
        }
        false
    }

    /// Vertical move. Returns true when landing on something while moving down; sets
    /// `bonk` when hitting a ceiling while moving up.
    pub fn move_y(&mut self, map: &TileMap, dy: f64) -> bool {
        let pb = self.y;
        self.y += dy;
        self.bonk = false;
        let x0 = (self.x + 1e-4).floor() as i32;
        let x1 = (self.x + self.w - 1e-4).floor() as i32;
        let fx = (self.x + self.w / 2.0).floor() as i32;
        if dy < 0.0 {
            let cy = self.y.floor() as i32;
            let cs = map.is_slope(map.get(fx, cy)) || map.is_slope(map.get(fx, cy + 1));
            for cx in x0..=x1 {
                if cs && cx != fx {
                    continue;
                }
                if map.solid(cx, cy, true, pb) {
                    self.y = f64::from(cy) + 1.0;
                    self.vy = 0.0;
                    return true;
                }
            }
        } else if dy > 0.0 {
            let cy = (self.y + self.h + self.head).floor() as i32;
            for cx in x0..=x1 {
                if map.solid(cx, cy, false, 0.0) {
                    // Stop with the head (plus any `head` reach) under the tile, never lowering the body.
                    self.y = (f64::from(cy) - self.h - self.head - 1e-6).max(pb);
                    self.vy = 0.0;
                    self.bonk = true;
                    break;
                }
            }
        }
        false
    }

    /// Speed (tiles per second) the tile underfoot carries the body at; 0 unless grounded on a
    /// conveyor.
    pub fn ground_drift(&self, map: &TileMap) -> f64 {
        if !self.on_ground {
            return 0.0;
        }
        map.drift_at(
            self.centre_x().floor() as i32,
            (self.y - 0.05).floor() as i32,
        )
    }

    /// Whether the body's centre column overlaps a climbable tile at any height of its body.
    pub fn on_ladder(&self, map: &TileMap) -> bool {
        let cx = self.centre_x().floor() as i32;
        let y0 = (self.y + 0.05).floor() as i32;
        let y1 = (self.y + self.h - 0.05).floor() as i32;
        (y0..=y1).any(|cy| map.has_ladder(cx, cy))
    }

    /// Whether a climbable tile lies directly under the feet, so a body standing on a ladder top
    /// can start climbing down.
    pub fn ladder_below(&self, map: &TileMap) -> bool {
        map.has_ladder(
            self.centre_x().floor() as i32,
            (self.y - 0.05).floor() as i32,
        )
    }

    /// Full physics step: X, then Y, then slope snapping, then moving platforms. A body grounded
    /// on a conveyor is carried along it on top of its own velocity.
    pub fn phys(&mut self, map: &TileMap, plats: &[Platform], dt: f64) {
        let was = self.on_ground;
        // The belt moves the body as a step of its own: if it only pushes the body into a wall, that
        // is not a hit of the body's own motion, so its velocity survives and it can walk away.
        let drift = if was { self.ground_drift(map) } else { 0.0 };
        if drift != 0.0 {
            self.move_x(map, drift * dt, was);
        }
        self.hit_x = self.move_x(map, self.vx * dt, was);
        if self.hit_x {
            self.vx = 0.0;
        }
        let pb = self.y;
        self.on_ground = self.move_y(map, self.vy * dt);
        self.on_slope = 0;
        let fx = self.x + self.w / 2.0;
        let tx = fx.floor() as i32;
        let ty = self.y.floor() as i32;
        for k in 0..2 {
            let yy = ty - k;
            let t = map.get(tx, yy);
            if map.is_slope(t) {
                let hs = f64::from(yy) + slope_height(map.props.slope(t), fx - f64::from(tx));
                if self.vy <= 0.0
                    && ((self.y < hs && self.y > hs - 1.0)
                        || (was && self.y >= hs && self.y - hs < STEP_UP))
                {
                    self.y = hs;
                    self.vy = 0.0;
                    self.on_ground = true;
                    self.on_slope = t;
                }
                break;
            } else if k == 0 && map.is_solid_tile(t) {
                break;
            }
        }
        self.on_plat = None;
        if self.vy <= 0.0 {
            for (i, pl) in plats.iter().enumerate() {
                let top = pl.y + pl.h;
                if self.x + self.w > pl.x
                    && self.x < pl.x + pl.w
                    && pb >= top - pl.dy - 0.05
                    && self.y <= top + 0.001
                {
                    self.y = top;
                    self.vy = 0.0;
                    self.on_ground = true;
                    self.on_plat = Some(i);
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::STEP;
    use crate::tilemap::{
        CONVEY_L, CONVEY_R, CONVEYOR_SPEED, LADDER, ONEWAY, SLOPE_R45, SOLID, TileProps,
    };

    const FLOOR: u8 = 1;
    const PLAT: u8 = 2;
    const RAMP: u8 = 3;
    const BELT_R: u8 = 4;
    const BELT_L: u8 = 5;
    const RUNG: u8 = 6;
    const RUNG_TOP: u8 = 7;

    fn world() -> TileMap {
        let mut p = TileProps::default();
        p.set_flags(FLOOR, SOLID);
        p.set_flags(PLAT, ONEWAY);
        p.set_slope(RAMP, SLOPE_R45);
        p.set_flags(BELT_R, SOLID | CONVEY_R);
        p.set_flags(BELT_L, SOLID | CONVEY_L);
        p.set_flags(RUNG, LADDER);
        p.set_flags(RUNG_TOP, LADDER | ONEWAY);
        let mut m = TileMap::new(40, 20, p);
        for x in 0..40 {
            m.set(x, 0, FLOOR);
            m.set(x, 1, FLOOR);
        }
        m
    }

    fn settle(b: &mut Body, m: &TileMap, ticks: usize) {
        for _ in 0..ticks {
            b.fall(STEP, 22.0);
            b.phys(m, &[], STEP);
        }
    }

    #[test]
    fn falls_and_rests_on_floor() {
        let m = world();
        let mut b = Body::new(5.0, 6.0, 0.7, 1.4);
        settle(&mut b, &m, 90);
        assert!(b.on_ground);
        assert!((b.y - 2.0).abs() < 1e-3, "y = {}", b.y);
        assert_eq!(b.vy, 0.0);
    }

    #[test]
    fn walls_stop_horizontal_motion() {
        let mut m = world();
        for y in 2..6 {
            m.set(10, y, FLOOR);
        }
        let mut b = Body::new(5.0, 2.0, 0.7, 1.4);
        b.on_ground = true;
        b.vx = 7.0;
        for _ in 0..120 {
            b.fall(STEP, 22.0);
            b.vx = 7.0;
            b.phys(&m, &[], STEP);
        }
        assert!(b.x + b.w <= 10.0 + 1e-3 && b.x + b.w > 9.9, "x = {}", b.x);
    }

    #[test]
    fn steps_up_small_ledges_but_not_big_ones() {
        let mut m = world();
        // A 0.5-tile ledge is modelled as the top of a one-tile block checked against STEP_UP:
        // standing at y=2 against a block whose top is y=3 (1.0 tile) must block.
        m.set(8, 2, FLOOR);
        let mut b = Body::new(6.0, 2.0, 0.7, 1.4);
        b.on_ground = true;
        for _ in 0..60 {
            b.fall(STEP, 22.0);
            b.vx = 5.0;
            b.phys(&m, &[], STEP);
        }
        assert!(
            b.x + b.w <= 8.0 + 1e-3,
            "blocked by 1-tile wall, x = {}",
            b.x
        );
        // Same block but body starts 0.5 tiles higher relative to it: ledge of 0.5 is steppable.
        let mut b2 = Body::new(6.0, 2.5, 0.7, 1.4);
        b2.on_ground = true;
        b2.vx = 5.0;
        let hit = b2.move_x(&m, 1.5, true);
        assert!(!hit, "0.5 ledge is stepped over");
    }

    #[test]
    fn walks_up_a_45_degree_ramp() {
        let mut m = world();
        // A staircase of 45 degree tiles: column i has fill up to row 1+i and a ramp on top.
        for i in 0..4 {
            for y in 0..(2 + i) {
                m.set(10 + i, y, FLOOR);
            }
            m.set(10 + i, 2 + i, RAMP);
        }
        let mut b = Body::new(8.0, 2.0, 0.7, 1.4);
        settle(&mut b, &m, 30);
        for _ in 0..40 {
            b.fall(STEP, 22.0);
            b.vx = 4.0;
            b.phys(&m, &[], STEP);
        }
        assert!(b.on_ground);
        let fx = b.x + b.w / 2.0;
        let expect = 2.0 + (fx - 10.0).max(0.0);
        assert!(
            (b.y - expect).abs() < 0.3,
            "y = {} expect ~ {}",
            b.y,
            expect
        );
        assert!(b.y > 2.4, "climbed the ramp, y = {}", b.y);
    }

    #[test]
    fn one_way_platform_lets_you_jump_through_and_land() {
        let mut m = world();
        m.set(5, 4, PLAT);
        let mut b = Body::new(5.0, 2.0, 0.7, 1.4);
        b.on_ground = true;
        b.vy = 24.0; // jumps higher than the platform (needs >4 tiles)
        let mut landed = false;
        for _ in 0..200 {
            b.fall(STEP, 22.0);
            b.phys(&m, &[], STEP);
            if b.on_ground && b.y > 4.5 {
                landed = true;
                break;
            }
        }
        assert!(
            landed,
            "should land on top of the one-way tile; y = {}",
            b.y
        );
        assert!((b.y - 5.0).abs() < 1e-3);
    }

    #[test]
    fn ceiling_bonk_stops_rising() {
        let mut m = world();
        m.set(5, 5, FLOOR);
        let mut b = Body::new(5.0, 2.0, 0.7, 1.4);
        b.on_ground = true;
        b.vy = 20.5;
        let mut bonked = false;
        for _ in 0..30 {
            b.fall(STEP, 22.0);
            b.phys(&m, &[], STEP);
            bonked |= b.bonk;
        }
        assert!(bonked);
        assert!(b.y + b.h <= 5.0 + 1e-3);
    }

    #[test]
    fn head_reach_stops_the_body_lower_and_never_pushes_it_down() {
        let mut m = world();
        m.set(5, 8, FLOOR);
        let mut b = Body::new(5.0, 2.0, 0.7, 1.4);
        b.head = 0.6;
        b.vy = 28.0;
        for _ in 0..30 {
            b.fall(STEP, 22.0);
            b.phys(&m, &[], STEP);
        }
        assert!(
            b.y + b.h + b.head <= 8.0 + 1e-3,
            "head under the tile, y = {}",
            b.y
        );
        // With no room at all (the tile just above the head reach), the body stays put.
        let mut c = Body::new(5.0, 6.5, 0.7, 1.4);
        c.head = 0.6;
        c.vy = 10.0;
        let y0 = c.y;
        c.phys(&m, &[], STEP);
        assert!(c.bonk && c.y >= y0);
    }

    #[test]
    fn jump_apex_matches_ballistics() {
        let m = world();
        let mut b = Body::new(5.0, 2.0, 0.7, 1.4);
        settle(&mut b, &m, 5);
        b.vy = 20.5;
        b.on_ground = false;
        let mut max_y = b.y;
        for _ in 0..120 {
            b.fall(STEP, 22.0);
            b.phys(&m, &[], STEP);
            max_y = max_y.max(b.y);
        }
        let apex = max_y - 2.0;
        let ideal = 20.5 * 20.5 / (2.0 * 60.0); // 3.5 tiles
        assert!((apex - ideal).abs() < 0.3, "apex {apex} vs {ideal}");
    }

    #[test]
    fn moving_platform_carries_rider() {
        let m = world();
        let mut pl = Platform::new(10.0, 4.0, 14.0, 4.0, 0.35);
        let mut b = Body::new(10.4, 4.5, 0.7, 1.4);
        b.on_ground = true;
        let start = b.x;
        for _ in 0..60 {
            pl.tick(STEP);
            if let Some(_i) = b.on_plat {
                b.x += pl.dx;
                b.y += pl.dy;
            }
            b.fall(STEP, 22.0);
            b.phys(&m, std::slice::from_ref(&pl), STEP);
        }
        assert_eq!(b.on_plat, Some(0));
        assert!(b.x > start + 0.1, "carried right: {} -> {}", start, b.x);
    }

    #[test]
    fn a_conveyor_carries_a_standing_body_at_its_speed() {
        let mut m = world();
        for x in 5..20 {
            m.set(x, 1, BELT_R);
        }
        let mut b = Body::new(6.0, 2.0, 0.7, 1.4);
        settle(&mut b, &m, 5);
        let start = b.x;
        for _ in 0..60 {
            b.fall(STEP, 22.0);
            b.vx = 0.0;
            b.phys(&m, &[], STEP);
        }
        assert!(b.on_ground);
        assert!(
            (b.x - start - CONVEYOR_SPEED).abs() < 0.1,
            "carried {} in 1 s",
            b.x - start
        );
    }

    #[test]
    fn a_conveyor_adds_to_walking_and_runs_both_ways() {
        let mut m = world();
        for x in 5..20 {
            m.set(x, 1, BELT_L);
        }
        let mut b = Body::new(15.0, 2.0, 0.7, 1.4);
        settle(&mut b, &m, 5);
        let start = b.x;
        for _ in 0..30 {
            b.fall(STEP, 22.0);
            b.vx = 2.0;
            b.phys(&m, &[], STEP);
        }
        assert!(
            (b.x - start - (2.0 - CONVEYOR_SPEED) * 0.5).abs() < 0.1,
            "net drift {}",
            b.x - start
        );
    }

    #[test]
    fn a_body_can_walk_away_from_a_wall_a_belt_is_pushing_it_into() {
        let mut m = world();
        for x in 5..20 {
            m.set(x, 1, BELT_R);
        }
        for y in 2..6 {
            m.set(18, y, FLOOR);
        }
        // Pinned against the wall by the belt, then pushing left at less than the belt's speed:
        // with the old combined-move collision the wall hit zeroed the velocity every tick, so
        // it never built up and the body stayed trapped.
        let mut b = Body::new(17.2, 2.0, 0.7, 1.4);
        settle(&mut b, &m, 20);
        assert!(b.x + b.w <= 18.0 + 1e-3, "against the wall, x = {}", b.x);
        let start = b.x;
        for _ in 0..120 {
            b.fall(STEP, 22.0);
            // Ground acceleration as the game applies it: build up towards 7 tiles/s.
            b.vx = (b.vx - 55.0 * STEP).max(-7.0);
            b.phys(&m, &[], STEP);
        }
        assert!(
            b.x < start - 3.0,
            "walked away from the wall against the belt, x = {} from {}",
            b.x,
            start
        );
    }

    #[test]
    fn a_conveyor_does_not_push_through_walls_or_carry_airborne_bodies() {
        let mut m = world();
        for x in 5..20 {
            m.set(x, 1, BELT_R);
        }
        for y in 2..6 {
            m.set(10, y, FLOOR);
        }
        let mut b = Body::new(6.0, 2.0, 0.7, 1.4);
        settle(&mut b, &m, 5);
        for _ in 0..180 {
            b.fall(STEP, 22.0);
            b.vx = 0.0;
            b.phys(&m, &[], STEP);
        }
        assert!(b.x + b.w <= 10.0 + 1e-3, "stopped by the wall, x = {}", b.x);
        let mut a = Body::new(12.0, 8.0, 0.7, 1.4);
        a.vx = 0.0;
        let x0 = a.x;
        a.fall(STEP, 22.0);
        a.phys(&m, &[], STEP);
        assert_eq!(a.x, x0, "airborne bodies are not carried");
        assert_eq!(a.ground_drift(&m), 0.0);
    }

    #[test]
    fn ladder_overlap_and_ladder_below_are_detected() {
        let mut m = world();
        for y in 2..8 {
            m.set(5, y, RUNG);
        }
        m.set(5, 8, RUNG_TOP);
        let on = Body::new(4.8, 3.0, 0.7, 1.4);
        assert!(on.on_ladder(&m), "centre column inside the ladder");
        let off = Body::new(7.0, 3.0, 0.7, 1.4);
        assert!(!off.on_ladder(&m));
        let beside = Body::new(3.9, 3.0, 0.7, 1.4);
        assert!(!beside.on_ladder(&m), "only the centre column counts");
        let atop = Body::new(4.8, 9.0, 0.7, 1.4);
        assert!(atop.ladder_below(&m), "feet rest on the ladder top");
    }

    #[test]
    fn a_ladder_top_can_be_stood_on_from_above_but_walked_through_from_below() {
        let mut m = world();
        for y in 2..8 {
            m.set(5, y, RUNG);
        }
        m.set(5, 8, RUNG_TOP);
        let mut b = Body::new(4.85, 12.0, 0.7, 1.4);
        settle(&mut b, &m, 90);
        assert!(b.on_ground);
        assert!((b.y - 9.0).abs() < 1e-3, "standing on the top, y = {}", b.y);
        // Rising from the rungs below, the body's head and then its feet cross the ladder-top tile
        // (row 8) and it ends up above it.
        let mut up = Body::new(4.85, 6.0, 0.7, 1.4);
        up.vy = 14.0;
        for _ in 0..30 {
            up.phys(&m, &[], STEP);
        }
        assert!(up.y > 9.5, "rose through the ladder top, y = {}", up.y);
    }
}
