// Parallax layers: stars, clouds and horizon bands that scroll slower than the tiles, in integers, into a palette-indexed frame.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Fizz 1's layer vocabulary: a layer at factor `f` is offset by `camera * (1 - f)`, so `f = 0` stays fixed on the screen and
//! `f = 1` moves with the tiles. Here a factor is a fixed-point number out of 256 and a layer scrolls on each axis by its own
//! factor, so a screen pixel `(sx, sy)` shows layer position `(sx + shift_x, sy + shift_y)` with `shift = camera - offset`.
//!
//! Only the three kinds the Episode 2 slice uses exist: [`Layer::Stars`], [`Layer::Clouds`] and [`Layer::Band`] (a row of
//! stepped heights with a sprite on top and a fill below, which is both the far headlands and the sea). Every layer is a pure
//! function of the camera and a tick, with no float arithmetic: hashed grids and baked tables stand in for noise and sines.
//! The drawing starts from the clear colour (black) and writes only inside a clip rectangle, so a caller can draw the sky behind
//! open air and nowhere else.

/// The fixed-point one: a factor of 256 moves with the tiles.
pub const ONE: u16 = 256;

/// A tile in pixels, the column width of a [`Layer::Band`].
const TILE: i32 = 16;

const BLACK: u8 = 0;
const WHITE: u8 = 15;
const LIGHT_CYAN: u8 = 11;
const YELLOW: u8 = 14;

/// The layer's offset from the camera, `camera * (256 - f) / 256` rounded down: where the layer is drawn relative to the world,
/// as Fizz 1's `render.rs` offsets it.
pub fn offset(cam: i32, f: u16) -> i32 {
    let k = i64::from(ONE) - i64::from(f.min(ONE));
    (i64::from(cam) * k).div_euclid(i64::from(ONE)) as i32
}

/// How far a layer at factor `f` scrolls on the screen while the camera is at `cam`: `cam - offset(cam, f)`, so factor 0 is the
/// whole camera and factor 256 is zero.
pub fn shift(cam: i32, f: u16) -> i32 {
    cam.saturating_sub(offset(cam, f))
}

/// One layer. All positions are in layer space, which maps to the screen by [`shift`] on each axis.
#[derive(Clone, Copy, Debug)]
pub enum Layer<'a> {
    /// Points of white, light cyan and yellow, one in three 16 by 16 cells, two pixels square one time in five and one pixel
    /// otherwise. A star is drawn only where the frame is still the clear colour, so it never lies over another layer.
    Stars { fx: u16, fy: u16 },
    /// [`CLOUD`] sprites in cells 56 pixels (3.5 tiles) wide and 32 tall for `rows` rows from layer `y = 0`, each kept with
    /// 65% odds, placed up to 32 pixels (2 tiles) along its cell and 16 down, and drifting right by [`DRIFT_PER_75_TICKS`]
    /// pixels every 75 ticks (0.2 tiles a second).
    Clouds { fx: u16, fy: u16, rows: i32 },
    /// A band of 16 px columns. Column `ix` has `heights[ix mod len]` tiles of height above `base_y`; the 16 by 16 `top` sprite
    /// (255 transparent) is drawn with its row `surface_row` on that surface, and `fill` below it, down to the clip's bottom.
    Band {
        fx: u16,
        fy: u16,
        base_y: i32,
        heights: &'a [i8],
        top: &'a [u8; 256],
        surface_row: i32,
        fill: u8,
    },
}

/// Pixels a cloud drifts every 75 ticks: 4, which is 3.2 pixels a second (0.2 tiles) at 60 ticks a second.
pub const DRIFT_PER_75_TICKS: u32 = 4;

/// The cloud sprite's width and height.
pub const CLOUD_W: usize = 24;
pub const CLOUD_H: usize = 8;

