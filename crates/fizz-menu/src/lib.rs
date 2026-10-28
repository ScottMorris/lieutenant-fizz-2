// The menu model: a generic list with a cursor, the Options screen and the Paused menu, with every label and value word.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Nothing here draws, reads a key from hardware or touches a file. A presenter turns its keys into `MenuKey`, calls `key`, draws
// the `RowView`s that `rows` returns and saves `Values::to_prefs` through `FIZZ2.CFG` when `changed` is set. The words live here
// so the Web page, the DOS screen and the manual agree. The crate is `no_std`, allocates nothing and has no floating point.
#![no_std]
#![forbid(unsafe_code)]

// Host tests only: the tests use `std` for formatting; the library itself never links it.
#[cfg(test)]
extern crate std;

pub mod menu;
pub mod options;
pub mod paused;
pub mod text;

pub use menu::{Entry, Kind, Menu, MenuKey, Outcome, RowId};
pub use options::{
    Caps, Device, MotionPick, OVERRIDE_NOTE, OptionsScreen, Overrides, Platform, RowList, RowView,
    Values,
};
pub use paused::PausedScreen;
pub use text::{TEXT_CAP, Text};
