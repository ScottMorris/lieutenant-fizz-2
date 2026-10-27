// A Fizz Blaster shot in flight.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: Fizz 1 `episodes/episode-1/game/src/ents.rs` at commit `4bce036`: `Shot`, reduced to Ben's own shots.
// Left out: the owner flag (every shot here is Ben's), gravity, the sprite and the mirror bookkeeping.

/// A bubble of fizz. It flies straight, stuns the first chicken it enters and vanishes on a wall or when its life runs
/// out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    /// Seconds of flight left.
    pub life: f64,
}
