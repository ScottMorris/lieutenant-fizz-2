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
    // The glyph is looked up once for both its drawing and its advance, and the pixel loop is 32-bit: this is the hot loop of a
    // caption on a 486, where `/BENCH` found six captions costing 3.8 ms a frame with the 64-bit arithmetic and two lookups.
    let (width, height) = (
        i32::try_from(w).unwrap_or(i32::MAX),
        i32::try_from(h).unwrap_or(i32::MAX),
    );
    let mut pen = x;
    for c in text.chars() {
        let g = glyph(c);
        let step = if c == ' ' {
            SPACE
        } else {
            g.map_or(SPACE, |g| i32::from(g.w) + 1)
        };
        if let Some(g) = g {
            let gw = i32::from(g.w);
            // Skip glyphs wholly off to a side; the per-pixel test below clips the rest. `pen` is inside (-gw, w) from here on.
            if pen < width && i64::from(pen) + i64::from(gw) > 0 {
                for (dy, &bits) in g.rows.iter().enumerate() {
                    let py = i64::from(y) + dy as i64;
                    if bits == 0 || py < 0 || py >= i64::from(height) {
                        continue;
                    }
                    let row = py as usize * w;
                    for dx in 0..gw {
                        if bits >> (gw - 1 - dx) & 1 != 0 {
                            let px = pen + dx;
                            if px >= 0 && px < width {
                                frame[row + px as usize] = colour;
                            }
                        }
                    }
                }
            }
        }
        pen = pen.saturating_add(step);
    }
}

/// The width in pixels of `text` drawn by [`draw_text_scaled`] at `k` times: every advance times `k`.
pub fn text_width_scaled(text: &str, k: i32) -> i32 {
    text_width(text).saturating_mul(k)
}

/// [`draw_text`] with every font pixel drawn as a `k` by `k` square (a headline), clipped to the frame like it. A `k` below one
/// draws nothing. With `k` of one it draws exactly what [`draw_text`] does.
#[allow(clippy::too_many_arguments)]
pub fn draw_text_scaled(
    frame: &mut [u8],
    w: usize,
    h: usize,
    x: i32,
    y: i32,
    text: &str,
    colour: u8,
    k: i32,
) {
    if k < 1 {
        return;
    }
    let h = frame.len().checked_div(w).map_or(0, |rows| rows.min(h));
    let mut pen = i64::from(x);
    for c in text.chars() {
        if let Some(g) = glyph(c) {
            let gw = i64::from(g.w);
            let k64 = i64::from(k);
            if pen < w as i64 && pen + gw * k64 > 0 {
                for (dy, &bits) in g.rows.iter().enumerate() {
                    if bits == 0 {
                        continue;
                    }
                    let top = i64::from(y) + dy as i64 * k64;
                    let (y0, y1) = (top.max(0), (top + k64).min(h as i64));
                    if y0 >= y1 {
                        continue;
                    }
                    for dx in 0..gw {
                        if bits >> (gw - 1 - dx) & 1 == 0 {
                            continue;
                        }
                        let left = pen + dx * k64;
                        let (x0, x1) = (left.max(0), (left + k64).min(w as i64));
                        if x0 >= x1 {
                            continue;
                        }
                        for py in y0..y1 {
                            let row = py as usize * w;
                            frame[row + x0 as usize..row + x1 as usize].fill(colour);
                        }
                    }
                }
            }
        }
        pen += i64::from(advance(c)) * i64::from(k);
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
    fn scaled_text_is_the_plain_text_with_every_pixel_a_square() {
        let (w, h) = (60usize, 30usize);
        let mut one = [0u8; 60 * 30];
        draw_text(&mut one, w, h, 3, 2, "Op", 9);
        let mut same = [0u8; 60 * 30];
        draw_text_scaled(&mut same, w, h, 3, 2, "Op", 9, 1);
        assert_eq!(one, same);
        let mut two = [0u8; 60 * 30];
        draw_text_scaled(&mut two, w, h, 3, 2, "Op", 9, 2);
        let mut plain = [0u8; 30 * 15];
        draw_text(&mut plain, 30, 15, 0, 0, "Op", 9);
        for y in 0..15 {
            for x in 0..30 {
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = (3 + x * 2 + dx, 2 + y * 2 + dy);
                    if sx < w && sy < h {
                        assert_eq!(two[sy * w + sx], plain[y * 30 + x], "{x},{y}");
                    }
                }
            }
        }
        assert_eq!(text_width_scaled("Op", 2), text_width("Op") * 2);
        // Off every side, with a zero or negative scale, an empty frame and a short buffer, nothing panics.
        for (x, y) in [
            (i32::MAX, 0),
            (0, i32::MAX),
            (i32::MIN, i32::MIN),
            (-9, -9),
            (50, 25),
        ] {
            draw_text_scaled(&mut two, w, h, x, y, "Op", 1, 3);
        }
        let before = two;
        draw_text_scaled(&mut two, w, h, 0, 0, "Op", 1, 0);
        draw_text_scaled(&mut two, w, h, 0, 0, "Op", 1, -2);
        assert_eq!(before, two);
        draw_text_scaled(&mut [], 0, 0, 0, 0, "A", 1, 2);
        draw_text_scaled(&mut two[..50], w, h, 0, 0, "gy", 1, 2);
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
