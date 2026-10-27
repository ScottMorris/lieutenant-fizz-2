// Tile-to-pixel conversion, the y-flip and the stateless follow camera shared by both presenters.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::pixel;

/// The side of a tile in logical pixels.
pub const TILE_PX: i32 = 16;

/// The logical pixel that contains `v` tiles: `pixel(v * 16.0)`.
///
/// The multiplication by 16 is exact in `f64`, so the floor is taken of the true position.
pub fn tile_px(v: f64) -> i32 {
    pixel(v * 16.0)
}

/// The screen row of a world height.
///
/// The rules are y-up (a larger `y` is higher) and the screen is y-down. `world_y` is in tiles and `map_h_tiles` is the map
/// height in tiles. The height is floored to a pixel first, then flipped, so `screen_y(y, h) == h * 16 - tile_px(y)` and a
/// sprite at an integer tile boundary sits exactly on the tile edge. The result is a row of the whole map, measured from its
/// top; subtract the camera's top to get a row of the view. Arithmetic saturates.
pub fn screen_y(world_y: f64, map_h_tiles: i32) -> i32 {
    map_h_tiles
        .saturating_mul(TILE_PX)
        .saturating_sub(tile_px(world_y))
}

/// The origin (left or top) of a view of `view_px` pixels that follows `target_px` inside a map of `map_px` pixels.
///
/// All values are whole logical pixels along one axis; call it once per axis, fresh every frame. It has no state and no
/// easing (the eased camera is Milestone 3). The target is centred, then the origin is clamped so the view stays inside
/// the map: the result is in `0..=map_px - view_px`. A map no larger than the view cannot scroll, so it is centred in the
/// view and the origin is `(map_px - view_px) / 2` rounded down, which is zero or negative. A negative `map_px` counts as
/// zero and a non-positive `view_px` gives zero. The arithmetic is 64-bit, so no input overflows.
pub fn follow(target_px: i32, map_px: i32, view_px: i32) -> i32 {
    if view_px <= 0 {
        return 0;
    }
    let map = i64::from(map_px.max(0));
    let view = i64::from(view_px);
    let slack = map - view;
    if slack <= 0 {
        return slack.div_euclid(2) as i32;
    }
    let centred = i64::from(target_px) - view / 2;
    centred.clamp(0, slack) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_convert_to_pixels_by_floor() {
        assert_eq!(tile_px(0.0), 0);
        assert_eq!(tile_px(1.0), 16);
        assert_eq!(tile_px(1.0 / 16.0), 1);
        assert_eq!(tile_px(1.0 / 16.0 - 1e-9), 0);
        assert_eq!(tile_px(-1.0 / 16.0), -1);
        assert_eq!(tile_px(-0.5), -8);
        assert_eq!(tile_px(f64::NAN), 0);
        assert_eq!(tile_px(1.0e300), i32::MAX);
        assert_eq!(tile_px(-1.0e300), i32::MIN);
    }

    #[test]
    fn the_flip_puts_the_map_bottom_at_the_bottom_row() {
        // A 4 tile high map is 64 rows: height 0 is row 64, height 4 is row 0.
        assert_eq!(screen_y(0.0, 4), 64);
        assert_eq!(screen_y(4.0, 4), 0);
        assert_eq!(screen_y(1.0, 4), 48);
        assert_eq!(screen_y(1.0 / 16.0, 4), 63);
        assert_eq!(screen_y(-1.0, 4), 80);
    }

    #[test]
    fn the_flip_is_monotone_and_saturates() {
        let mut prev = screen_y(-3.0, 8);
        let mut y = -3.0;
        while y < 10.0 {
            y += 0.0137;
            let s = screen_y(y, 8);
            assert!(s <= prev, "{y}");
            prev = s;
        }
        assert_eq!(screen_y(-1.0e300, 8), i32::MAX);
        assert_eq!(screen_y(1.0e300, 8), 128 - i32::MAX);
        assert_eq!(screen_y(1.0e300, i32::MAX), 0);
        assert_eq!(screen_y(f64::NAN, 8), 128);
    }

    #[test]
    fn follow_centres_the_target_inside_the_map() {
        assert_eq!(follow(500, 1000, 200), 400);
        assert_eq!(follow(101, 1000, 200), 1);
        assert_eq!(follow(100, 1000, 200), 0);
        assert_eq!(follow(101, 1000, 201), 1);
    }

    #[test]
    fn follow_clamps_at_both_map_edges() {
        assert_eq!(follow(0, 1000, 200), 0);
        assert_eq!(follow(50, 1000, 200), 0);
        assert_eq!(follow(-5000, 1000, 200), 0);
        assert_eq!(follow(999, 1000, 200), 800);
        assert_eq!(follow(5000, 1000, 200), 800);
        assert_eq!(follow(900, 1000, 200), 800);
        assert_eq!(follow(899, 1000, 200), 799);
    }

    #[test]
    fn a_map_no_larger_than_the_view_is_centred() {
        assert_eq!(follow(10, 200, 200), 0);
        assert_eq!(follow(10, 100, 200), -50);
        assert_eq!(follow(9999, 100, 200), -50);
        assert_eq!(follow(0, 101, 200), -50);
        assert_eq!(follow(0, 0, 200), -100);
    }

    #[test]
    fn follow_is_safe_for_negative_and_extreme_input() {
        assert_eq!(follow(5, -10, 200), -100);
        assert_eq!(follow(5, 1000, 0), 0);
        assert_eq!(follow(5, 1000, -7), 0);
        assert_eq!(follow(i32::MIN, i32::MAX, 200), 0);
        assert_eq!(follow(i32::MAX, i32::MAX, 200), i32::MAX - 200);
        assert_eq!(follow(i32::MAX, i32::MAX, i32::MAX), 0);
        assert_eq!(follow(0, i32::MIN, i32::MAX), -1_073_741_824);
        assert_eq!(follow(0, i32::MAX, i32::MIN), 0);
    }

    #[test]
    fn follow_always_keeps_the_view_inside_or_centred() {
        for map in [-3, 0, 50, 199, 200, 201, 320, 1000] {
            for view in [-1, 0, 1, 200, 320] {
                for target in (-400..1400).step_by(37) {
                    let o = follow(target, map, view);
                    let (m, v) = (i64::from(map.max(0)), i64::from(view));
                    if v <= 0 {
                        assert_eq!(o, 0);
                    } else if m > v {
                        assert!((0..=m - v).contains(&i64::from(o)), "{target} {map} {view}");
                    } else {
                        assert_eq!(i64::from(o), (m - v).div_euclid(2));
                    }
                }
            }
        }
    }
}
