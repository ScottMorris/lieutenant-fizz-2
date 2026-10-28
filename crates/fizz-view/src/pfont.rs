// Draws text in Fizz 1's proportional pixel font (mixed case, with descenders) into a palette-indexed frame.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The glyph data is Fizz 1's own, exported by `tools/font/export-fizz1.ts` into `fizz_font_data.rs`. Capitals are seven rows
// tall and descenders take two more, so a line of text is [`LINE`] pixels apart. Each glyph advances by its width and one pixel
// of air, a space is three pixels, and a character the font lacks takes the width of a space and draws nothing, as Fizz 1's
// DOS port does. The 8 by 8 font in `font.rs` is the HUD's and is not replaced by this one.

use crate::fizz_font_data::{FONT, Glyph};

/// The tallest glyph: seven rows of capital and two of descender.
pub const ROWS: usize = 9;
/// The distance between lines of text: the glyph rows and one row of air.
pub const LINE: i32 = 10;
/// The width of a space, and of a character the font lacks.
const SPACE: i32 = 3;

/// The glyph for `c`, or `None` when the font lacks it (a space has none either; it only advances).
pub fn glyph(c: char) -> Option<&'static Glyph> {
    FONT.binary_search_by_key(&(c as u32), |g| g.cp)
        .ok()
        .map(|i| &FONT[i])
}

/// How far the pen moves for `c`: its width and one pixel, or three for a space or a character the font lacks.
pub fn advance(c: char) -> i32 {
    if c == ' ' {
        SPACE
    } else {
        glyph(c).map_or(SPACE, |g| i32::from(g.w) + 1)
    }
}

/// The width in pixels of `text` drawn by [`draw_text`]: every advance, so it includes the pixel of air after the last glyph.
pub fn text_width(text: &str) -> i32 {
    text.chars().fold(0i32, |w, c| w.saturating_add(advance(c)))
}

/// Draws `text` with its top-left corner at (`x`, `y`) into the `w` by `h` frame, clipped to it. Set bits take `colour`; clear
/// bits are left alone. Any position is safe, and a frame shorter than `w * h` is treated as only as tall as it is.
pub fn draw_text(frame: &mut [u8], w: usize, h: usize, x: i32, y: i32, text: &str, colour: u8) {
    let h = frame.len().checked_div(w).map_or(0, |rows| rows.min(h));
    let mut pen = x;
    for c in text.chars() {
        let step = advance(c);
        if let Some(g) = glyph(c) {
            let gw = i32::from(g.w);
            // Skip glyphs wholly off to a side; the per-pixel test below clips the rest.
            if i64::from(pen) < w as i64 && i64::from(pen) + i64::from(gw) > 0 {
                for (dy, &bits) in g.rows.iter().enumerate() {
                    let py = i64::from(y) + dy as i64;
                    if bits == 0 || py < 0 || py >= h as i64 {
                        continue;
                    }
                    for dx in 0..gw {
                        if bits >> (gw - 1 - dx) & 1 != 0 {
                            let px = i64::from(pen) + i64::from(dx);
                            if px >= 0 && px < w as i64 {
                                frame[py as usize * w + px as usize] = colour;
                            }
                        }
                    }
                }
            }
        }
        pen = pen.saturating_add(step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_font_is_sorted_and_every_glyph_fits_nine_rows() {
        assert!(FONT.windows(2).all(|p| p[0].cp < p[1].cp));
        for g in FONT {
            assert!(g.rows.len() <= ROWS && g.w <= 16, "{}", g.cp);
            assert!(g.rows.iter().any(|&r| r != 0), "{}", g.cp);
            assert!(g.rows.iter().all(|&r| u32::from(r) < 1 << g.w), "{}", g.cp);
        }
    }

    #[test]
    fn every_character_the_captions_and_toasts_need_is_present() {
        // Fizz 1's sound captions and toasts use these (its dashes are the em dash and the quotes are straight).
        for c in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-,.!?:'\"\u{2014}"
            .chars()
        {
            assert!(glyph(c).is_some(), "{c}");
        }
        assert_eq!(advance(' '), 3);
        assert_eq!(advance('~'), glyph('~').map_or(3, |g| i32::from(g.w) + 1));
    }

    #[test]
    fn lower_case_differs_from_capitals_and_descenders_go_below_the_baseline() {
        for (l, c) in ('a'..='z').zip('A'..='Z') {
            let (gl, gc) = (glyph(l).unwrap(), glyph(c).unwrap());
            assert!(gl.w != gc.w || gl.rows != gc.rows, "{l}");
            assert!(gl.rows.len() <= ROWS);
        }
        for c in "gjpqy".chars() {
            assert!(glyph(c).unwrap().rows.len() > 7, "{c}");
        }
        assert_eq!(glyph('a').unwrap().rows.len(), 7);
    }

    #[test]
    fn widths_are_proportional_and_text_clips() {
        assert_eq!(text_width(""), 0);
        assert_eq!(text_width("a b"), advance('a') + 3 + advance('b'));
        assert!(text_width("i") < text_width("m"));
        let mut a = [0u8; 40 * 12];
        draw_text(&mut a, 40, 12, 2, 1, "bwuk", 15);
        assert!(a.contains(&15));
        // Far outside, an empty frame, a zero width and a short buffer are all safe.
        draw_text(&mut a, 40, 12, i32::MAX, i32::MIN, "bwuk", 1);
        draw_text(&mut a, 40, 12, -1000, 0, "bwuk", 1);
        draw_text(&mut [], 0, 0, 0, 0, "A", 1);
        draw_text(&mut a, 0, 12, 0, 0, "A", 1);
        draw_text(&mut a[..50], 40, 12, 0, 0, "gy", 1);
        // Drawn clipped on the left it equals the unclipped picture shifted.
        let mut b = [0u8; 40 * 12];
        let mut c = [0u8; 40 * 12];
        draw_text(&mut b, 40, 12, 5, 0, "bwuk", 15);
        draw_text(&mut c, 40, 12, 1, 0, "bwuk", 15);
        for y in 0..12 {
            for x in 0..35 {
                assert_eq!(b[y * 40 + x + 4], c[y * 40 + x], "{x},{y}");
            }
        }
    }
}
