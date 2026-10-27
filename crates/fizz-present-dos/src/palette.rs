// Defines the sixteen EGA colours as 6-bit VGA DAC values, with named palette indices.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_hal::Rgb6;

pub const BLACK: u8 = 0;
pub const BLUE: u8 = 1;
pub const GREEN: u8 = 2;
pub const CYAN: u8 = 3;
pub const RED: u8 = 4;
pub const MAGENTA: u8 = 5;
pub const BROWN: u8 = 6;
pub const LIGHT_GREY: u8 = 7;
pub const DARK_GREY: u8 = 8;
pub const LIGHT_BLUE: u8 = 9;
pub const LIGHT_GREEN: u8 = 10;
pub const LIGHT_CYAN: u8 = 11;
pub const LIGHT_RED: u8 = 12;
pub const LIGHT_MAGENTA: u8 = 13;
pub const YELLOW: u8 = 14;
pub const WHITE: u8 = 15;

const fn c(r: u8, g: u8, b: u8) -> Rgb6 {
    Rgb6 { r, g, b }
}

/// The sixteen EGA colours in standard order, as DAC values (0xAA is 42, 0x55 is 21, 0xFF is 63).
pub const fn ega_palette() -> [Rgb6; 16] {
    [
        c(0, 0, 0),
        c(0, 0, 42),
        c(0, 42, 0),
        c(0, 42, 42),
        c(42, 0, 0),
        c(42, 0, 42),
        c(42, 21, 0),
        c(42, 42, 42),
        c(21, 21, 21),
        c(21, 21, 63),
        c(21, 63, 21),
        c(21, 63, 63),
        c(63, 21, 21),
        c(63, 21, 63),
        c(63, 63, 21),
        c(63, 63, 63),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sixteen_entries_are_in_dac_range() {
        let p = ega_palette();
        assert_eq!(p.len(), 16);
        for e in p {
            assert!(e.r <= 63 && e.g <= 63 && e.b <= 63);
        }
    }

    #[test]
    fn named_indices_pick_the_expected_colours() {
        let p = ega_palette();
        assert_eq!(p[usize::from(BLACK)], c(0, 0, 0));
        assert_eq!(p[usize::from(BLUE)], c(0, 0, 42));
        assert_eq!(p[usize::from(BROWN)], c(42, 21, 0));
        assert_eq!(p[usize::from(LIGHT_GREY)], c(42, 42, 42));
        assert_eq!(p[usize::from(DARK_GREY)], c(21, 21, 21));
        assert_eq!(p[usize::from(LIGHT_CYAN)], c(21, 63, 63));
        assert_eq!(p[usize::from(YELLOW)], c(63, 63, 21));
        assert_eq!(p[usize::from(WHITE)], c(63, 63, 63));
        assert_eq!((MAGENTA, GREEN, CYAN, RED), (5, 2, 3, 4));
        assert_eq!(
            (LIGHT_BLUE, LIGHT_GREEN, LIGHT_RED, LIGHT_MAGENTA),
            (9, 10, 12, 13)
        );
    }
}