/// The cloud: a B checker inside an ellipse, `((x - 11.5) / 12)^2 + ((y - 3.5) / 4)^2 <= 1` and `(x + y)` odd, 255 elsewhere.
/// The ellipse test is the integer form `(2x - 23)^2 + 9 (2y - 7)^2 <= 576`.
pub const CLOUD: [u8; CLOUD_W * CLOUD_H] = {
    let mut t = [255u8; CLOUD_W * CLOUD_H];
    let mut y = 0;
    while y < CLOUD_H {
        let mut x = 0;
        while x < CLOUD_W {
            let dx = 2 * x as i32 - 23;
            let dy = 2 * y as i32 - 7;
            if dx * dx + 9 * dy * dy <= 576 && (x + y) % 2 == 1 {
                t[y * CLOUD_W + x] = 1;
            }
            x += 1;
        }
        y += 1;
    }
    t
};

/// A 32-bit hash of a layer cell: the design prototype's `hsh`, wrapping, reduced to 16 bits.
fn hash(x: i32, y: i32, s: i32) -> u32 {
    let mut h = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add((s as u32).wrapping_mul(2_246_822_519));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) & 0xffff
}

/// A clip rectangle in frame pixels, `x0..x1` by `y0..y1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Clip {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

struct Frame<'a> {
    px: &'a mut [u8],
    w: i32,
    h: i32,
}

impl Frame<'_> {
    fn fill(&mut self, x: i32, y: i32, len: i32, c: u8, clip: Clip) {
        if y < clip.y0 || y >= clip.y1 || y < 0 || y >= self.h {
            return;
        }
        let a = x.max(clip.x0).max(0);
        let b = x.saturating_add(len).min(clip.x1).min(self.w);
        if a < b {
            let row = y as usize * self.w as usize;
            self.px[row + a as usize..row + b as usize].fill(c);
        }
    }

    /// Draws a row of a sprite, 255 transparent.
    fn sprite_row(&mut self, x: i32, y: i32, src: &[u8], clip: Clip) {
        if y < clip.y0 || y >= clip.y1 || y < 0 || y >= self.h {
            return;
        }
        let a = x.max(clip.x0).max(0);
        let b = x.saturating_add(src.len() as i32).min(clip.x1).min(self.w);
        if a >= b {
            return;
        }
        let row = y as usize * self.w as usize;
        let dst = &mut self.px[row + a as usize..row + b as usize];
        for (d, &s) in dst.iter_mut().zip(&src[(a - x) as usize..]) {
            if s != 255 {
                *d = s;
            }
        }
    }

    /// Writes `c` at a pixel inside the clip only where it is the clear colour.
    fn star(&mut self, x: i32, y: i32, c: u8, clip: Clip) {
        if x < clip.x0
            || x >= clip.x1
            || y < clip.y0
            || y >= clip.y1
            || x < 0
            || y < 0
            || x >= self.w
            || y >= self.h
        {
            return;
        }
        let i = y as usize * self.w as usize + x as usize;
        if self.px[i] == BLACK {
            self.px[i] = c;
        }
    }
}

/// Draws `layers`, back to front, over the whole `w` by `h` frame after clearing it to black. `cam_x` and `cam_y` are the view's
/// top left in map pixels; `tick` drives the drift of the clouds.
pub fn draw_layers(
    frame: &mut [u8],
    w: usize,
    h: usize,
    cam_x: i32,
    cam_y: i32,
    layers: &[Layer],
    tick: u32,
) {
    let clip = Clip {
        x0: 0,
        y0: 0,
        x1: w.min(i32::MAX as usize) as i32,
        y1: h.min(i32::MAX as usize) as i32,
    };
    draw_layers_clipped(frame, w, h, cam_x, cam_y, layers, tick, clip);
}

