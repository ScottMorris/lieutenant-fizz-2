// The keyboard decoder: scancode set 1 bytes in, key events out.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pure: no ports, no interrupts, no allocation. In set 1 a make code is the key's number and a break code is the make code with
// bit 7 set. The arrows and other 101-key additions send `0xE0` first. Pause sends `E1 1D 45 E1 9D C5` on press and nothing on
// release; the decoder swallows that sequence, including its `1D`, so a Pause never looks like a Ctrl press. The left and right Ctrl and Alt
// keys (the right ones send `E0` first) report the same codes. Shaped after Desk's `desk-kbd` decoder, reduced to the
// keys the game uses.

use crate::input::{KeyCode, KeyEvent};

const EXTENDED: u8 = 0xE0;
const PAUSE_PREFIX: u8 = 0xE1;
/// The bytes after `E1` in the Pause sequence.
const PAUSE_TAIL: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    /// After `E0`.
    Extended,
    /// After `E1`: how many of the five trailing bytes remain.
    Pause(u8),
}

/// Turns scancode set 1 bytes into key events. Feed it every byte in order.
#[derive(Clone, Copy, Debug)]
pub struct Set1Decoder {
    state: State,
}

impl Default for Set1Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Set1Decoder {
    pub const fn new() -> Self {
        Set1Decoder { state: State::Idle }
    }

    /// Takes one byte. `None` while a sequence is incomplete and for bytes that carry no key (the Pause sequence, the keyboard's
    /// ACK, resend and overrun codes).
    pub fn feed(&mut self, byte: u8) -> Option<KeyEvent> {
        match self.state {
            State::Idle => match byte {
                EXTENDED => {
                    self.state = State::Extended;
                    None
                }
                PAUSE_PREFIX => {
                    self.state = State::Pause(PAUSE_TAIL);
                    None
                }
                0x00 | 0xFA | 0xFE | 0xFF => None,
                _ => Some(key(false, byte)),
            },
            State::Extended => {
                self.state = State::Idle;
                match byte {
                    EXTENDED => {
                        self.state = State::Extended;
                        None
                    }
                    PAUSE_PREFIX => {
                        self.state = State::Pause(PAUSE_TAIL);
                        None
                    }
                    // The fake shifts wrapped around Print Screen, Insert and the cursor block, and the error marker.
                    0x2A | 0xAA | 0x36 | 0xB6 | 0xFF => None,
                    _ => Some(key(true, byte)),
                }
            }
            State::Pause(left) => {
                self.state = if left <= 1 {
                    State::Idle
                } else {
                    State::Pause(left - 1)
                };
                None
            }
        }
    }
}

