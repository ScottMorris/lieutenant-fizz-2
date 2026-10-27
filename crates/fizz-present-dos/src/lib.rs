// Renders the Milestone 0 world into a 320x200 indexed frame and maps keys to buttons, for the DOS and host presenters.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The frame is a pure function of the world. Nothing here touches hardware: `present` hands the finished frame to a
// `fizz_hal::Display`, and the platform layer owns the copy to the screen.
#![no_std]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

mod font;
mod frame;
mod palette;

use core::fmt::{self, Write};

use fizz_hal::{Display, HalError, KeyCode};
use fizz_hello::World;
use fizz_sim::Buttons;

pub use font::{GLYPH, glyph};
pub use frame::Frame;
pub use palette::*;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT;

/// Side length of the marker square in pixels.
pub const MARKER_SIZE: i32 = 8;

/// Left edge of the HUD text, clear of the marker when it sits against the left clamp.
const HUD_X: i32 = 16;
const HUD_TICK_Y: i32 = 8;
const HUD_HASH_Y: i32 = 16;
const HUD_DASH_Y: i32 = 24;

/// A fixed-capacity text buffer, so the HUD needs no heap and no `format!`.
struct TextBuf {
    bytes: [u8; 32],
    len: usize,
}

impl TextBuf {
    const fn new() -> Self {
        Self {
            bytes: [0; 32],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

impl Write for TextBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        if end > self.bytes.len() {
            return Err(fmt::Error);
        }
        self.bytes[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// Draws the world: dark blue field, border, yellow marker with a light-cyan outline and a white HUD.
pub fn render(world: &World, frame: &mut Frame) {
    frame.clear(BLUE);
    frame.stroke_rect(0, 0, WIDTH as i32, HEIGHT as i32, LIGHT_GREY);

    let mx = world.x as i32;
    let my = world.y as i32;
    frame.fill_rect(mx, my, MARKER_SIZE, MARKER_SIZE, YELLOW);
    frame.stroke_rect(mx - 1, my - 1, MARKER_SIZE + 2, MARKER_SIZE + 2, LIGHT_CYAN);

    let mut line = TextBuf::new();
    if write!(line, "TICK {}", world.tick.0).is_ok() {
        frame.draw_text(HUD_X, HUD_TICK_Y, line.as_str(), WHITE);
    }
    let mut line = TextBuf::new();
    if write!(line, "HASH {:016x}", fizz_hello::hash(world).finish()).is_ok() {
        frame.draw_text(HUD_X, HUD_HASH_Y, line.as_str(), WHITE);
    }
    let mut line = TextBuf::new();
    if write!(line, "DASH {}", world.dashes).is_ok() {
        frame.draw_text(HUD_X, HUD_DASH_Y, line.as_str(), WHITE);
    }
}

/// Maps a key to the game button it drives: arrows and WASD move, Space is the action.
pub fn key_to_buttons(code: KeyCode) -> Option<Buttons> {
    match code {
        KeyCode::Up | KeyCode::W => Some(Buttons::UP),
        KeyCode::Down | KeyCode::S => Some(Buttons::DOWN),
        KeyCode::Left | KeyCode::A => Some(Buttons::LEFT),
        KeyCode::Right | KeyCode::D => Some(Buttons::RIGHT),
        KeyCode::Space => Some(Buttons::ACTION),
        _ => None,
    }
}

/// Hands the finished frame to the display.
pub fn present<D: Display>(display: &mut D, frame: &Frame) -> Result<(), HalError> {
    display.present(frame.pixels(), WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mapped_key_drives_its_button() {
        let cases = [
            (KeyCode::Up, Buttons::UP),
            (KeyCode::W, Buttons::UP),
            (KeyCode::Down, Buttons::DOWN),
            (KeyCode::S, Buttons::DOWN),
            (KeyCode::Left, Buttons::LEFT),
            (KeyCode::A, Buttons::LEFT),
            (KeyCode::Right, Buttons::RIGHT),
            (KeyCode::D, Buttons::RIGHT),
            (KeyCode::Space, Buttons::ACTION),
        ];
        for (key, want) in cases {
            assert_eq!(key_to_buttons(key), Some(want), "{key:?}");
        }
    }

    #[test]
    fn unmapped_keys_drive_nothing() {
        assert_eq!(key_to_buttons(KeyCode::Escape), None);
        let other = KeyCode::Unknown {
            extended: false,
            scancode: 0x1e,
        };
        assert_eq!(key_to_buttons(other), None);
    }

    #[test]
    fn text_buf_rejects_overflow() {
        let mut t = TextBuf::new();
        assert!(write!(t, "{:040}", 1).is_err());
        let mut t = TextBuf::new();
        assert!(write!(t, "TICK {}", u64::MAX).is_ok());
        assert_eq!(t.as_str(), "TICK 18446744073709551615");
    }
}
