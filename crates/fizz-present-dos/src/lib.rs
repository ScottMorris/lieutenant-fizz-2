// Renders the slice into a 320x200 indexed frame and maps keys to buttons, for the DOS and host presenters.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The frame is a pure function of the world. Nothing here touches hardware: `present` hands the finished frame to a
// `fizz_hal::Display`, and the platform layer owns the copy to the screen.
#![no_std]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

mod frame;
mod palette;

use core::fmt::{self, Write};

use fizz_hal::{Display, HalError, KeyCode};
use fizz_sim::Buttons;

pub use fizz_slice_view::DrawOptions;
pub use fizz_view::{GLYPH, glyph};
pub use frame::Frame;
pub use palette::*;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT;

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

/// Height of the black strip that carries the slice's HUD text.
const SLICE_HUD_H: i32 = 10;

/// The ticks at which `frame_digest` is taken by `FIZZ2.EXE /BENCH` and by the `frames` example. The last is before the exit
/// banner, which would add text below the HUD strip.
pub const DIGEST_TICKS: [u64; 7] = [150, 270, 400, 602, 700, 865, 1000];

/// An FNV-1a digest of the picture below the HUD strip, the part that `fizz_slice_view::draw_slice` alone draws. The DOS
/// bench and the host `frames` example print it for the same ticks, so the DOS drawing path (its `memset`, `memcpy` and any
/// faster routine) is proved to draw the pixels the portable code does.
pub fn frame_digest(frame: &Frame) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in &frame.pixels()[SLICE_HUD_H as usize * WIDTH..] {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Draws the slice: the shared picture from `fizz_slice_view`, then the text HUD over a black strip along the top (ammo,
/// whether Ben carries the key, deaths and the tick), and a banner once Ben has reached the exit. The text is this
/// presenter's own; the picture beneath it is the one the Web draws too. Captions are on; see [`render_slice_with`].
pub fn render_slice(world: &fizz_ep2::World, frame: &mut Frame) {
    render_slice_with(world, frame, &fizz_slice_view::DrawOptions::default());
}

/// [`render_slice`] with the shared picture's options (the DOS `/NOCAPTIONS` switch sets `captions` false).
pub fn render_slice_with(
    world: &fizz_ep2::World,
    frame: &mut Frame,
    options: &fizz_slice_view::DrawOptions,
) {
    fizz_slice_view::draw_slice_with(
        frame.pixels_mut(),
        WIDTH,
        HEIGHT,
        world,
        world.tick.0 as u32,
        options,
    );
    frame.fill_rect(0, 0, WIDTH as i32, SLICE_HUD_H, BLACK);
    let key = world.flags.contains(fizz_ep2::WorldFlags::KEY_RED);
    let mut line = TextBuf::new();
    if write!(line, "AMMO {}", world.ammo).is_ok() {
        frame.draw_text(4, 1, line.as_str(), WHITE);
    }
    let mut line = TextBuf::new();
    if write!(line, "KEY {}", if key { "RED" } else { "-" }).is_ok() {
        frame.draw_text(
            76,
            1,
            line.as_str(),
            if key { LIGHT_RED } else { LIGHT_GREY },
        );
    }
    let mut line = TextBuf::new();
    if write!(line, "DEATHS {}", world.deaths).is_ok() {
        frame.draw_text(148, 1, line.as_str(), WHITE);
    }
    let mut line = TextBuf::new();
    if write!(line, "TICK {}", world.tick.0).is_ok() {
        frame.draw_text(244, 1, line.as_str(), LIGHT_GREY);
    }
    if world.flags.contains(fizz_ep2::WorldFlags::EXIT_REACHED) {
        let text = "LEVEL CLEAR";
        let x = (WIDTH as i32 - text.len() as i32 * GLYPH as i32) / 2;
        frame.fill_rect(x - 4, 146, text.len() as i32 * GLYPH as i32 + 8, 16, BLACK);
        frame.draw_text(x, 150, text, YELLOW);
    }
}

/// Which keys drive the buttons. The two sets are exclusive: a key of the other layout does nothing.
///
/// `Keen` is Fizz 1's classic layout and the default everywhere. `Modern` keeps the keys away from the Ctrl and Alt
/// shortcuts that browsers and desktops take.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Layout {
    /// Arrows move, Ctrl jumps, Alt pogos, Space fires.
    #[default]
    Keen,
    /// Arrows or `W` `A` `S` `D` move, `Z` jumps, `X` pogos, `C` fires; Space fires too.
    Modern,
}

/// Maps a key to the game button it drives in `layout`, or `None` for a key the layout does not use.
///
/// Arrows move in both layouts and Space fires in both; `W` `A` `S` `D` move only in `Modern`, and Ctrl and Alt are only
/// jump and pogo in `Keen`.
pub fn key_to_buttons(code: KeyCode, layout: Layout) -> Option<Buttons> {
    let modern = layout == Layout::Modern;
    match code {
        KeyCode::Up => Some(Buttons::UP),
        KeyCode::Down => Some(Buttons::DOWN),
        KeyCode::Left => Some(Buttons::LEFT),
        KeyCode::Right => Some(Buttons::RIGHT),
        KeyCode::Space => Some(Buttons::FIRE),
        KeyCode::W if modern => Some(Buttons::UP),
        KeyCode::S if modern => Some(Buttons::DOWN),
        KeyCode::A if modern => Some(Buttons::LEFT),
        KeyCode::D if modern => Some(Buttons::RIGHT),
        KeyCode::Z if modern => Some(Buttons::JUMP),
        KeyCode::X if modern => Some(Buttons::POGO),
        KeyCode::C if modern => Some(Buttons::FIRE),
        KeyCode::Ctrl if !modern => Some(Buttons::JUMP),
        KeyCode::Alt if !modern => Some(Buttons::POGO),
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

    const ALL_KEYS: [KeyCode; 14] = [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::W,
        KeyCode::A,
        KeyCode::S,
        KeyCode::D,
        KeyCode::Space,
        KeyCode::Z,
        KeyCode::X,
        KeyCode::C,
        KeyCode::Ctrl,
        KeyCode::Alt,
    ];

    fn check(layout: Layout, table: &[(KeyCode, Buttons)]) {
        for key in ALL_KEYS {
            let want = table.iter().find(|(k, _)| *k == key).map(|(_, b)| *b);
            assert_eq!(key_to_buttons(key, layout), want, "{layout:?} {key:?}");
        }
    }

    #[test]
    fn keen_layout_is_arrows_ctrl_alt_space() {
        check(
            Layout::Keen,
            &[
                (KeyCode::Up, Buttons::UP),
                (KeyCode::Down, Buttons::DOWN),
                (KeyCode::Left, Buttons::LEFT),
                (KeyCode::Right, Buttons::RIGHT),
                (KeyCode::Ctrl, Buttons::JUMP),
                (KeyCode::Alt, Buttons::POGO),
                (KeyCode::Space, Buttons::FIRE),
            ],
        );
    }

    #[test]
    fn modern_layout_is_wasd_or_arrows_zxc() {
        check(
            Layout::Modern,
            &[
                (KeyCode::Up, Buttons::UP),
                (KeyCode::W, Buttons::UP),
                (KeyCode::Down, Buttons::DOWN),
                (KeyCode::S, Buttons::DOWN),
                (KeyCode::Left, Buttons::LEFT),
                (KeyCode::A, Buttons::LEFT),
                (KeyCode::Right, Buttons::RIGHT),
                (KeyCode::D, Buttons::RIGHT),
                (KeyCode::Z, Buttons::JUMP),
                (KeyCode::X, Buttons::POGO),
                (KeyCode::C, Buttons::FIRE),
                (KeyCode::Space, Buttons::FIRE),
            ],
        );
    }

    #[test]
    fn the_default_layout_is_keen() {
        assert_eq!(Layout::default(), Layout::Keen);
    }

    #[test]
    fn unmapped_keys_drive_nothing() {
        for layout in [Layout::Keen, Layout::Modern] {
            assert_eq!(key_to_buttons(KeyCode::Escape, layout), None);
            let other = KeyCode::Unknown {
                extended: false,
                scancode: 0x1e,
            };
            assert_eq!(key_to_buttons(other, layout), None);
        }
    }

    #[test]
    fn the_slice_hud_shows_ammo_key_deaths_and_tick() {
        let mut world = fizz_ep2::level::world();
        let mut frame = Frame::new();
        render_slice(&world, &mut frame);
        let expect = |text: &str, x: i32, colour: u8, frame: &Frame| {
            let mut e = Frame::new();
            e.draw_text(x, 1, text, colour);
            for y in 0..8usize {
                for px in 0..WIDTH {
                    if e.pixels()[(y + 1) * WIDTH + px] == colour {
                        assert_eq!(frame.pixels()[(y + 1) * WIDTH + px], colour, "{text}");
                    }
                }
            }
        };
        expect("AMMO 5", 4, WHITE, &frame);
        expect("KEY -", 76, LIGHT_GREY, &frame);
        expect("DEATHS 0", 148, WHITE, &frame);
        expect("TICK 0", 244, LIGHT_GREY, &frame);
        world.flags.set(fizz_ep2::WorldFlags::KEY_RED);
        render_slice(&world, &mut frame);
        expect("KEY RED", 76, LIGHT_RED, &frame);
        // Below the strip the picture is the shared one.
        let mut bare = Frame::new();
        fizz_slice_view::draw_slice(bare.pixels_mut(), WIDTH, HEIGHT, &world, 0);
        let below = SLICE_HUD_H as usize * WIDTH..;
        assert_eq!(frame.pixels()[below.clone()], bare.pixels()[below]);
    }

    #[test]
    fn the_slice_banner_appears_at_the_exit() {
        let mut world = fizz_ep2::level::world();
        let mut frame = Frame::new();
        render_slice(&world, &mut frame);
        let before = frame.pixels().to_vec();
        world.flags.set(fizz_ep2::WorldFlags::EXIT_REACHED);
        render_slice(&world, &mut frame);
        assert_ne!(before, frame.pixels());
        assert!(frame.pixels().contains(&YELLOW));
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
