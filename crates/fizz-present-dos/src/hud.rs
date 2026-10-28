// The DOS slice's in-picture HUD: a small dark box in the top-left corner, cached as a palette-indexed strip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// It follows the box Fizz 1's DOS port draws (`hud()` in its shell): a panel with a black outer edge and a grey inner edge,
// and rows of text in Fizz 1's proportional pixel font with a one-pixel black shadow. The port dims the picture beneath
// (`ui::dim`, which needs a palette of shades); this presenter has the sixteen EGA colours and no shades, so the panel is
// solid. The slice has three things to show: `Fizz` (the ammunition, which Fizz 1 calls Fizz) in light cyan and light red
// at zero, `Deaths`, and a chip for the red key: the key sprite when Ben carries it, its dim outline when he does not.
//
// The strip is rendered only when one of the three values differs from the ones it was rendered for; every frame just copies
// its rows into the picture, which is redrawn from scratch each frame. Nothing is drawn outside [`X`], [`Y`], [`W`] by [`H`].

use fizz_view::pfont::{LINE, draw_text, text_width};
use fizz_view::sprites_data::sprite;

use crate::palette::{BLACK, DARK_GREY, LIGHT_CYAN, LIGHT_GREY, LIGHT_RED, WHITE};

/// The box's left edge in the picture, in pixels.
pub const X: usize = 4;
/// The box's top edge in the picture, in pixels.
pub const Y: usize = 4;
/// The box's width, wide enough for `Deaths 99` and the key chip.
pub const W: usize = 72;
/// The box's height: two rows of text between the edges.
pub const H: usize = 27;

/// The largest number shown; a larger value reads as this one.
pub const MAX_SHOWN: u32 = 99;

/// Where the rows of text start inside the strip.
const TEXT_X: i32 = 5;
const ROW1_Y: i32 = 4;
const ROW2_Y: i32 = ROW1_Y + LINE;
/// The left edge of the key sprite's 16 by 16 cell inside the strip; its drawn part is 12 by 11 and sits in the middle.
const KEY_CELL_X: i32 = W as i32 - 16 - 3;
const KEY_CELL_Y: i32 = 5;

/// The three values the HUD shows, which are the whole of what its picture depends on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HudValues {
    pub ammo: u32,
    pub key: bool,
    pub deaths: u32,
}

/// The cached HUD strip and the values it shows.
pub struct Hud {
    strip: [u8; W * H],
    shown: Option<HudValues>,
    renders: u32,
}

impl Hud {
    pub const fn new() -> Self {
        Self {
            strip: [BLACK; W * H],
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
            render(&mut self.strip, values);
            self.shown = Some(values);
            self.renders += 1;
        }
    }

