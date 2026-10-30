// The DOS slice's in-picture HUD: a small translucent two-row box in the top-left corner, cached as a palette-indexed overlay strip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// It follows the box Fizz 1's DOS port draws (`hud()` in its shell): a panel with a light grey outer edge and a dark grey inner
// edge, and rows of text in Fizz 1's proportional pixel font with a one-pixel black shadow. The port dims the picture beneath
// (`ui::dim`); here the DOS palette holds a dimmed copy of each EGA colour in entries 16 to 31 (see [`crate::dos_palette`]),
// so the panel turns each picture pixel it covers into its dimmed counterpart and the scene shows through, darkened.
//
// The layout is the Two-Row Stack. The top row is `Fizz` and its number (light magenta, Fizz 2's accent, `fizz_view::theme`;
// light red at zero; outlined in black so it reads over any picture) and `Score` with five yellow digits whose leading zeros are
// dark grey. For a second after a snack the word `Score` gives way to the yellow `+N`. The bottom row is `Deaths` and its number
// and the key tray: one 7 by 7 socket for each kind of key the level has, empty (a dark grey outline) until the key is picked up,
// when it fills with the key's colour and a white ring shows for a moment, and duplicates counted by white pips under the socket
// (see `fizz_slice_view::hud`, which holds the model and the animation both presenters share).
//
// The box is a pill like the Web HUD's: its corners are rounded by [`RADIUS`] pixels, the edge follows the curve, and the pixels
// beyond the curve are [`OUTSIDE`], which leaves the picture as it is. Its size never changes.
//
// The strip is an overlay: the edges, text and sockets in their own colours and [`TRANSPARENT`] everywhere else. It is rendered only
// when the view differs from the one it was rendered for (a count-up step, a pickup, a ring ending); every frame composites its
// rows over the picture, which is redrawn from scratch each frame (a strip pixel replaces the picture's, a transparent one dims
// it). Nothing is drawn outside [`X`], [`Y`], [`W`] by [`H`].

use fizz_slice_view::hud::{Extra, KEY_KINDS, KEY_SLOTS, SCORE_DIGITS, extra, score_digits};
use fizz_view::pfont::{LINE, draw_text, text_width};
use fizz_view::sprites_data::sprite;

use fizz_view::theme::FIZZ_INDEX;

use crate::palette::{BLACK, DARK_GREY, DIM_BASE, LIGHT_GREY, LIGHT_RED, WHITE, YELLOW};

/// What the HUD is drawn from: the shared view of the world (`fizz_slice_view::HudView`).
pub type HudValues = fizz_slice_view::HudView;

/// The box's left edge in the picture, in pixels. A multiple of four, so rows composite whole words.
pub const X: usize = 4;
/// The box's top edge in the picture, in pixels.
pub const Y: usize = 4;
/// The box's width: `Fizz 99`, `Score` and five digits on the top row, `Deaths 99` and four sockets on the bottom. A multiple of four.
pub const W: usize = 120;
/// The box's height: two rows of text between the edges.
pub const H: usize = 27;

/// The strip value that means "the picture, dimmed". It is no palette index the strip draws with.
pub const TRANSPARENT: u8 = 0xFF;
/// The strip value for a pixel beyond a rounded corner: the picture shows there undimmed.
pub const OUTSIDE: u8 = 0xFE;
/// The corner radius of the box's outer edge, in pixels; the dark inner edge is one pixel inside it with a radius one smaller.
pub const RADIUS: usize = 5;

/// Where the rows of text start inside the strip.
const TEXT_X: i32 = 5;
const ROW1_Y: i32 = 4;
const ROW2_Y: i32 = ROW1_Y + LINE;
/// The left edge of the `Score` column, and of its five digits, each in a cell [`DIGIT_CELL`] pixels wide.
const SCORE_X: i32 = 50;
const DIGITS_X: i32 = SCORE_X + 33;
const DIGIT_CELL: i32 = 6;
/// The tray: the left edge of the first socket, the distance between sockets and the top of the 7 by 7 key. Beyond the right end of
/// `Deaths 99` (which ends at 56) by the width of the ring.
const TRAY_X: i32 = 60;
const SOCKET_PITCH: i32 = 10;
const SOCKET_Y: i32 = 15;
/// The row of pips under a socket, two pixels apart.
const PIPS_Y: i32 = SOCKET_Y + 8;
const PIP_PITCH: i32 = 2;

/// The cached HUD strip and the view it shows.
pub struct Hud {
    strip: [u8; W * H],
    /// The strip as a mask, four pixels to a word: a picture word becomes `(word & keep) | set`. An overlay pixel keeps nothing
    /// and sets its colour, a transparent one keeps the colour's low nibble and sets the dimmed half's bit, an outside one keeps all.
    keep: [u32; W / 4 * H],
    set: [u32; W / 4 * H],
    shown: Option<HudValues>,
    renders: u32,
}

