// Crate root for Fizz 2's owned physics: Fizz 1's tile map, body, platform and rng code, `no_std`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: `STEP`, `GRAVITY` and the re-exports follow Fizz 1 `crates/sim/src/lib.rs` at commit `4bce036`;
// `instance`, `events` and `lights` are not carried over, and the `no_std` wiring follows fizz-dos `m2/patches/sim.patch`.

//! Fizz 1's deterministic tile and AABB physics as an owned `no_std` crate.
//!
//! The function bodies of `tilemap`, `body`, `platform` and `rng` are Fizz 1's, byte for byte apart from the mechanical
//! changes listed in each file's Provenance comment; `tools/lf-sim-drift.sh` reports any difference. Floating-point
//! functions that `core` lacks come from `libm` through [`fmath`]. The `exp` family is deliberately absent.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod body;
pub mod fmath;
pub mod platform;
pub mod rng;
pub mod tilemap;

pub use body::Body;
pub use platform::Platform;
pub use rng::{Rng, hashf};
pub use tilemap::{TileMap, TileProps, slope_height};

/// Fixed simulation step (60 Hz).
pub const STEP: f64 = 1.0 / 60.0;
/// World gravity in tiles per second squared.
pub const GRAVITY: f64 = 60.0;