    /// Updates the strip for `values`, then copies it into the `pitch` pixels wide picture `pixels` at (`X`, `Y`). A picture too
    /// small to hold the box (its width under `X + W` or too few rows) is left alone.
    pub fn draw(&mut self, values: HudValues, pixels: &mut [u8], pitch: usize) {
        if pitch < X + W || pixels.len() < (Y + H) * pitch {
            return;
        }
        self.update(values);
        for (row, src) in self.strip.as_chunks::<W>().0.iter().enumerate() {
            let at = (Y + row) * pitch + X;
            pixels[at..at + W].copy_from_slice(src);
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

/// `n` in decimal, at most [`MAX_SHOWN`], in a fixed buffer.
fn digits(n: u32, buf: &mut [u8; 2]) -> &str {
    let n = n.min(MAX_SHOWN);
    let len = if n >= 10 { 2 } else { 1 };
    for (i, b) in buf[..len].iter_mut().enumerate() {
        let place = 10u32.pow((len - 1 - i) as u32);
        *b = b'0' + (n / place % 10) as u8;
    }
    core::str::from_utf8(&buf[..len]).unwrap_or("")
}

fn render(strip: &mut [u8; W * H], v: HudValues) {
    strip.fill(BLACK);
    // The grey inner edge; the black outer edge is the strip's own border of fill.
    for x in 1..W - 1 {
        strip[W + x] = DARK_GREY;
        strip[(H - 2) * W + x] = DARK_GREY;
    }
    for y in 1..H - 1 {
        strip[y * W + 1] = DARK_GREY;
        strip[y * W + W - 2] = DARK_GREY;
    }
    let mut buf = [0u8; 2];
    let x = shadowed(strip, TEXT_X, ROW1_Y, "Fizz ", LIGHT_GREY);
    let colour = if v.ammo == 0 { LIGHT_RED } else { LIGHT_CYAN };
    shadowed(strip, x, ROW1_Y, digits(v.ammo, &mut buf), colour);
    let x = shadowed(strip, TEXT_X, ROW2_Y, "Deaths ", LIGHT_GREY);
    shadowed(strip, x, ROW2_Y, digits(v.deaths, &mut buf), WHITE);
    key_chip(strip, v.key);
}

/// The key chip: Fizz 1's red key when carried, only its outline in dark grey when not.
fn key_chip(strip: &mut [u8; W * H], carried: bool) {
    let Some(key) = sprite("keyRed") else { return };
    if carried {
        fizz_view::blit_sprite(
            strip,
            W,
            H,
            key,
            KEY_CELL_X + i32::from(key.ax),
            KEY_CELL_Y + i32::from(key.ay),
            false,
        );
    } else {
        let kw = usize::from(key.w);
        for sy in 0..usize::from(key.h) {
            for sx in 0..kw {
                // The sprite's outline is its black pixels; its fill and highlights are what the chip leaves out.
                if key.pixels[sy * kw + sx] == BLACK {
                    strip[(KEY_CELL_Y as usize + sy) * W + KEY_CELL_X as usize + sx] = DARK_GREY;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use crate::{HEIGHT, WIDTH};
    use std::vec;

    fn vals(ammo: u32, key: bool, deaths: u32) -> HudValues {
        HudValues { ammo, key, deaths }
    }

    fn count(strip: &[u8], colour: u8) -> usize {
        strip.iter().filter(|&&p| p == colour).count()
    }

    fn rendered(v: HudValues) -> [u8; W * H] {
        let mut s = [0u8; W * H];
        render(&mut s, v);
        s
    }

    #[test]
    fn ammo_is_light_cyan_and_turns_light_red_at_zero() {
        let five = rendered(vals(5, false, 0));
        assert!(count(&five, LIGHT_CYAN) > 0);
        assert_eq!(count(&five, LIGHT_RED), 0);
        let none = rendered(vals(0, false, 0));
        assert!(count(&none, LIGHT_RED) > 0);
        assert_eq!(count(&none, LIGHT_CYAN), 0);
        // One shot short of empty is still cyan.
        assert_eq!(count(&rendered(vals(1, false, 0)), LIGHT_RED), 0);
    }

    #[test]
    fn the_key_chip_shows_the_sprite_when_carried_and_a_dim_outline_when_not() {
        let without = rendered(vals(5, false, 0));
        let with = rendered(vals(5, true, 0));
        let key = sprite("keyRed").unwrap();
        // The sprite brings its light red body; the outline is the sprite's black pixels drawn dark grey.
        assert!(count(&with, LIGHT_RED) > 20);
        assert_eq!(count(&without, LIGHT_RED), 0);
        let outline = key.pixels.iter().filter(|&&p| p == BLACK).count();
        assert_eq!(
            count(&without, DARK_GREY),
            count(&with, DARK_GREY) + outline
        );
        // Only the chip's cell differs between the two.
        for (i, (a, b)) in with.iter().zip(&without).enumerate() {
            if a != b {
                let (x, y) = ((i % W) as i32, (i / W) as i32);
                assert!((KEY_CELL_X..KEY_CELL_X + 16).contains(&x), "{x}");
                assert!((KEY_CELL_Y..KEY_CELL_Y + 16).contains(&y), "{y}");
            }
        }
    }

    #[test]
    fn the_text_is_the_pixel_font_with_a_black_shadow() {
        let s = rendered(vals(5, false, 7));
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
    fn the_widest_values_fit_in_the_box() {
        for label in ["Deaths ", "Fizz "] {
            let widest = TEXT_X + text_width(label) + text_width("99");
            assert!(
                widest <= KEY_CELL_X + 2,
                "{label}: {widest} runs into the key chip"
            );
        }
        // Descenders of the second row stay inside the inner edge.
        assert!(ROW2_Y + 9 <= H as i32 - 2);
        // Values above the cap read as the cap.
        let mut a = [0u8; 2];
        assert_eq!(digits(0, &mut a), "0");
        assert_eq!(digits(7, &mut a), "7");
        assert_eq!(digits(40, &mut a), "40");
        assert_eq!(digits(99, &mut a), "99");
        assert_eq!(digits(u32::MAX, &mut a), "99");
    }

    #[test]
    fn the_strip_renders_only_when_a_value_changes_and_again_when_it_changes_back() {
        let mut hud = Hud::new();
        let mut pic = vec![7u8; WIDTH * HEIGHT];
        for _ in 0..3 {
            hud.draw(vals(5, false, 0), &mut pic, WIDTH);
        }
        assert_eq!(hud.renders(), 1);
        let first = pic.clone();
        hud.draw(vals(4, false, 0), &mut pic, WIDTH);
        assert_eq!(hud.renders(), 2);
        assert_ne!(pic, first);
        hud.draw(vals(4, true, 0), &mut pic, WIDTH);
        hud.draw(vals(4, true, 1), &mut pic, WIDTH);
        assert_eq!(hud.renders(), 4);
        // Back to the first values: rendered afresh, and the picture is the first one again (no stale pixels).
        hud.draw(vals(5, false, 0), &mut pic, WIDTH);
        assert_eq!(hud.renders(), 5);
        assert_eq!(pic, first);
    }

    #[test]
    fn the_cached_draw_equals_a_fresh_render() {
        let mut hud = Hud::new();
        let mut a = vec![3u8; WIDTH * HEIGHT];
        for v in [vals(5, false, 0), vals(0, true, 12), vals(99, false, 99)] {
            hud.draw(v, &mut a, WIDTH);
            let mut b = vec![3u8; WIDTH * HEIGHT];
            Hud::new().draw(v, &mut b, WIDTH);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn the_hud_never_draws_outside_its_box() {
        for (pitch, rows) in [(320usize, 200usize), (X + W, Y + H), (400, 240), (640, 360)] {
            for v in [vals(5, false, 0), vals(0, true, 99)] {
                let mut pic = vec![200u8; pitch * rows];
                Hud::new().draw(v, &mut pic, pitch);
                for (i, &p) in pic.iter().enumerate() {
                    let (x, y) = (i % pitch, i / pitch);
                    let inside = (X..X + W).contains(&x) && (Y..Y + H).contains(&y);
                    assert!(inside || p == 200, "({x},{y}) in {pitch}x{rows}");
                    // The box is opaque: nothing of the picture shows through it.
                    assert!(!inside || p != 200, "({x},{y}) in {pitch}x{rows}");
                }
            }
        }
    }

    #[test]
    fn a_picture_too_small_for_the_box_is_left_alone() {
        let mut hud = Hud::new();
        let mut narrow = vec![9u8; (X + W - 1) * 100];
        hud.draw(vals(1, false, 1), &mut narrow, X + W - 1);
        assert!(narrow.iter().all(|&p| p == 9));
        let mut short = vec![9u8; 320 * (Y + H - 1)];
        hud.draw(vals(1, false, 1), &mut short, 320);
        assert!(short.iter().all(|&p| p == 9));
        assert_eq!(hud.renders(), 0);
    }

    #[test]
    fn the_box_is_small_and_stays_in_the_corner() {
        const { assert!(X + W <= WIDTH / 4 && Y + H <= HEIGHT / 6) };
    }
}