impl Hud {
    pub const fn new() -> Self {
        Self {
            strip: [TRANSPARENT; W * H],
            keep: [0; W / 4 * H],
            set: [0; W / 4 * H],
            shown: None,
            renders: 0,
        }
    }

    /// How many times the strip has been rendered (not copied) since this cache was made.
    pub fn renders(&self) -> u32 {
        self.renders
    }

    /// Renders the strip if `values` differ from the ones it shows.
    pub fn update(&mut self, values: HudValues) {
        if self.shown != Some(values) {
            render(&mut self.strip, &values);
            for (i, px) in self.strip.as_chunks::<4>().0.iter().enumerate() {
                let (mut keep, mut set) = ([0u8; 4], [0u8; 4]);
                for (k, &p) in px.iter().enumerate() {
                    (keep[k], set[k]) = match p {
                        TRANSPARENT => (0x0F, DIM_BASE),
                        OUTSIDE => (0xFF, 0),
                        _ => (0, p),
                    };
                }
                self.keep[i] = u32::from_le_bytes(keep);
                self.set[i] = u32::from_le_bytes(set);
            }
            self.shown = Some(values);
            self.renders += 1;
        }
    }

    /// Updates the strip for `values`, then composites it over the `pitch` pixels wide picture `pixels` at (`X`, `Y`): strip
    /// pixels replace the picture's, and where the strip is transparent the picture's colour becomes its dimmed copy (entry
    /// 16 + colour). A picture too small to hold the box (its width under `X + W` or too few rows) is left alone.
    pub fn draw(&mut self, values: HudValues, pixels: &mut [u8], pitch: usize) {
        if pitch < X + W || pixels.len() < (Y + H) * pitch {
            return;
        }
        self.update(values);
        const WORDS: usize = W / 4;
        for row in 0..H {
            let at = (Y + row) * pitch + X;
            let masks = self.keep[row * WORDS..][..WORDS]
                .iter()
                .zip(&self.set[row * WORDS..][..WORDS]);
            for (dst, (&keep, &set)) in pixels[at..at + W]
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(masks)
            {
                let word = u32::from_le_bytes([dst[0], dst[1], dst[2], dst[3]]);
                dst.copy_from_slice(&((word & keep) | set).to_le_bytes());
            }
        }
    }
}

impl Default for Hud {
    fn default() -> Self {
        Self::new()
    }
}

/// Draws `text` with a one-pixel black shadow down and to the right, and returns where the next text starts.
fn shadowed(strip: &mut [u8], x: i32, y: i32, text: &str, colour: u8) -> i32 {
    draw_text(strip, W, H, x + 1, y + 1, text, BLACK);
    draw_text(strip, W, H, x, y, text, colour);
    x + text_width(text)
}

/// Draws `text` ringed in black on all eight sides, so the ink has black beside it whatever the picture is, and returns where the
/// next text starts.
fn outlined(strip: &mut [u8], x: i32, y: i32, text: &str, colour: u8) -> i32 {
    for (dx, dy) in [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        draw_text(strip, W, H, x + dx, y + dy, text, BLACK);
    }
    draw_text(strip, W, H, x, y, text, colour);
    x + text_width(text)
}

/// `n` in decimal in a fixed buffer, at most three digits.
fn decimal(n: u32, buf: &mut [u8; 3]) -> &str {
    let n = n.min(999);
    let len = if n >= 100 {
        3
    } else if n >= 10 {
        2
    } else {
        1
    };
    for (i, b) in buf[..len].iter_mut().enumerate() {
        let place = 10u32.pow((len - 1 - i) as u32);
        *b = b'0' + (n / place % 10) as u8;
    }
    core::str::from_utf8(&buf[..len]).unwrap_or("")
}

/// Whether pixel (`x`, `y`) of the strip is inside the box's rounded outline shrunk by `inset` pixels on every side (the corner
/// radius shrinks with it, so the outlines are concentric).
fn in_pill(x: usize, y: usize, inset: usize) -> bool {
    if x < inset || y < inset || x >= W - inset || y >= H - inset {
        return false;
    }
    let r = RADIUS.saturating_sub(inset);
    // Distance from the pixel's centre to the centre of the nearest corner's circle, in half pixels.
    let near = |v: usize, len: usize| {
        let (lo, hi) = (inset + r, len - inset - r);
        if v < lo {
            (2 * lo - 2 * v - 1) as i32
        } else if v >= hi {
            (2 * v + 1 - 2 * hi) as i32
        } else {
            0
        }
    };
    let (dx, dy) = (near(x, W), near(y, H));
    dx * dx + dy * dy <= (2 * r * 2 * r) as i32
}

