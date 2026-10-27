// Renders the Milestone 0 world into a 320x200 indexed frame for the DOS and host presenters.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

mod font;
mod frame;
mod palette;

pub use font::{GLYPH, glyph};
pub use frame::Frame;
pub use palette::*;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT;