/// [`draw_layers`] into the part of the frame inside `clip` (cleared to black first); nothing outside it is written.
#[allow(clippy::too_many_arguments)]
pub fn draw_layers_clipped(
    frame: &mut [u8],
    w: usize,
    h: usize,
    cam_x: i32,
    cam_y: i32,
    layers: &[Layer],
    tick: u32,
    clip: Clip,
) {
    let rows = frame.len().checked_div(w).map_or(0, |r| r.min(h));
    let mut f = Frame {
        px: frame,
        w: w.min(i32::MAX as usize) as i32,
        h: rows.min(i32::MAX as usize) as i32,
    };
    let clip = Clip {
        x0: clip.x0.max(0),
        y0: clip.y0.max(0),
        x1: clip.x1.min(f.w),
        y1: clip.y1.min(f.h),
    };
    if clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
        return;
    }
    for y in clip.y0..clip.y1 {
        f.fill(clip.x0, y, clip.x1 - clip.x0, BLACK, clip);
    }
    // Camera values this far out show nothing but the layer's repeated pattern; clamping keeps the arithmetic in range.
    let (cam_x, cam_y) = (
        cam_x.clamp(-(1 << 24), 1 << 24),
        cam_y.clamp(-(1 << 24), 1 << 24),
    );
    for layer in layers {
        match *layer {
            Layer::Stars { fx, fy } => stars(&mut f, shift(cam_x, fx), shift(cam_y, fy), clip),
            Layer::Clouds { fx, fy, rows } => {
                clouds(&mut f, shift(cam_x, fx), shift(cam_y, fy), rows, tick, clip)
            }
            Layer::Band {
                fx,
                fy,
                base_y,
                heights,
                top,
                surface_row,
                fill,
            } => band(
                &mut f,
                shift(cam_x, fx),
                shift(cam_y, fy),
                base_y,
                heights,
                top,
                surface_row,
                fill,
                clip,
            ),
        }
    }
}

fn stars(f: &mut Frame, sx: i32, sy: i32, clip: Clip) {
    // A star lies up to 14 pixels into its cell and is at most 2 square, so the cells one before the clip can reach it.
    let (cx0, cx1) = (
        (clip.x0 + sx).div_euclid(16) - 1,
        (clip.x1 + sx).div_euclid(16),
    );
    let (cy0, cy1) = (
        (clip.y0 + sy).div_euclid(16) - 1,
        (clip.y1 + sy).div_euclid(16),
    );
    for cy in cy0..=cy1 {
        for cx in cx0..=cx1 {
            let h = hash(cx, cy, 31);
            if !h.is_multiple_of(3) {
                continue;
            }
            let colour = [WHITE, LIGHT_CYAN, YELLOW][((h >> 2) % 3) as usize];
            let x = cx * 16 + ((h >> 3) % 14) as i32 - sx;
            let y = cy * 16 + ((h >> 7) % 14) as i32 - sy;
            let size = if (h >> 9).is_multiple_of(5) { 2 } else { 1 };
            for dy in 0..size {
                for dx in 0..size {
                    f.star(x + dx, y + dy, colour, clip);
                }
            }
        }
    }
}