/// How deep pixel (`x`, `y`) of the strip is in the pill: 0 outside it, 1 on the outer edge, 2 on the inner edge, 3 open. Only the
/// four corner squares need the curve; everywhere else the depth is the distance to the nearest side.
fn depth(x: usize, y: usize) -> u8 {
    let (cx, cy) = (x.min(W - 1 - x), y.min(H - 1 - y));
    if cx >= RADIUS || cy >= RADIUS {
        return (cx.min(cy) + 1).min(3) as u8;
    }
    (0..3).find(|&d| !in_pill(x, y, d)).unwrap_or(3) as u8
}

fn render(strip: &mut [u8; W * H], v: &HudValues) {
    // The light grey outer edge, the dark grey inner edge inside it, and the open middle, as in Fizz 1, on a rounded outline.
    for y in 0..H {
        for x in 0..W {
            strip[y * W + x] = match depth(x, y) {
                0 => OUTSIDE,
                1 => LIGHT_GREY,
                2 => DARK_GREY,
                _ => TRANSPARENT,
            };
        }
    }
    let mut buf = [0u8; 3];
    // Row 1: Fizz and its number, Score (or the +N of a snack just taken) and its five digits.
    let x = shadowed(strip, TEXT_X, ROW1_Y, "Fizz ", LIGHT_GREY);
    let colour = if v.fizz == 0 { LIGHT_RED } else { FIZZ_INDEX };
    outlined(strip, x, ROW1_Y, decimal(v.fizz, &mut buf), colour);
    if v.pop > 0 {
        let mut text = [b'+', 0, 0, 0];
        let n = decimal(v.pop.min(99), &mut buf).len();
        text[1..=n].copy_from_slice(&buf[..n]);
        let text = core::str::from_utf8(&text[..=n]).unwrap_or("+");
        shadowed(strip, DIGITS_X - 3 - text_width(text), ROW1_Y, text, YELLOW);
    } else {
        shadowed(strip, SCORE_X, ROW1_Y, "Score", LIGHT_GREY);
    }
    let (digits, pad) = score_digits(v.score);
    for (i, &d) in digits.iter().enumerate().take(SCORE_DIGITS) {
        let ch = [b'0' + d];
        let text = core::str::from_utf8(&ch).unwrap_or("0");
        let colour = if i < pad { DARK_GREY } else { YELLOW };
        shadowed(
            strip,
            DIGITS_X + i as i32 * DIGIT_CELL,
            ROW1_Y,
            text,
            colour,
        );
    }
    // Row 2: the deaths in the lives slot, and the key tray.
    let x = shadowed(strip, TEXT_X, ROW2_Y, "Deaths ", LIGHT_GREY);
    shadowed(strip, x, ROW2_Y, decimal(v.deaths, &mut buf), WHITE);
    let mut at = TRAY_X;
    for (kind, slot) in KEY_KINDS.iter().zip(&v.keys).take(KEY_SLOTS) {
        if slot.in_level {
            socket(strip, at, kind.sprite(), slot.held, slot.ring);
            at += SOCKET_PITCH;
        }
    }
}