fn key(extended: bool, byte: u8) -> KeyEvent {
    let pressed = byte & 0x80 == 0;
    let scancode = byte & 0x7F;
    let code = match (extended, scancode) {
        (true, 0x48) => KeyCode::Up,
        (true, 0x50) => KeyCode::Down,
        (true, 0x4B) => KeyCode::Left,
        (true, 0x4D) => KeyCode::Right,
        (_, 0x1D) => KeyCode::Ctrl,
        (_, 0x38) => KeyCode::Alt,
        (false, 0x11) => KeyCode::W,
        (false, 0x1E) => KeyCode::A,
        (false, 0x1F) => KeyCode::S,
        (false, 0x20) => KeyCode::D,
        (false, 0x39) => KeyCode::Space,
        (false, 0x01) => KeyCode::Escape,
        (false, 0x2C) => KeyCode::Z,
        (false, 0x2D) => KeyCode::X,
        (false, 0x2E) => KeyCode::C,
        _ => KeyCode::Unknown { extended, scancode },
    };
    KeyEvent { code, pressed }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(d: &mut Set1Decoder, bytes: &[u8]) -> [Option<KeyEvent>; 4] {
        let mut out = [None; 4];
        for (slot, b) in out.iter_mut().zip(bytes) {
            *slot = d.feed(*b);
        }
        out
    }

    fn ev(code: KeyCode, pressed: bool) -> Option<KeyEvent> {
        Some(KeyEvent { code, pressed })
    }

    #[test]
    fn plain_keys_press_and_release() {
        let table = [
            (0x11, KeyCode::W),
            (0x1E, KeyCode::A),
            (0x1F, KeyCode::S),
            (0x20, KeyCode::D),
            (0x39, KeyCode::Space),
            (0x01, KeyCode::Escape),
            (0x2C, KeyCode::Z),
            (0x2D, KeyCode::X),
            (0x2E, KeyCode::C),
            (0x1D, KeyCode::Ctrl),
            (0x38, KeyCode::Alt),
        ];
        let mut d = Set1Decoder::new();
        for (make, code) in table {
            assert_eq!(d.feed(make), ev(code, true));
            assert_eq!(d.feed(make | 0x80), ev(code, false));
        }
    }

    #[test]
    fn right_ctrl_and_right_alt_match_the_left_keys() {
        let mut d = Set1Decoder::new();
        for (make, code) in [(0x1D, KeyCode::Ctrl), (0x38, KeyCode::Alt)] {
            assert_eq!(feed_all(&mut d, &[0xE0, make])[..2], [None, ev(code, true)]);
            assert_eq!(
                feed_all(&mut d, &[0xE0, make | 0x80])[..2],
                [None, ev(code, false)]
            );
        }
    }

    #[test]
    fn a_pause_does_not_produce_a_ctrl_event() {
        let mut d = Set1Decoder::new();
        for b in [0xE1, 0x1D, 0x45, 0xE1, 0x9D, 0xC5] {
            assert_eq!(d.feed(b), None, "{b:#04x}");
        }
        // The decoder is back to idle: a real Ctrl press afterwards still works.
        assert_eq!(d.feed(0x1D), ev(KeyCode::Ctrl, true));
        assert_eq!(d.feed(0x9D), ev(KeyCode::Ctrl, false));
    }

    #[test]
    fn extended_arrows_press_and_release() {
        let table = [
            (0x48, KeyCode::Up),
            (0x50, KeyCode::Down),
            (0x4B, KeyCode::Left),
            (0x4D, KeyCode::Right),
        ];
        let mut d = Set1Decoder::new();
        for (make, code) in table {
            assert_eq!(feed_all(&mut d, &[0xE0, make])[..2], [None, ev(code, true)]);
            assert_eq!(
                feed_all(&mut d, &[0xE0, make | 0x80])[..2],
                [None, ev(code, false)]
            );
        }
    }

    #[test]
    fn extended_and_plain_codes_are_distinct() {
        let mut d = Set1Decoder::new();
        // Keypad 8 (plain 0x48) is not the Up arrow; extended 0x11 is not W.
        assert_eq!(
            d.feed(0x48),
            ev(
                KeyCode::Unknown {
                    extended: false,
                    scancode: 0x48
                },
                true
            )
        );
        assert_eq!(d.feed(0xE0), None);
        assert_eq!(
            d.feed(0x11),
            ev(
                KeyCode::Unknown {
                    extended: true,
                    scancode: 0x11
                },
                true
            )
        );
    }

    #[test]
    fn unknown_codes_carry_the_make_code() {
        let mut d = Set1Decoder::new();
        assert_eq!(
            d.feed(0x9C),
            ev(
                KeyCode::Unknown {
                    extended: false,
                    scancode: 0x1C
                },
                false
            )
        );
    }

    #[test]
    fn a_stray_prefix_then_a_release_is_decoded_as_extended() {
        let mut d = Set1Decoder::new();
        assert_eq!(d.feed(0xE0), None);
        assert_eq!(
            d.feed(0x9E),
            ev(
                KeyCode::Unknown {
                    extended: true,
                    scancode: 0x1E
                },
                false
            )
        );
        // The prefix is consumed: the next byte is plain again.
        assert_eq!(d.feed(0x1E), ev(KeyCode::A, true));
    }

    #[test]
    fn the_pause_sequence_and_fake_shifts_report_nothing() {
        let mut d = Set1Decoder::new();
        for b in [0xE1, 0x1D, 0x45, 0xE1, 0x9D, 0xC5] {
            assert_eq!(d.feed(b), None);
        }
        assert_eq!(d.feed(0x11), ev(KeyCode::W, true));
        assert_eq!(d.feed(0xE0), None);
        assert_eq!(d.feed(0x2A), None);
        assert_eq!(d.feed(0xFA), None);
    }
}
