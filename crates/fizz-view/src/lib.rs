// Turns the rules' positions into the logical pixels that every presenter places things at.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![forbid(unsafe_code)]

//! The rules say where things are, in game units. Each presenter places them in its own environment: the Web scales the
//! logical pixels to its canvas and DOS writes them into the VGA frame. This crate holds the one step they share, the
//! choice of which logical pixel a position falls in, so the two screens never differ by a pixel and neither presenter
//! depends on the other.

/// Generated EGA-indexed sprite data, emitted from `assets/sprites/` by `tools/sprites/emit-rust.ts`.
#[rustfmt::skip]
pub mod sprites_data;

mod camera;
mod draw;

pub use camera::{TILE_PX, follow, screen_y, tile_px};
pub use draw::{TRANSPARENT, blit_sprite, draw_tilemap, fill_rect};

/// The logical pixel that contains `v`: the floor of `v`.
///
/// Truncation toward zero would put both -0.5 and 0.4 on pixel 0 and make a position that crosses zero hitch for a frame,
/// so negative values round down. Values beyond the `i32` range saturate, and NaN is pixel 0.
pub fn pixel(v: f64) -> i32 {
    let t = v as i32;
    if v < f64::from(t) {
        t.saturating_sub(1)
    } else {
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_fall_in_the_pixel_that_contains_them() {
        for (v, expected) in [
            (0.0, 0),
            (0.4, 0),
            (0.5, 0),
            (0.999, 0),
            (1.0, 1),
            (10.5, 10),
            (160.5, 160),
            (312.0, 312),
            (312.999, 312),
        ] {
            assert_eq!(pixel(v), expected, "{v}");
        }
    }

    #[test]
    fn negative_positions_round_down() {
        for (v, expected) in [
            (-0.0, 0),
            (-0.5, -1),
            (-0.999, -1),
            (-1.0, -1),
            (-1.000_001, -2),
            (-2.5, -3),
        ] {
            assert_eq!(pixel(v), expected, "{v}");
        }
    }

    #[test]
    fn a_position_is_never_below_its_pixel_or_a_whole_pixel_above_it() {
        let mut v = -300.0;
        while v < 300.0 {
            let p = f64::from(pixel(v));
            assert!(p <= v && v < p + 1.0, "{v} fell in pixel {p}");
            v += 0.173;
        }
    }

    #[test]
    fn extreme_values_saturate_and_nan_is_zero() {
        assert_eq!(pixel(1.0e12), i32::MAX);
        assert_eq!(pixel(-1.0e12), i32::MIN);
        assert_eq!(pixel(f64::INFINITY), i32::MAX);
        assert_eq!(pixel(f64::NEG_INFINITY), i32::MIN);
        assert_eq!(pixel(f64::NAN), 0);
    }
}

#[cfg(test)]
mod sprite_tests {
    use crate::sprites_data::{SPRITES, sprite};

    #[test]
    fn sprites_are_found_by_name() {
        for name in [
            "ben_stand",
            "ben_pogo2",
            "keyRed",
            "doorRed",
            "bubble",
            "benA_idle",
            "cluckzarg_alert",
        ] {
            assert_eq!(sprite(name).map(|s| s.name), Some(name));
        }
        assert!(sprite("no_such_sprite").is_none());
    }

    #[test]
    fn names_are_unique() {
        for (i, a) in SPRITES.iter().enumerate() {
            assert!(
                SPRITES[i + 1..].iter().all(|b| b.name != a.name),
                "{}",
                a.name
            );
        }
    }

    #[test]
    fn pixel_counts_match_sizes_and_indices_are_ega_or_transparent() {
        for s in SPRITES {
            assert_eq!(
                s.pixels.len(),
                usize::from(s.w) * usize::from(s.h),
                "{}",
                s.name
            );
            assert!(s.pixels.iter().all(|&p| p <= 15 || p == 255), "{}", s.name);
            assert!(s.pixels.iter().any(|&p| p != 255), "{} is blank", s.name);
        }
    }

    #[test]
    fn fizz1_sizes_and_anchors_are_as_the_catalog_draws_them() {
        for (name, w, h) in [
            ("ben_stand", 16, 24),
            ("ben_pogo", 16, 32),
            ("keyRed", 16, 16),
            ("bubble", 8, 8),
        ] {
            let s = sprite(name).unwrap();
            assert_eq!((s.w, s.h), (w, h), "{name}");
            assert_eq!(
                (i16::from(s.ax), i16::from(s.ay)),
                (i16::from(w) / 2, i16::from(h)),
                "{name}"
            );
        }
    }

    fn row(name: &str, y: usize) -> &'static [u8] {
        let s = sprite(name).unwrap();
        let w = usize::from(s.w);
        &s.pixels[y * w..(y + 1) * w]
    }

    #[test]
    fn the_squash_frame_is_two_rows_lower_than_the_pogo_frame() {
        // Fizz 1's `ben('pogo2')` draws the body at an offset of 2 (characters.ts): the helmet's top rows, rows 0 and 1 of
        // `ben_pogo`, are rows 2 and 3 of `ben_pogo2`, and the first row of `ben_pogo2` is clear.
        assert_eq!(row("ben_pogo2", 2), row("ben_pogo", 0));
        assert_eq!(row("ben_pogo2", 3), row("ben_pogo", 1));
        assert!(row("ben_pogo2", 0).iter().all(|&p| p == 255));
    }
}