/// A socket at column `x`: the key's colour with its mark when held, only its dark grey outline when not; a white ring when it was
/// just picked up; and white pips under it for duplicates, or the count over the key past four.
fn socket(strip: &mut [u8; W * H], x: i32, name: &str, held: u8, ring: bool) {
    let Some(key) = sprite(name) else { return };
    let (kw, kh) = (usize::from(key.w), usize::from(key.h));
    let body = |sx: i32, sy: i32| {
        sx >= 0
            && sy >= 0
            && (sx as usize) < kw
            && (sy as usize) < kh
            && key.pixels[sy as usize * kw + sx as usize] != TRANSPARENT
    };
    let mut put = |sx: i32, sy: i32, colour: u8| {
        let (px, py) = (x + sx, SOCKET_Y + sy);
        if (0..W as i32).contains(&px) && (0..H as i32).contains(&py) {
            strip[py as usize * W + px as usize] = colour;
        }
    };
    if ring && held > 0 {
        for sy in -1..=kh as i32 {
            for sx in -1..=kw as i32 {
                let near = (-1..=1).any(|dy| (-1..=1).any(|dx| body(sx + dx, sy + dy)));
                if near && !body(sx, sy) {
                    put(sx, sy, WHITE);
                }
            }
        }
    }
    for sy in 0..kh as i32 {
        for sx in 0..kw as i32 {
            if !body(sx, sy) {
                continue;
            }
            if held > 0 {
                put(sx, sy, key.pixels[sy as usize * kw + sx as usize]);
            } else if [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| !body(sx + dx, sy + dy))
            {
                // The empty socket is the key's silhouette: its edge pixels, in dark grey.
                put(sx, sy, DARK_GREY);
            }
        }
    }
    match extra(held) {
        Extra::None => {}
        Extra::Pips(n) => {
            for p in 0..i32::from(n) {
                put(p * PIP_PITCH, PIPS_Y - SOCKET_Y, WHITE);
            }
        }
        Extra::Digit(n) => {
            let ch = [b'0' + n];
            let text = core::str::from_utf8(&ch).unwrap_or("9");
            let (px, py) = (x + (i32::from(key.w) - 4) / 2, SOCKET_Y - 1);
            shadowed(strip, px, py, text, WHITE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use crate::{HEIGHT, WIDTH};
    use fizz_slice_view::hud::{KeySlot, MAX_PIPS};
    use std::vec;

    fn vals(fizz: u32, score: u32, deaths: u32, held: [u8; 2]) -> HudValues {
        let mut keys = [KeySlot::default(); KEY_SLOTS];
        for (k, h) in keys.iter_mut().zip(held) {
            *k = KeySlot {
                in_level: true,
                held: h,
                ring: false,
            };
        }
        HudValues {
            fizz,
            score,
            pop: 0,
            deaths,
            keys,
        }
    }

    fn plain(fizz: u32, deaths: u32) -> HudValues {
        vals(fizz, 0, deaths, [0, 0])
    }

    fn count(strip: &[u8], colour: u8) -> usize {
        strip.iter().filter(|&&p| p == colour).count()
    }

    fn rendered(v: &HudValues) -> [u8; W * H] {
        let mut s = [0u8; W * H];
        render(&mut s, v);
        // A render writes every pixel of the strip, so the starting contents never matter.
        let mut t = [0x5au8; W * H];
        render(&mut t, v);
        assert_eq!(s, t);
        s
    }

    /// The strip of `v` with all four sockets in the level.
    fn all_four(mut v: HudValues) -> HudValues {
        for k in &mut v.keys {
            k.in_level = true;
        }
        v
    }

    #[test]
    fn the_fizz_number_is_grape_and_turns_light_red_at_zero() {
        let five = rendered(&plain(5, 0));
        assert!(count(&five, FIZZ_INDEX) > 0);
        assert_eq!(count(&five, LIGHT_RED), 0);
        let none = rendered(&plain(0, 0));
        assert!(count(&none, LIGHT_RED) > 0);
        assert_eq!(count(&none, FIZZ_INDEX), 0);
        // One shot short of empty is still grape.
        assert_eq!(count(&rendered(&plain(1, 0)), LIGHT_RED), 0);
    }

    #[test]
    fn the_fizz_number_is_ringed_in_black() {
        for fizz in [0, 5, 99] {
            let s = rendered(&plain(fizz, 0));
            let ink = if fizz == 0 { LIGHT_RED } else { FIZZ_INDEX };
            let mut lit = 0;
            for y in 0..H as i32 {
                for x in 0..W as i32 {
                    if s[y as usize * W + x as usize] != ink {
                        continue;
                    }
                    lit += 1;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let p = s[(y + dy) as usize * W + (x + dx) as usize];
                            assert!(p == ink || p == BLACK, "fizz {fizz}: ({x},{y}) beside {p}");
                        }
                    }
                }
            }
            assert!(lit > 10);
        }
    }

    #[test]
    fn the_score_is_five_yellow_digits_with_dim_leading_zeros() {
        let zero = rendered(&vals(5, 0, 0, [0, 0]));
        // Four padding zeros in dark grey and one real zero in yellow, so each colour appears; the Fizz and edges use the grey too.
        let mut digits = [0u8; W * H];
        for (i, ch) in ["0", "0", "0", "0"].iter().enumerate() {
            draw_text(
                &mut digits,
                W,
                H,
                DIGITS_X + i as i32 * DIGIT_CELL,
                ROW1_Y,
                ch,
                DARK_GREY,
            );
        }
        for (i, &p) in digits.iter().enumerate() {
            if p == DARK_GREY {
                assert_eq!(zero[i], DARK_GREY);
            }
        }
        assert!(count(&zero, YELLOW) > 5);
        let full = rendered(&vals(5, 12_345, 0, [0, 0]));
        let full_yellow = count(&full, YELLOW);
        assert!(full_yellow > count(&zero, YELLOW) * 3);
        // The cap: 99999 and anything over it draw the same.
        assert_eq!(
            rendered(&vals(5, 99_999, 0, [0, 0])),
            rendered(&vals(5, 4_000_000, 0, [0, 0]))
        );
        // The last digit is where the cell says: changing it changes only pixels in its cell.
        let a = rendered(&vals(5, 10, 0, [0, 0]));
        let b = rendered(&vals(5, 11, 0, [0, 0]));
        for (i, (p, q)) in a.iter().zip(&b).enumerate() {
            if p != q {
                let x = (i % W) as i32;
                assert!(
                    (DIGITS_X + 4 * DIGIT_CELL..DIGITS_X + 5 * DIGIT_CELL + 1).contains(&x),
                    "{x}"
                );
            }
        }
    }

    #[test]
    fn a_snack_pop_takes_the_place_of_the_word_score() {
        let mut v = vals(5, 3, 0, [0, 0]);
        let without = rendered(&v);
        v.pop = 5;
        let with = rendered(&v);
        assert_ne!(without, with);
        // Nothing left of the word Score is drawn in light grey in that slot, and the +N is yellow.
        let slot = |s: &[u8], colour: u8| {
            (ROW1_Y as usize..ROW2_Y as usize)
                .flat_map(|y| (SCORE_X as usize..DIGITS_X as usize).map(move |x| (x, y)))
                .filter(|&(x, y)| s[y * W + x] == colour)
                .count()
        };
        assert!(slot(&without, LIGHT_GREY) > 20);
        assert_eq!(slot(&with, LIGHT_GREY), 0);
        assert!(slot(&with, YELLOW) > 5 && slot(&without, YELLOW) == 0);
        // The pop never reaches the digits.
        let mut big = v;
        big.pop = 4_000;
        let s = rendered(&big);
        for y in 0..H {
            for x in (DIGITS_X as usize - 2)..DIGITS_X as usize {
                assert_ne!(s[y * W + x], YELLOW, "({x},{y})");
            }
        }
    }

    #[test]
    fn the_sockets_are_empty_outlines_until_the_key_is_held() {
        let empty = rendered(&plain(5, 0));
        let red = fizz_view::sprites_data::sprite("hudkey_red").unwrap();
        let edge = |s: &[u8], x0: i32| {
            (0..7)
                .flat_map(|sy| (0..7).map(move |sx| (sx, sy)))
                .filter(|&(sx, sy)| {
                    s[(SOCKET_Y + sy) as usize * W + (x0 + sx) as usize] == DARK_GREY
                })
                .count()
        };
        // The empty socket is the silhouette's edge in dark grey: no fill, no mark.
        assert!(edge(&empty, TRAY_X) >= 16);
        assert!(count(&empty, LIGHT_RED) == 0);
        let held = rendered(&vals(5, 0, 0, [1, 0]));
        assert_eq!(edge(&held, TRAY_X), 0);
        let body = red.pixels.iter().filter(|&&p| p == LIGHT_RED).count();
        assert_eq!(count(&held, LIGHT_RED), body);
        // The second socket did not change.
        for sy in 0..7 {
            for sx in 0..7 {
                let i = (SOCKET_Y + sy) as usize * W + (TRAY_X + SOCKET_PITCH + sx) as usize;
                assert_eq!(held[i], empty[i]);
            }
        }
        // The socket marks are black pixels inside the body, so the key is not told apart by colour alone.
        let at = |x: i32, y: i32| held[(SOCKET_Y + y) as usize * W + (TRAY_X + x) as usize];
        assert_eq!(at(3, 3), BLACK);
    }

    #[test]
    fn the_tray_has_a_socket_only_for_kinds_the_level_has_and_packs_them_left() {
        let mut none = plain(5, 0);
        for k in &mut none.keys {
            k.in_level = false;
        }
        let bare = rendered(&none);
        assert_eq!(count(&bare, DARK_GREY), count(&rendered(&none), DARK_GREY));
        // One kind in the level (the blue one): its socket is the first in the tray.
        let mut blue_only = none;
        blue_only.keys[1].in_level = true;
        blue_only.keys[1].held = 1;
        let s = rendered(&blue_only);
        let blue = fizz_view::sprites_data::sprite("hudkey_blue").unwrap();
        for sy in 0..7usize {
            for sx in 0..7usize {
                let want = blue.pixels[sy * 7 + sx];
                if want != 255 {
                    assert_eq!(s[(SOCKET_Y as usize + sy) * W + TRAY_X as usize + sx], want);
                }
            }
        }
        // All four kinds fit in the box, the last socket ending inside the inner edge.
        let last = TRAY_X + 3 * SOCKET_PITCH + 7 + 1;
        assert!(last < W as i32 - 2);
        let four = rendered(&all_four(plain(5, 0)));
        assert!(count(&four, DARK_GREY) > count(&rendered(&plain(5, 0)), DARK_GREY));
    }

    #[test]
    fn a_pickup_rings_the_key_in_white_and_the_ring_goes_with_it() {
        let held = vals(5, 0, 0, [1, 0]);
        let mut ringed = held;
        ringed.keys[0].ring = true;
        let a = rendered(&held);
        let b = rendered(&ringed);
        assert!(count(&b, WHITE) > count(&a, WHITE) + 15);
        // Only pixels around the key change, and none inside its silhouette.
        let red = fizz_view::sprites_data::sprite("hudkey_red").unwrap();
        for (i, (p, q)) in a.iter().zip(&b).enumerate() {
            if p != q {
                let (x, y) = ((i % W) as i32 - TRAY_X, (i / W) as i32 - SOCKET_Y);
                assert!((-1..=7).contains(&x) && (-1..=7).contains(&y), "({x},{y})");
                if (0..7).contains(&x) && (0..7).contains(&y) {
                    assert_eq!(red.pixels[(y * 7 + x) as usize], 255);
                }
                assert_eq!(*q, WHITE);
            }
        }
        // An empty socket does not ring.
        let mut empty = plain(5, 0);
        empty.keys[0].ring = true;
        assert_eq!(rendered(&empty), rendered(&plain(5, 0)));
    }

    #[test]
    fn duplicates_show_as_pips_under_their_socket_and_a_digit_past_four() {
        let pips = |held: u8| {
            let s = rendered(&vals(5, 0, 0, [held, 0]));
            (0..W as i32)
                .filter(|&x| s[PIPS_Y as usize * W + x as usize] == WHITE)
                .collect::<std::vec::Vec<_>>()
        };
        assert!(pips(0).is_empty() && pips(1).is_empty());
        assert_eq!(pips(2), [TRAY_X, TRAY_X + 2]);
        assert_eq!(pips(3), [TRAY_X, TRAY_X + 2, TRAY_X + 4]);
        assert_eq!(pips(4).len(), usize::from(MAX_PIPS));
        // The pips stay inside the box's inner edge.
        assert!(PIPS_Y < H as i32 - 2);
        // Five or more: a digit over the key instead of pips.
        assert!(pips(5).is_empty());
        let five = rendered(&vals(5, 0, 0, [5, 0]));
        let one = rendered(&vals(5, 0, 0, [1, 0]));
        assert_ne!(five, one);
        assert!(count(&five, WHITE) > count(&one, WHITE));
    }

    #[test]
    fn the_text_is_the_pixel_font_with_a_black_shadow() {
        let s = rendered(&plain(5, 7));
        // The label and number lit pixels are where the pixel font puts them.
        let mut label = [0u8; W * H];
        draw_text(&mut label, W, H, TEXT_X, ROW2_Y, "Deaths ", LIGHT_GREY);
        for (i, &p) in label.iter().enumerate() {
            if p == LIGHT_GREY {
                assert_eq!(s[i], LIGHT_GREY);
            }
        }
        let mut num = [0u8; W * H];
        draw_text(
            &mut num,
            W,
            H,
            TEXT_X + text_width("Deaths "),
            ROW2_Y,
            "7",
            WHITE,
        );
        assert_eq!(count(&s, WHITE), count(&num, WHITE));
        // The proportional face is narrower than the 8 by 8 capitals the old bar used for the same words.
        assert!(text_width("Deaths 99") < 10 * 8);
    }

    #[test]
    fn the_widest_values_fit_in_the_box_and_clear_the_columns_beside_them() {
        // Fizz 99 ends before the Score column, and Deaths 99 before the tray's first ring.
        let fizz = TEXT_X + text_width("Fizz ") + text_width("99");
        assert!(fizz < SCORE_X, "{fizz} runs into Score");
        let deaths = TEXT_X + text_width("Deaths ") + text_width("99");
        assert!(deaths < TRAY_X - 1, "{deaths} runs into the tray");
        // The five digits end inside the inner edge, each digit inside its cell.
        for d in '0'..='9' {
            assert!(text_width(&std::format!("{d}")) <= DIGIT_CELL + 1, "{d}");
        }
        assert!(DIGITS_X + SCORE_DIGITS as i32 * DIGIT_CELL <= W as i32 - 3);
        // The word Score and its gap fill the 33 pixels before the digits, so the digits sit where the pop is right-aligned.
        assert_eq!(text_width("Score "), DIGITS_X - SCORE_X);
        // Descenders of the second row stay inside the inner edge.
        assert!(ROW2_Y + 9 <= H as i32 - 2);
        // Values above the cap read as the cap.
        let mut a = [0u8; 3];
        assert_eq!(decimal(0, &mut a), "0");
        assert_eq!(decimal(40, &mut a), "40");
        assert_eq!(decimal(99, &mut a), "99");
    }

    #[test]
    fn the_strip_renders_only_when_a_value_changes_and_again_when_it_changes_back() {
        // The picture is redrawn from scratch each frame, as the presenter does.
        let mut hud = Hud::new();
        let mut pic = vec![7u8; WIDTH * HEIGHT];
        let mut frame = |hud: &mut Hud, v: HudValues| {
            pic.fill(7);
            hud.draw(v, &mut pic, WIDTH);
            pic.clone()
        };
        for _ in 0..3 {
            frame(&mut hud, plain(5, 0));
        }
        assert_eq!(hud.renders(), 1);
        let first = frame(&mut hud, plain(5, 0));
        let second = frame(&mut hud, plain(4, 0));
        assert_eq!(hud.renders(), 2);
        assert_ne!(second, first);
        frame(&mut hud, vals(4, 0, 0, [1, 0]));
        frame(&mut hud, vals(4, 1, 0, [1, 0]));
        assert_eq!(hud.renders(), 4);
        // Back to the first values: rendered afresh, and the picture is the first one again (no stale pixels).
        let again = frame(&mut hud, plain(5, 0));
        assert_eq!(hud.renders(), 5);
        assert_eq!(again, first);
    }

    #[test]
    fn the_cached_draw_equals_a_fresh_render() {
        let mut hud = Hud::new();
        let mut a = vec![3u8; WIDTH * HEIGHT];
        let mut ringed = vals(0, 40, 12, [1, 2]);
        ringed.keys[0].ring = true;
        ringed.pop = 5;
        for v in [plain(5, 0), ringed, vals(99, 99_999, 99, [5, 1])] {
            a.fill(3);
            hud.draw(v, &mut a, WIDTH);
            let mut b = vec![3u8; WIDTH * HEIGHT];
            Hud::new().draw(v, &mut b, WIDTH);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn the_hud_never_draws_outside_its_box() {
        for (pitch, rows) in [(320usize, 200usize), (X + W, Y + H), (400, 240), (640, 360)] {
            for v in [plain(5, 0), all_four(vals(0, 99_999, 99, [4, 1]))] {
                let mut pic = vec![3u8; pitch * rows];
                Hud::new().draw(v, &mut pic, pitch);
                for (i, &p) in pic.iter().enumerate() {
                    let (x, y) = (i % pitch, i / pitch);
                    let inside = (X..X + W).contains(&x) && (Y..Y + H).contains(&y);
                    assert!(inside || p == 3, "({x},{y}) in {pitch}x{rows}");
                    // Every pixel of the pill changes (dimmed or overlay); only the corners' outside is left alone.
                    if inside {
                        assert_eq!(
                            p == 3,
                            !in_pill(x - X, y - Y, 0),
                            "({x},{y}) in {pitch}x{rows}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_picture_shows_through_the_box_dimmed_and_the_overlay_stays_in_full_colour() {
        let mut ringed = all_four(vals(0, 1240, 7, [2, 1]));
        ringed.keys[1].ring = true;
        ringed.pop = 5;
        for v in [plain(5, 0), ringed, vals(99, 99_999, 99, [5, 1])] {
            let strip = rendered(&v);
            // A picture of vertical bars of all sixteen colours.
            let mut pic = vec![0u8; WIDTH * HEIGHT];
            for (i, p) in pic.iter_mut().enumerate() {
                *p = ((i % WIDTH) / 5 % 16) as u8;
            }
            let before = pic.clone();
            Hud::new().draw(v, &mut pic, WIDTH);
            let (mut dimmed_px, mut overlay_px, mut outside_px) = (0, 0, 0);
            for row in 0..H {
                for col in 0..W {
                    let at = (Y + row) * WIDTH + X + col;
                    let o = strip[row * W + col];
                    if o == OUTSIDE {
                        assert_eq!(pic[at], before[at], "({col},{row})");
                        outside_px += 1;
                    } else if o == TRANSPARENT {
                        assert_eq!(pic[at], DIM_BASE + before[at], "({col},{row})");
                        dimmed_px += 1;
                    } else {
                        assert_eq!(pic[at], o, "({col},{row})");
                        assert!(o < 16, "the overlay draws with the sixteen colours");
                        overlay_px += 1;
                    }
                }
            }
            assert!(dimmed_px > W * H / 2 && overlay_px > 200);
            // Four rounded corners, and nothing else is outside the pill.
            assert!((4 * 3..4 * 14).contains(&outside_px), "{outside_px}");
            for (x, y) in [(0, 0), (W - 1, 0), (0, H - 1), (W - 1, H - 1)] {
                assert_eq!(strip[y * W + x], OUTSIDE);
            }
            // The double edge: light grey outside, dark grey inside it, along the straight sides.
            for col in RADIUS..W - RADIUS {
                assert_eq!(strip[col], LIGHT_GREY);
                assert_eq!(strip[(H - 1) * W + col], LIGHT_GREY);
                assert_eq!(strip[W + col], DARK_GREY);
            }
            for row in RADIUS..H - RADIUS {
                assert_eq!(strip[row * W], LIGHT_GREY);
                assert_eq!(strip[row * W + W - 1], LIGHT_GREY);
                assert_eq!(strip[row * W + 1], DARK_GREY);
            }
            // Round the curve too: every pill pixel that touches the outside is the light grey edge, and no open pixel
            // touches the outside (the edge follows the curve, with the dark edge inside it).
            for y in 0..H {
                for x in 0..W {
                    let nb = |dx: i32, dy: i32| {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
                            OUTSIDE
                        } else {
                            strip[ny as usize * W + nx as usize]
                        }
                    };
                    let touches = |what: u8| {
                        [(1, 0), (-1, 0), (0, 1), (0, -1)]
                            .iter()
                            .any(|&(dx, dy)| nb(dx, dy) == what)
                    };
                    if strip[y * W + x] != OUTSIDE && touches(OUTSIDE) {
                        assert_eq!(strip[y * W + x], LIGHT_GREY, "({x},{y})");
                    }
                }
            }
        }
    }

    /// The WCAG contrast of `ink` over each of the sixteen dimmed picture colours (the worst) and over black.
    #[allow(clippy::float_arithmetic)]
    fn contrast(ink: u8) -> (f64, f64) {
        let table = crate::dos_palette();
        let lum = |c: fizz_hal::Rgb6| {
            let f = |v: u8| (f64::from(v) / 63.0).powf(2.2);
            0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
        };
        let ratio = |a: f64, b: f64| (a.max(b) + 0.05) / (a.min(b) + 0.05);
        let worst = (0..16u8)
            .map(|bg| {
                ratio(
                    lum(table[usize::from(ink)]),
                    lum(table[usize::from(DIM_BASE + bg)]),
                )
            })
            .fold(f64::MAX, f64::min);
        (worst, ratio(lum(table[usize::from(ink)]), lum(table[0])))
    }

    #[test]
    fn the_text_reads_over_every_picture_colour() {
        // The brightest thing behind the box is white or yellow at 4/13. Each ink is measured over the worst dimmed picture
        // colour, and the Fizz number, which has a black ring all round, also over that black.
        let floor = 3.0;
        // Grape over the worst dimmed colour is 3.28; light red over dimmed yellow is only 2.74, under the floor, which the
        // outline lifts: the ring is what its ink sits against, 6.66 over black.
        assert!(contrast(FIZZ_INDEX).0 > floor, "{}", contrast(FIZZ_INDEX).0);
        assert!(contrast(FIZZ_INDEX).1 > floor);
        assert!(
            contrast(LIGHT_RED).0 < floor,
            "the unringed case really is under"
        );
        assert!(contrast(LIGHT_RED).1 > floor, "{}", contrast(LIGHT_RED).1);
        assert!(contrast(LIGHT_RED).1 > 6.5);
        // The grey labels, white deaths, yellow score and white pips and ring.
        assert!(contrast(LIGHT_GREY).0 > 3.5, "{}", contrast(LIGHT_GREY).0);
        assert!(contrast(WHITE).0 > 7.0, "{}", contrast(WHITE).0);
        assert!(contrast(YELLOW).0 > 7.0, "{}", contrast(YELLOW).0);
        // The leading zeros are meant to be recessive, not read: well under the floor, on purpose.
        assert!(contrast(DARK_GREY).0 < 2.0);
    }

    #[test]
    fn a_picture_too_small_for_the_box_is_left_alone() {
        let mut hud = Hud::new();
        let mut narrow = vec![9u8; (X + W - 1) * 100];
        hud.draw(plain(1, 1), &mut narrow, X + W - 1);
        assert!(narrow.iter().all(|&p| p == 9));
        let mut short = vec![9u8; 320 * (Y + H - 1)];
        hud.draw(plain(1, 1), &mut short, 320);
        assert!(short.iter().all(|&p| p == 9));
        assert_eq!(hud.renders(), 0);
    }

    #[test]
    fn the_box_is_small_and_stays_in_the_corner() {
        // 120 by 27 is 3,240 pixels, 5.1 per cent of the picture, in the top left corner.
        const { assert!(X + W <= WIDTH * 2 / 5 && Y + H <= HEIGHT / 6) };
        const { assert!(X.is_multiple_of(4) && W.is_multiple_of(4)) };
    }
}
