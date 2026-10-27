// Owns the 320x200 indexed back buffer and draws rectangles, glyphs and text into it with clipping.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::font::{GLYPH, glyph};
use crate::{FRAME_BYTES, HEIGHT, WIDTH};

/// An indexed-colour back buffer in row-major order with a pitch of `WIDTH`.
pub struct Frame {
    pixels: [u8; FRAME_BYTES],
}

impl Frame {
    pub const fn new() -> Self {
        Self {
            pixels: [0; FRAME_BYTES],
        }
    }

    pub fn clear(&mut self, colour: u8) {
        self.pixels.fill(colour);
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Sets one pixel; coordinates outside the frame are ignored.
    pub fn put(&mut self, x: i32, y: i32, colour: u8) {
        if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
            self.pixels[y as usize * WIDTH + x as usize] = colour;
        }
    }

    /// Fills the `w` by `h` rectangle at (`x`, `y`), clipped to the frame.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, colour: u8) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = x.saturating_add(w).min(WIDTH as i32);
        let y1 = y.saturating_add(h).min(HEIGHT as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for row in y0..y1 {
            let start = row as usize * WIDTH;
            self.pixels[start + x0 as usize..start + x1 as usize].fill(colour);
        }
    }

    /// Draws the one-pixel outline of the `w` by `h` rectangle at (`x`, `y`), clipped to the frame.
    pub fn stroke_rect(&mut self, x: i32, y: i32, w: i32, h: i32, colour: u8) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.fill_rect(x, y, w, 1, colour);
        self.fill_rect(x, y + h - 1, w, 1, colour);
        self.fill_rect(x, y, 1, h, colour);
        self.fill_rect(x + w - 1, y, 1, h, colour);
    }

    /// Draws one glyph with its top-left corner at (`x`, `y`). Set bits take `colour`; clear bits are left alone. Characters
    /// the font lacks draw nothing.
    pub fn draw_glyph(&mut self, x: i32, y: i32, ch: char, colour: u8) {
        let Some(rows) = glyph(ch) else { return };
        for (dy, &bits) in rows.iter().enumerate() {
            for dx in 0..GLYPH {
                if bits & (0x80 >> dx) != 0 {
                    self.put(x + dx as i32, y + dy as i32, colour);
                }
            }
        }
    }

    /// Draws `text` left to right, one glyph cell per character.
    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, colour: u8) {
        let mut cx = x;
        for ch in text.chars() {
            self.draw_glyph(cx, y, ch, colour);
            cx = cx.saturating_add(GLYPH as i32);
        }
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(f: &Frame, x: usize, y: usize) -> u8 {
        f.pixels()[y * WIDTH + x]
    }

    #[test]
    fn a_new_frame_is_zeroed_and_clear_fills_it() {
        let mut f = Frame::new();
        assert_eq!(f.pixels().len(), FRAME_BYTES);
        assert!(f.pixels().iter().all(|&p| p == 0));
        f.clear(9);
        assert!(f.pixels().iter().all(|&p| p == 9));
    }

    #[test]
    fn fill_rect_clips_on_every_edge() {
        let mut f = Frame::new();
        f.fill_rect(-2, -2, 4, 4, 5);
        assert_eq!((px(&f, 0, 0), px(&f, 1, 1), px(&f, 2, 2)), (5, 5, 0));
        f.fill_rect(318, 198, 10, 10, 6);
        assert_eq!((px(&f, 319, 199), px(&f, 317, 199)), (6, 0));
        f.fill_rect(500, 500, 3, 3, 7);
        f.fill_rect(10, 10, 0, 5, 7);
        f.fill_rect(i32::MAX, 0, 5, 5, 7);
        assert_eq!(f.pixels().iter().filter(|&&p| p == 7).count(), 0);
        assert_eq!(f.pixels().iter().filter(|&&p| p != 0).count(), 4 + 4);
    }

    #[test]
    fn glyph_one_has_the_designed_pattern() {
        let mut f = Frame::new();
        f.draw_glyph(0, 0, '1', 15);
        let expected = [0x18u8, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00];
        for (y, bits) in expected.iter().enumerate() {
            for x in 0..8 {
                let want = if bits & (0x80 >> x) != 0 { 15 } else { 0 };
                assert_eq!(px(&f, x, y), want, "({x},{y})");
            }
        }
    }

    #[test]
    fn glyphs_and_text_clip_at_the_edges() {
        let mut f = Frame::new();
        // The hyphen is row 3, columns 1 to 6; at x = -4 only columns 0 and 1 of the frame remain lit.
        f.draw_glyph(-4, 0, '-', 15);
        let lit = f.pixels().iter().filter(|&&p| p == 15).count();
        assert_eq!(lit, 3);
        assert_eq!((px(&f, 0, 3), px(&f, 1, 3), px(&f, 2, 3)), (15, 15, 15));
        assert_eq!(px(&f, 3, 3), 0);

        let mut g = Frame::new();
        g.draw_glyph(316, 196, '-', 15);
        g.draw_text(312, 190, "HELLO", 15);
        g.draw_text(-100, -100, "NOTHING", 15);
        g.draw_text(0, 400, "NOTHING", 15);
        assert!(g.pixels().contains(&15));
    }

    #[test]
    fn text_advances_one_cell_per_character_and_skips_unknown() {
        let mut a = Frame::new();
        a.draw_text(0, 0, "A~A", 15);
        let mut b = Frame::new();
        b.draw_glyph(0, 0, 'A', 15);
        b.draw_glyph(16, 0, 'A', 15);
        assert_eq!(a.pixels(), b.pixels());
    }

    #[test]
    fn stroke_rect_draws_only_the_outline() {
        let mut f = Frame::new();
        f.stroke_rect(2, 2, 4, 4, 3);
        assert_eq!(f.pixels().iter().filter(|&&p| p == 3).count(), 12);
        assert_eq!((px(&f, 3, 3), px(&f, 2, 2), px(&f, 5, 5)), (0, 3, 3));
    }
}
