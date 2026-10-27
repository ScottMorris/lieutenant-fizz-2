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
// Host tests only: the ring's threaded test needs `std`; the library itself never links it.
#[cfg(test)]
extern crate std;

pub mod input;
#[cfg(feature = "mock")]
pub mod mock;
pub mod ring;
pub mod scancode;
pub mod time;
pub mod video;

pub use input::{InputSource, KeyCode, KeyEvent};
pub use ring::{Consumer, Producer, SpscRing};
pub use scancode::Set1Decoder;
pub use time::{Clock, Pacer, Ticks};
pub use video::{Display, HalError, ModeInfo, Rgb6};
