// Crate root for Episode 2's rules: Ben, the chicken, the red key and door, ported from Fizz 1 and proved against its traces.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Episode 2's rules on `fizz-sim` and `fizz-phys`.
//!
//! The crate holds [`ben::Ben`], the player's movement, ported line for line from Fizz 1's `tick_ben_move`; the
//! [`chicken::Chicken`] (Fizz 1's Gloop); the Fizz Blaster [`shot::Shot`]; and the [`world::World`] that ties them to the
//! red key and door, death, respawn and the exit. The golden traces under `tests/fizz1` come from Fizz 1's own
//! simulation and are compared bit for bit.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod ben;
pub mod chicken;
pub mod shot;
pub mod tiles;
pub mod world;

pub use ben::Ben;
pub use chicken::Chicken;
pub use shot::Shot;
pub use world::{World, WorldFlags, hash, step};