fn clouds(f: &mut Frame, sx: i32, sy: i32, rows: i32, tick: u32, clip: Clip) {
    let drift = (u64::from(tick) * u64::from(DRIFT_PER_75_TICKS) / 75) as i32;
    // The layer position that the frame's columns show, before the drift.
    let (u0, u1) = (clip.x0 + sx - drift, clip.x1 + sx - drift);
    let (cx0, cx1) = ((u0 - CLOUD_W as i32 - 32).div_euclid(56), u1.div_euclid(56));
    for cy in 0..rows.max(0) {
        for cx in cx0..=cx1 {
            let h = hash(cx, cy, 17);
            if h % 100 >= 65 {
                continue;
            }
            let x = cx * 56 + ((h >> 4) % 32) as i32 + drift - sx;
            let y = cy * 32 + ((h >> 9) % 16) as i32 - sy;
            for (r, src) in CLOUD.as_chunks::<CLOUD_W>().0.iter().enumerate() {
                f.sprite_row(x, y + r as i32, src, clip);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn band(
    f: &mut Frame,
    sx: i32,
    sy: i32,
    base_y: i32,
    heights: &[i8],
    top: &[u8; 256],
    surface_row: i32,
    fill: u8,
    clip: Clip,
) {
    if heights.is_empty() {
        return;
    }
    let (ix0, ix1) = (
        (clip.x0 + sx).div_euclid(TILE),
        (clip.x1 - 1 + sx).div_euclid(TILE),
    );
    let column_top = |ix: i32| {
        let height = i32::from(heights[ix.rem_euclid(heights.len() as i32) as usize]);
        base_y - height * TILE - surface_row - sy
    };
    for y in clip.y0..clip.y1 {
        let mut run: Option<i32> = None;
        for ix in ix0..=ix1 {
            let x = ix * TILE - sx;
            let t = column_top(ix);
            let below = y >= t + TILE;
            match (below, run) {
                (true, None) => run = Some(x),
                (false, Some(start)) => {
                    f.fill(start, y, x - start, fill, clip);
                    run = None;
                }
                _ => {}
            }
            if !below && y >= t {
                let r = (y - t) as usize;
                f.sprite_row(x, y, &top[r * 16..r * 16 + 16], clip);
            }
        }
        if let Some(start) = run {
            f.fill(start, y, (ix1 + 1) * TILE - sx - start, fill, clip);
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;
    use std::vec::Vec;

    const TOP: [u8; 256] = {
        let mut t = [255u8; 256];
        let mut i = 0;
        while i < 16 {
            t[i] = 8;
            i += 1;
        }
        t
    };

    fn stack<'a>() -> [Layer<'a>; 4] {
        static HEIGHTS: [i8; 3] = [0, 2, 1];
        [
            Layer::Stars { fx: 10, fy: 10 },
            Layer::Clouds {
                fx: 15,
                fy: 15,
                rows: 3,
            },
            Layer::Band {
                fx: 31,
                fy: 15,
                base_y: 100,
                heights: &HEIGHTS,
                top: &TOP,
                surface_row: 0,
                fill: 8,
            },
            Layer::Band {
                fx: 46,
                fy: 15,
                base_y: 120,
                heights: &[0],
                top: &TOP,
                surface_row: 0,
                fill: 1,
            },
        ]
    }

    #[test]
    fn a_layers_offset_is_the_camera_times_one_less_the_factor() {
        // f = 0 stays on the screen (the whole camera is the shift), f = 256 moves with the tiles (no shift).
        assert_eq!((offset(1000, 0), shift(1000, 0)), (1000, 0));
        assert_eq!((offset(1000, 256), shift(1000, 256)), (0, 1000));
        // The board's factors: 0.04, 0.06, 0.12 and 0.18 of 256 are 10, 15, 31 and 46.
        for (f, cam, off) in [
            (10u16, 1000, 960),
            (15, 1000, 941),
            (31, 1000, 878),
            (46, 1000, 820),
            (10, 100, 96),
            (15, 416, 391),
            (46, 192, 157),
            (31, 0, 0),
            (10, -100, -97),
        ] {
            assert_eq!(offset(cam, f), off, "factor {f} at {cam}");
            assert_eq!(shift(cam, f), cam - off);
            // The same arithmetic in floats, as Fizz 1 offsets a layer: camera * (1 - f / 256), rounded down.
            let float = (f64::from(cam) * (1.0 - f64::from(f) / 256.0)).floor() as i32;
            assert_eq!(off, float, "factor {f} at {cam}");
        }
    }

    #[test]
    fn the_cloud_is_the_ellipse_checker_of_the_board() {
        let mut on = 0;
        for y in 0..CLOUD_H {
            for x in 0..CLOUD_W {
                let ex = (x as f64 - 11.5) / 12.0;
                let ey = (y as f64 - 3.5) / 4.0;
                let want = ex * ex + ey * ey <= 1.0 && (x + y) % 2 == 1;
                assert_eq!(CLOUD[y * CLOUD_W + x] != 255, want, "({x}, {y})");
                on += usize::from(want);
            }
        }
        assert!(on > 40);
    }

    fn picture(cam_x: i32, cam_y: i32, tick: u32, w: usize, h: usize) -> Vec<u8> {
        let mut f = vec![0xEE; w * h];
        draw_layers(&mut f, w, h, cam_x, cam_y, &stack(), tick);
        f
    }

    #[test]
    fn frames_are_deterministic_and_all_ega() {
        let a = picture(37, 411, 99, 320, 200);
        assert_eq!(a, picture(37, 411, 99, 320, 200));
        assert!(a.iter().all(|&p| p <= 15));
        assert_ne!(a, picture(37, 411, 1000, 320, 200), "clouds drift");
        assert_ne!(a, picture(300, 411, 99, 320, 200), "the layers scroll");
    }

    #[test]
    fn each_layer_scrolls_at_its_factor() {
        // Stars alone: moving the camera right by 160 moves them left by 160 - floor(160 * 246 / 256) = 7 pixels.
        let layers = [Layer::Stars { fx: 10, fy: 10 }];
        let draw = |cx: i32| {
            let mut f = vec![0xEE; 200 * 100];
            draw_layers(&mut f, 200, 100, cx, 0, &layers, 0);
            f
        };
        let (a, b) = (draw(0), draw(160));
        let s = shift(160, 10) as usize;
        assert_eq!(s, 7);
        for y in 0..100 {
            for x in 0..150 {
                assert_eq!(b[y * 200 + x], a[y * 200 + x + s], "({x}, {y})");
            }
        }
        // Clouds drift 4 pixels every 75 ticks.
        let clouds = [Layer::Clouds {
            fx: 15,
            fy: 15,
            rows: 3,
        }];
        let one = |tick: u32| {
            let mut f = vec![0xEE; 200 * 100];
            draw_layers(&mut f, 200, 100, 0, 0, &clouds, tick);
            f
        };
        let (c0, c75) = (one(0), one(75));
        for y in 0..100 {
            for x in 4..200 {
                if c0[y * 200 + x - 4] == 1 && x >= 28 {
                    assert_eq!(c75[y * 200 + x], 1, "({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn stars_lie_only_over_the_clear_colour() {
        let mut f = vec![0xEE; 200 * 100];
        let layers = [
            Layer::Band {
                fx: 0,
                fy: 0,
                base_y: 0,
                heights: &[0],
                top: &TOP,
                surface_row: 0,
                fill: 3,
            },
            Layer::Stars { fx: 10, fy: 10 },
        ];
        // The band's fill covers everything but the clear sky above it, and stars draw after it: they show only above.
        draw_layers(&mut f, 200, 100, 0, 0, &layers, 0);
        assert!(
            f.iter()
                .all(|&p| p == 3 || p == 8 || p == 0 || p == 15 || p == 11 || p == 14)
        );
        for y in 16..100 {
            assert!(
                f[y * 200..y * 200 + 200].iter().all(|&p| p == 3),
                "row {y} is all fill"
            );
        }
    }

    #[test]
    fn nothing_is_written_outside_the_clip_or_the_frame() {
        let clip = Clip {
            x0: 20,
            y0: 10,
            x1: 70,
            y1: 40,
        };
        for (cx, cy, t) in [
            (0, 0, 0),
            (-500, 300, 77),
            (4000, -90, 9999),
            (i32::MIN, i32::MAX, 5),
        ] {
            let mut f = vec![0xEE; 100 * 60 + 40];
            draw_layers_clipped(&mut f, 100, 60, cx, cy, &stack(), t, clip);
            for y in 0..60 {
                for x in 0..100 {
                    let inside = (20..70).contains(&x) && (10..40).contains(&y);
                    let p = f[y * 100 + x];
                    assert!(
                        if inside { p <= 15 } else { p == 0xEE },
                        "({x}, {y}) at {cx}, {cy}: {p}"
                    );
                }
            }
            assert!(f[6000..].iter().all(|&p| p == 0xEE));
        }
        // Degenerate frames and clips do nothing and do not panic.
        let mut tiny = vec![0u8; 4];
        draw_layers(&mut tiny, 0, 4, 0, 0, &stack(), 0);
        draw_layers(&mut tiny, 4, 0, 0, 0, &stack(), 0);
        draw_layers_clipped(
            &mut tiny,
            2,
            2,
            0,
            0,
            &stack(),
            0,
            Clip {
                x0: 5,
                y0: 5,
                x1: 1,
                y1: 1,
            },
        );
    }
}
