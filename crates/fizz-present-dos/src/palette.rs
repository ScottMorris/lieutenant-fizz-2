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

/// Where the dimmed copies start: entries 16 to 31 are entries 0 to 15 at 4/13 of their brightness.
pub const DIM_BASE: u8 = 16;
/// Fizz 1's DOS port draws its HUD panel over the picture lowered by nine of its fourteen brightness levels, which leaves level
/// 5 of 14. Its palette holds a level as `(level - 1) / 13` of the colour, so the panel shows 4/13 (about 0.31) of each channel.
pub const DIM_NUMERATOR: u16 = 4;

/// The palette index of the dimmed copy of EGA colour `c` (0 to 15): the HUD box shows the picture through itself in these.
pub const fn dimmed(c: u8) -> u8 {
    DIM_BASE + (c & 15)
}

const fn dim(v: u8) -> u8 {
    ((v as u16 * DIM_NUMERATOR + 6) / 13) as u8
}

/// The whole table the DOS display is given: the sixteen EGA colours (entries 0 to 15, unchanged), then the same sixteen at
/// 4/13 of their brightness (entries 16 to 31, rounded to the nearest DAC step). The frame stays one byte per pixel, and
/// every picture drawn so far uses only entries 0 to 15; only the HUD box puts the dimmed ones on screen.
pub const fn dos_palette() -> [Rgb6; 32] {
    let ega = ega_palette();
    let mut out = [c(0, 0, 0); 32];
    let mut i = 0;
    while i < 16 {
        out[i] = ega[i];
        out[16 + i] = c(dim(ega[i].r), dim(ega[i].g), dim(ega[i].b));
        i += 1;
    }
    out
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
    fn the_dos_table_keeps_the_ega_colours_and_adds_dimmed_copies() {
        let (ega, all) = (ega_palette(), dos_palette());
        assert_eq!(&all[..16], &ega[..]);
        for (i, e) in ega.iter().enumerate() {
            let d = all[usize::from(dimmed(i as u8))];
            assert_eq!(d, all[16 + i]);
            for (full, dim) in [(e.r, d.r), (e.g, d.g), (e.b, d.b)] {
                assert!(dim <= 19 && dim <= full);
                // Within half a DAC step of 4/13 of the colour.
                assert!((i32::from(dim) * 13 - i32::from(full) * 4).abs() <= 6);
            }
        }
        assert_eq!(all[16], c(0, 0, 0));
        assert_eq!(all[31], c(19, 19, 19));
        assert_eq!(all[usize::from(dimmed(LIGHT_GREEN))], c(6, 19, 6));
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
