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
