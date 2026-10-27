// Holds an original 8x8 bitmap font and draws text with it into a palette-indexed frame, for every presenter.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Each glyph is eight rows from top to bottom; bit 7 of a row is the leftmost pixel. The font began in the DOS presenter
// (`fizz-present-dos`) and moved here, unchanged, so the slice view can draw in-world captions with it; the DOS HUD draws
// through the same code.

/// Glyph cell size in pixels.
pub const GLYPH: usize = 8;

const DIGITS: [[u8; 8]; 10] = [
    [0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00],
    [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
    [0x3C, 0x66, 0x06, 0x0C, 0x18, 0x30, 0x7E, 0x00],
    [0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00],
    [0x0C, 0x1C, 0x3C, 0x6C, 0x7E, 0x0C, 0x0C, 0x00],
    [0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00],
    [0x1C, 0x30, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00],
    [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x00],
    [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00],
    [0x3C, 0x66, 0x66, 0x3E, 0x06, 0x0C, 0x38, 0x00],
];

const LETTERS: [[u8; 8]; 26] = [
    [0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x00],
    [0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00],
    [0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00],
    [0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00],
    [0x7E, 0x60, 0x60, 0x78, 0x60, 0x60, 0x7E, 0x00],
    [0x7E, 0x60, 0x60, 0x78, 0x60, 0x60, 0x60, 0x00],
    [0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3E, 0x00],
    [0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00],
    [0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
    [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38, 0x00],
    [0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00],
    [0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00],
    [0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00],
    [0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66, 0x66, 0x00],
    [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
    [0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00],
    [0x3C, 0x66, 0x66, 0x66, 0x6A, 0x6C, 0x36, 0x00],
    [0x7C, 0x66, 0x66, 0x7C, 0x6C, 0x66, 0x66, 0x00],
    [0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00],
    [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
    [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
    [0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00],
    [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00],
    [0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00],
    [0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00],
    [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E, 0x00],
];

const BLANK: [u8; 8] = [0; 8];
const COLON: [u8; 8] = [0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00];
const PERIOD: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00];
const SLASH: [u8; 8] = [0x02, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00];
const BANG: [u8; 8] = [0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00];
const COMMA: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30];
const HYPHEN: [u8; 8] = [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00];

/// Returns the glyph rows for `ch`, or `None` when the font has no such character.
/// Lower-case letters share the upper-case shapes.
pub fn glyph(ch: char) -> Option<&'static [u8; 8]> {
    match ch {
        '0'..='9' => Some(&DIGITS[ch as usize - '0' as usize]),
        'A'..='Z' => Some(&LETTERS[ch as usize - 'A' as usize]),
        'a'..='z' => Some(&LETTERS[ch as usize - 'a' as usize]),
        ' ' => Some(&BLANK),
        ':' => Some(&COLON),
        '.' => Some(&PERIOD),
        '/' => Some(&SLASH),
        '-' => Some(&HYPHEN),
        '!' => Some(&BANG),
        ',' => Some(&COMMA),
        _ => None,
    }
}

/// Draws one glyph with its top-left corner at (`x`, `y`) into the `w` by `h` frame, clipped to it. Set bits take `colour`;
/// clear bits are left alone. Characters the font lacks draw nothing. Any position is safe, and a frame shorter than
/// `w * h` is treated as only as tall as it is.
pub fn draw_glyph(frame: &mut [u8], w: usize, h: usize, x: i32, y: i32, ch: char, colour: u8) {
    let Some(rows) = glyph(ch) else { return };
    let h = if w == 0 { 0 } else { (frame.len() / w).min(h) };
    let cell = GLYPH as i64;
    if x >= 0 && y >= 0 && i64::from(x) + cell <= w as i64 && i64::from(y) + cell <= h as i64 {
        // The whole cell is inside the frame, so a row is one slice and no pixel needs its own clip test.
        for (dy, &bits) in rows.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            let start = (y as usize + dy) * w + x as usize;
            for (dx, pixel) in frame[start..start + GLYPH].iter_mut().enumerate() {
                if bits & (0x80 >> dx) != 0 {
                    *pixel = colour;
                }
            }
        }
        return;
    }
    for (dy, &bits) in rows.iter().enumerate() {
        let py = i64::from(y) + dy as i64;
        if py < 0 || py >= h as i64 {
            continue;
        }
        for dx in 0..GLYPH {
            if bits & (0x80 >> dx) != 0 {
                let px = i64::from(x) + dx as i64;
                if px >= 0 && px < w as i64 {
                    frame[py as usize * w + px as usize] = colour;
                }
            }
        }
    }
}

/// Draws `text` left to right, one glyph cell per character, with the top-left corner at (`x`, `y`), clipped to the frame.
pub fn draw_text(frame: &mut [u8], w: usize, h: usize, x: i32, y: i32, text: &str, colour: u8) {
    let mut cx = x;
    for ch in text.chars() {
        draw_glyph(frame, w, h, cx, y, ch, colour);
        cx = cx.saturating_add(GLYPH as i32);
    }
}

/// The width in pixels of `text` drawn by [`draw_text`]: one cell per character.
pub fn text_width(text: &str) -> i32 {
    (text.chars().count() as i32).saturating_mul(GLYPH as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ:./-!, ";

    #[test]
    fn every_supported_character_has_a_glyph() {
        for &b in CHARS {
            assert!(glyph(b as char).is_some(), "{}", b as char);
        }
        assert_eq!(glyph('a'), glyph('A'));
        assert_eq!(glyph('~'), None);
    }

    #[test]
    fn glyphs_are_distinct_and_visible_ones_are_non_empty() {
        for (i, &a) in CHARS.iter().enumerate() {
            let ga = glyph(a as char).unwrap();
            if a != b' ' {
                assert!(ga.iter().any(|&r| r != 0));
            }
            for &b in &CHARS[i + 1..] {
                assert_ne!(
                    ga,
                    glyph(b as char).unwrap(),
                    "{} vs {}",
                    a as char,
                    b as char
                );
            }
        }
    }

    #[test]
    fn text_clips_to_the_frame_and_advances_a_cell_per_character() {
        let mut f = [0u8; 16 * 8];
        draw_text(&mut f, 16, 8, 0, 0, "A~A", 15);
        let mut g = [0u8; 16 * 8];
        draw_glyph(&mut g, 16, 8, 0, 0, 'A', 15);
        draw_glyph(&mut g, 16, 8, 16, 0, 'A', 15);
        assert_eq!(f, g);
        assert_eq!(text_width("A~A"), 24);
        // Far outside, an empty frame and a zero width are all safe.
        draw_text(&mut f, 16, 8, i32::MAX, i32::MIN, "AAA", 1);
        draw_text(&mut [], 0, 0, 0, 0, "A", 1);
        draw_text(&mut f, 0, 8, 0, 0, "A", 1);
        draw_text(&mut f[..20], 16, 8, 0, 0, "A", 1);
    }
}
