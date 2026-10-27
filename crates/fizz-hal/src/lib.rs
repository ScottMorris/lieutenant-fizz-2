// The hardware abstraction for Lieutenant Fizz presenters: clock, input, display and the interrupt hand-off ring.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The shape follows Desk's `desk-hal` (traits a platform implements, no platform types in the core), written fresh so the real
// Desk crates can replace it later. Everything is `no_std` with no dependencies; the `mock` feature adds host-side test doubles.
#![no_std]
#![deny(unsafe_code)]
#![deny(clippy::float_arithmetic)]

#[cfg(feature = "mock")]
extern crate alloc;

pub mod time;

pub use time::{Clock, Pacer, Ticks};
