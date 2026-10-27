// Input: the key codes the game cares about, key events and the polling trait a platform implements.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Keys outside the game's set are reported as `Unknown` with their raw set 1 scancode, so a presenter can ignore them and a
// debugging build can still see them.

/// A physical key, named for the keys the game uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    Up,
    Down,
    Left,
    Right,
    W,
    A,
    S,
    D,
    Space,
    Escape,
    /// Any other key: its make code (bit 7 clear) and whether it came after the `0xE0` prefix.
    Unknown {
        extended: bool,
        scancode: u8,
    },
}

/// A key press or release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    /// True for a press (make code), false for a release (break code).
    pub pressed: bool,
}

/// A source of key events, polled by the game loop.
pub trait InputSource {
    /// The oldest pending event, if any.
    fn poll_key(&mut self) -> Option<KeyEvent>;
}
