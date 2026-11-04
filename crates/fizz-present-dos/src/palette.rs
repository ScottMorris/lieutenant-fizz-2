// Defines the sixteen EGA colours as 6-bit VGA DAC values with named palette indices, the 256-entry tables the DAC is given, and
// palette fades.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_hal::Rgb6;
use fizz_slice_view::{DrawOptions, light, palette_for};

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

const fn rgb6(t: &[[u8; 3]; 256]) -> [Rgb6; 256] {
    let mut out = [c(0, 0, 0); 256];
    let mut i = 0;
    while i < 256 {
        out[i] = c(t[i][0], t[i][1], t[i][2]);
        i += 1;
    }
    out
}

/// The whole table the DOS display is given by day: `fizz_slice_view::light::DAY`, the shared 256 entries the Web shows too. Its
/// banks keep the sixteen EGA colours at entries 0 to 15, unchanged, and the same sixteen at 4/13 of their brightness at 16 to
/// 31 ([`DIM_BASE`], rounded to the nearest DAC step); the other banks are the further brightness levels the night uses
/// (`light`). The frame stays one byte per pixel, and every day picture uses only entries 0 to 15 and the HUD box 16 to 31.
pub const fn dos_palette() -> [Rgb6; 256] {
    rgb6(&light::DAY)
}

/// The table for a picture drawn with `options`: [`dos_palette`] by day, and at night the one whose bank 0 is the theme's night
/// ambient (`fizz_slice_view::palette_for`).
pub fn dos_palette_for(options: &DrawOptions) -> [Rgb6; 256] {
    rgb6(&palette_for(options))
}

/// `p` faded towards black: each 6-bit value times `step / steps`, rounded to the nearest step (`step` 0 is black, `steps` is
/// `p` itself). A fade or a full-screen dim is a remap of the 256 entries and 768 writes to the DAC, not a pass over 64,000
/// pixels, so the frame keeps the undimmed picture (fizz-dos's `docs/PERFORMANCE.md`, "full-screen dims and fades through the
/// palette"). `steps` of 0 is taken as 1.
pub fn faded(p: &[Rgb6; 256], step: u8, steps: u8) -> [Rgb6; 256] {
    let steps = u16::from(steps.max(1));
    let step = u16::from(step).min(steps);
    let f = |v: u8| ((u16::from(v) * step + steps / 2) / steps) as u8;
    let mut out = *p;
    for e in &mut out {
        *e = c(f(e.r), f(e.g), f(e.b));
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
    fn the_night_table_differs_from_the_day_s_in_bank_0_only() {
        let day = dos_palette();
        let night = dos_palette_for(&DrawOptions {
            night: true,
            ..DrawOptions::default()
        });
        assert_eq!(dos_palette_for(&DrawOptions::default()), day);
        assert_eq!(&night[16..], &day[16..]);
        assert!(
            night[..16]
                .iter()
                .zip(&day[..16])
                .skip(1)
                .all(|(n, d)| n != d)
        );
        // The table is the shared one, entry for entry, so the Web and DOS show a frame's bytes alike.
        for (d, t) in day.iter().zip(light::DAY.iter()) {
            assert_eq!([d.r, d.g, d.b], *t);
        }
    }

    #[test]
    fn a_fade_scales_every_entry_and_ends_at_black_and_the_palette() {
        let day = dos_palette();
        assert_eq!(faded(&day, 12, 12), day);
        assert!(faded(&day, 0, 12).iter().all(|e| *e == c(0, 0, 0)));
        let half = faded(&day, 6, 12);
        assert_eq!(half[15], c(32, 32, 32));
        assert_eq!(half[usize::from(BLUE)], c(0, 0, 21));
        // Every step darkens or keeps every channel, and none goes past the palette or below black.
        for s in 0..12u8 {
            let (a, b) = (faded(&day, s, 12), faded(&day, s + 1, 12));
            for (x, y) in a.iter().zip(&b) {
                assert!(x.r <= y.r && x.g <= y.g && x.b <= y.b);
            }
        }
        assert_eq!(faded(&day, 99, 0), day);
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
