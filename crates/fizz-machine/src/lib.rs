// The machine model for the Fizz 2 DOS start-up check: probe results, sound settings, the resolver and the check screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Everything here is pure data and arithmetic. The DOS runtime finds out what the machine has and fills in a `Probe`; the
// player's file and flags become `Settings`; `resolve` turns the two into a `Resolved` plan; `cells` draws the check screen
// from the probe and the plan. Nothing touches hardware, so every rule is tested on the host, and nothing allocates.
#![no_std]
#![forbid(unsafe_code)]

// Host tests only: the tests use `std` for formatting; the library itself never links it.
#[cfg(test)]
extern crate std;

pub mod probe;
pub mod resolve;
pub mod route;
pub mod screen;
pub mod settings;

pub use probe::{Blaster, CpuClass, FmKind, Probe, SoundBlaster, VideoClass, parse_blaster};
pub use resolve::{FxPlan, MusicPlan, Reason, Resolved, SoundState, SoundStates, resolve};
pub use route::{OplMode, route};
pub use screen::{CELLS, COLS, MEMORY_FLOOR_KB, ROWS, cells, ready};
pub use settings::{Config, ConfigIssue, FlagReport, FxChoice, MusicChoice, Settings, apply_flags};
