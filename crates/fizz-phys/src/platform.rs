// Moving platforms that ease between anchor points and carry their riders.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: Derived from Fizz 1 `crates/sim/src/platform.rs` at commit `4bce036`; transformed as in fizz-dos `m2/patches/sim.patch`.
// Edits: `use crate::fmath::FloatMath;` added (unused in test builds, where `std` supplies `floor` and `cos`); `std::f64::consts::PI` became `core::f64::consts::PI`.

#[cfg_attr(test, allow(unused_imports))]
use crate::fmath::FloatMath;

/// A moving platform that eases between two anchor points; riders are carried by its
/// per-tick delta (`dx`, `dy`).
#[derive(Clone, Debug, Default)]
pub struct Platform {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub ax: f64,
    pub ay: f64,
    pub bx: f64,
    pub by: f64,
    pub speed: f64,
    pub ph: f64,
    pub dx: f64,
    pub dy: f64,
    /// Seconds to rest at each anchor before moving again; 0 keeps the platform in constant motion.
    pub dwell: f64,
    /// Seconds of rest remaining.
    pub wait: f64,
}

impl Platform {
    pub fn new(x: f64, y: f64, bx: f64, by: f64, speed: f64) -> Self {
        Platform {
            x,
            y,
            ax: x,
            ay: y,
            bx,
            by,
            w: 2.0,
            h: 0.5,
            speed,
            ..Default::default()
        }
    }

    /// Sets the platform's width and height in tiles.
    pub fn sized(mut self, w: f64, h: f64) -> Self {
        self.w = w;
        self.h = h;
        self
    }

    /// Makes the platform rest for `secs` at each anchor (a lift that waits at every floor).
    pub fn with_dwell(mut self, secs: f64) -> Self {
        self.dwell = secs;
        self
    }

    /// Advances the cosine ease and records the movement delta for riders.
    pub fn tick(&mut self, dt: f64) {
        if self.wait > 0.0 {
            self.wait = (self.wait - dt).max(0.0);
            self.dx = 0.0;
            self.dy = 0.0;
            return;
        }
        let before = self.ph;
        self.ph += dt * self.speed;
        if self.dwell > 0.0 && self.ph.floor() > before.floor() {
            // Phase boundaries are the anchors: stop exactly on the one just reached.
            self.ph = self.ph.floor();
            self.wait = self.dwell;
        }
        let k = 0.5 - 0.5 * (self.ph * core::f64::consts::PI).cos();
        let nx = self.ax + (self.bx - self.ax) * k;
        let ny = self.ay + (self.by - self.ay) * k;
        self.dx = nx - self.x;
        self.dy = ny - self.y;
        self.x = nx;
        self.y = ny;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_anchors_at_the_start_with_default_size() {
        let p = Platform::new(1.0, 2.0, 5.0, 2.0, 0.5);
        assert_eq!(
            (p.x, p.y, p.ax, p.ay, p.bx, p.by),
            (1.0, 2.0, 1.0, 2.0, 5.0, 2.0)
        );
        assert_eq!((p.w, p.h, p.speed), (2.0, 0.5, 0.5));
        assert_eq!((p.ph, p.dx, p.dy), (0.0, 0.0, 0.0));
    }

    #[test]
    fn tick_eases_toward_the_far_anchor_and_records_the_delta() {
        let mut p = Platform::new(0.0, 0.0, 4.0, 2.0, 1.0);
        // Phase 0.5 is the midpoint of the cosine ease.
        p.tick(0.5);
        assert!((p.x - 2.0).abs() < 1e-9 && (p.y - 1.0).abs() < 1e-9);
        assert!((p.dx - 2.0).abs() < 1e-9 && (p.dy - 1.0).abs() < 1e-9);
        // Phase 1.0 reaches the far anchor, and the delta is the second half only.
        p.tick(0.5);
        assert!((p.x - 4.0).abs() < 1e-9 && (p.y - 2.0).abs() < 1e-9);
        assert!((p.dx - 2.0).abs() < 1e-9 && (p.dy - 1.0).abs() < 1e-9);
    }

    #[test]
    fn tick_returns_to_the_start_after_a_full_period() {
        let mut p = Platform::new(3.0, 1.0, 7.0, 1.0, 1.0);
        p.tick(2.0);
        assert!((p.x - 3.0).abs() < 1e-9 && (p.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn deltas_sum_to_the_total_displacement() {
        let mut p = Platform::new(0.0, 0.0, 6.0, -3.0, 0.7);
        let (mut sx, mut sy) = (0.0, 0.0);
        for _ in 0..90 {
            p.tick(1.0 / 60.0);
            sx += p.dx;
            sy += p.dy;
        }
        assert!((sx - p.x).abs() < 1e-9 && (sy - p.y).abs() < 1e-9);
    }

    #[test]
    fn zero_speed_or_zero_dt_does_not_move() {
        let mut still = Platform::new(1.0, 1.0, 9.0, 9.0, 0.0);
        still.tick(1.0);
        assert_eq!((still.x, still.y, still.dx, still.dy), (1.0, 1.0, 0.0, 0.0));
        let mut paused = Platform::new(1.0, 1.0, 9.0, 9.0, 2.0);
        paused.tick(0.0);
        assert_eq!(
            (paused.x, paused.y, paused.dx, paused.dy),
            (1.0, 1.0, 0.0, 0.0)
        );
    }

    #[test]
    fn dwell_rests_at_each_anchor_then_resumes() {
        let mut p = Platform::new(0.0, 0.0, 4.0, 0.0, 1.0).with_dwell(0.5);
        let mut at_b = 0;
        for _ in 0..90 {
            p.tick(1.0 / 60.0);
            if (p.x - 4.0).abs() < 1e-9 && p.dx == 0.0 {
                at_b += 1;
            }
        }
        // 1 s to reach B (plus a tick of slack), then 0.5 s of rest at 60 Hz.
        assert!((29..=31).contains(&at_b), "rested {at_b} ticks at B");
        for _ in 0..30 {
            p.tick(1.0 / 60.0);
        }
        assert!(p.x < 4.0 - 1e-6, "left B after the rest, x = {}", p.x);
    }

    #[test]
    fn dwell_lands_exactly_on_the_anchor_even_when_the_step_overshoots() {
        let mut p = Platform::new(0.0, 0.0, 4.0, 2.0, 1.0).with_dwell(1.0);
        p.tick(1.3);
        assert!((p.x - 4.0).abs() < 1e-9 && (p.y - 2.0).abs() < 1e-9);
        assert!(p.wait > 0.99);
    }

    #[test]
    fn no_dwell_means_no_rest_and_sized_sets_the_footprint() {
        let mut p = Platform::new(0.0, 0.0, 4.0, 0.0, 1.0).sized(3.0, 1.0);
        assert_eq!((p.w, p.h), (3.0, 1.0));
        p.tick(1.0);
        p.tick(0.1);
        assert!(p.dx < 0.0 && p.wait == 0.0, "turned straight back");
    }
}
