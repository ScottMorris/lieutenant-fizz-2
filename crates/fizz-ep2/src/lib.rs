// Crate root for Episode 2's rules: Ben's movement, ported from Fizz 1 and proved against its traces.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Episode 2's rules on `fizz-sim` and `fizz-phys`.
//!
//! So far the crate holds [`ben::Ben`], the player's movement, ported line for line from Fizz 1's `tick_ben_move`. The
//! golden traces under `tests/fizz1` come from Fizz 1's own simulation and are compared bit for bit.
#![no_std]
#![forbid(unsafe_code)]

pub mod ben;

pub use ben::Ben;
