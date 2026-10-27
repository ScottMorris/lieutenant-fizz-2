// Clipped software drawing into a palette-indexed frame buffer, shared by both presenters.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::TILE_PX;
use crate::sprites_data::Sprite;

/// The palette index that marks a transparent sprite pixel. Index 0 is black and is drawn.
pub const TRANSPARENT: u8 = 255;

/// The part of `frame` that is a whole `w` by `h` image: `(w, h)` reduced if the slice is too short, and zero if a
/// dimension is zero. Drawing never writes outside it.
fn extent(frame: &[u8], w: usize, h: usize) -> (i64, i64) {
    if w == 0 || h == 0 {
        return (0, 0);
    }
    let rows = (frame.len() / w).min(h);
    (w as i64, rows as i64)
}

/// Draws `sprite` with its anchor at screen position (`x`, `y`) in a `w` by `h` frame, clipped to the frame.
///
/// The anchor is the sprite's own `ax`, `ay`: feet and centre unless the set says otherwise. With `flip_x` the sprite is
/// mirrored about the vertical line through the anchor (the left edge of the anchor's pixel column, so an even-width sprite with a centred anchor covers the same box), so facing the other way never moves the feet. The pixel value 255 is transparent;
/// 0 is black and is drawn. Any position, including far outside the frame, is safe.
pub fn blit_sprite(
    frame: &mut [u8],
    w: usize,
    h: usize,
    sprite: &Sprite,
    x: i32,
    y: i32,
    flip_x: bool,
) {
    let (fw, fh) = extent(frame, w, h);
    let (sw, sh) = (usize::from(sprite.w), usize::from(sprite.h));
    if sw == 0 || sh == 0 || sprite.pixels.len() < sw * sh {
        return;
    }
    let (ax, ay) = (i64::from(sprite.ax), i64::from(sprite.ay));
    // The sprite's rows `sy0..sy1` land on frame rows `top + sy`.
    let top = i64::from(y) - ay;
    let sy0 = (-top).clamp(0, sh as i64) as usize;
    let sy1 = (fh - top).clamp(0, sh as i64) as usize;
    // Facing right, source column `sx` lands on `left + sx`; flipped, on `right - sx`. The columns `sx0..sx1` are the ones that
    // land inside the frame, so the rows below need no per-pixel clipping.
    let (sx0, sx1, dx0) = if flip_x {
        let right = i64::from(x) + ax - 1;
        let sx0 = (right - (fw - 1)).clamp(0, sw as i64);
        let sx1 = (right + 1).clamp(0, sw as i64);
        (sx0 as usize, sx1 as usize, right - (sx1 - 1))
    } else {
        let left = i64::from(x) - ax;
        let sx0 = (-left).clamp(0, sw as i64);
        let sx1 = (fw - left).clamp(0, sw as i64);
        (sx0 as usize, sx1 as usize, left + sx0)
    };
    if sx0 >= sx1 || sy0 >= sy1 {
        return;
    }
    let n = sx1 - sx0;
    let dx0 = dx0 as usize;
    for sy in sy0..sy1 {
        let row = (top + sy as i64) as usize * w;
        let src = &sprite.pixels[sy * sw + sx0..sy * sw + sx1];
        let dst = &mut frame[row + dx0..row + dx0 + n];
        if flip_x {
            for (d, &p) in dst.iter_mut().zip(src.iter().rev()) {
                if p != TRANSPARENT {
                    *d = p;
                }
            }
        } else {
            for (d, &p) in dst.iter_mut().zip(src) {
                if p != TRANSPARENT {
                    *d = p;
                }
            }
        }
    }
}

/// Fills the rectangle with its top left at (`x`, `y`) and size `rw` by `rh` with `colour`, clipped to the frame. A
/// non-positive size draws nothing.
#[allow(clippy::too_many_arguments)]
pub fn fill_rect(
    frame: &mut [u8],
    w: usize,
    h: usize,
    x: i32,
    y: i32,
    rw: i32,
    rh: i32,
    colour: u8,
) {
    let (fw, fh) = extent(frame, w, h);
    let x0 = i64::from(x).max(0);
    let y0 = i64::from(y).max(0);
    let x1 = (i64::from(x) + i64::from(rw.max(0))).min(fw);
    let y1 = (i64::from(y) + i64::from(rh.max(0))).min(fh);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for row in y0..y1 {
        let start = row as usize * w;
        frame[start + x0 as usize..start + x1 as usize].fill(colour);
    }
}

/// Draws the part of a tile map that the view shows, as flat colour tiles.
///
/// The map is `map_w` by `map_h` tiles with world coordinates that are y-up: tile (0, 0) is bottom left. `colour(tx, ty)`
/// gives the palette index for a tile, or `None` to leave it undrawn (sky, or a tile the caller draws another way); it is
/// only called for tiles inside the map that touch the view. `cam_x` and `cam_y` are the view's top left in map pixels,
/// y-down, as `follow` gives them, so a tile row `ty` begins at map row `(map_h - 1 - ty) * 16`, the same row `screen_y`
/// gives for its upper edge. Only visible tiles are visited, whatever the camera, and everything is clipped to the frame.
#[allow(clippy::too_many_arguments)]
pub fn draw_tilemap(
    frame: &mut [u8],
    w: usize,
    h: usize,
    cam_x: i32,
    cam_y: i32,
    map_w: i32,
    map_h: i32,
    mut colour: impl FnMut(i32, i32) -> Option<u8>,
) {
    draw_tiles(frame, w, h, cam_x, cam_y, map_w, map_h, None, &mut colour);
}

/// Draws the whole view as tiles on a flat sky: [`draw_tilemap`] with every pixel it leaves undrawn, and everything outside
/// the map, filled with `sky`, and each pixel written exactly once. It is the same picture as filling the frame with `sky`
/// and then calling [`draw_tilemap`], without writing the tiles' area twice. Horizontal neighbours of one colour are filled
/// as one run, so the frame is written in long rows, which is also what a fast fill wants.
#[allow(clippy::too_many_arguments)]
pub fn draw_tilemap_on_sky(
    frame: &mut [u8],
    w: usize,
    h: usize,
    cam_x: i32,
    cam_y: i32,
    map_w: i32,
    map_h: i32,
    sky: u8,
    mut colour: impl FnMut(i32, i32) -> Option<u8>,
) {
    draw_tiles(
        frame,
        w,
        h,
        cam_x,
        cam_y,
        map_w,
        map_h,
        Some(sky),
        &mut colour,
    );
}

/// Fills the columns `col0..col1` of the tile row whose pixel rows are `y0..y1` with `colour`, clipped to the `fw` wide frame.
#[allow(clippy::too_many_arguments)]
fn fill_run(
    frame: &mut [u8],
    w: usize,
    cx: i64,
    fw: i64,
    col0: i64,
    col1: i64,
    y0: usize,
    y1: usize,
    colour: u8,
) {
    let x0 = (col0 * i64::from(TILE_PX) - cx).max(0) as usize;
    let x1 = (col1 * i64::from(TILE_PX) - cx).min(fw) as usize;
    if x0 >= x1 {
        return;
    }
    for y in y0..y1 {
        frame[y * w + x0..y * w + x1].fill(colour);
    }
}

/// The tile rows that touch the view, each drawn as runs of one colour. With `sky` the undrawn tiles and the area outside the
/// map take that colour, and the whole view is visited; without it only tiles inside the map are visited.
#[allow(clippy::too_many_arguments)]
fn draw_tiles(
    frame: &mut [u8],
    w: usize,
    h: usize,
    cam_x: i32,
    cam_y: i32,
    map_w: i32,
    map_h: i32,
    sky: Option<u8>,
    colour: &mut dyn FnMut(i32, i32) -> Option<u8>,
) {
    let (fw, fh) = extent(frame, w, h);
    if fw == 0 || fh == 0 {
        return;
    }
    let t = i64::from(TILE_PX);
    let (cx, cy) = (i64::from(cam_x), i64::from(cam_y));
    let (mw, mh) = (i64::from(map_w.max(0)), i64::from(map_h.max(0)));
    let (mut col0, mut col1) = (cx.div_euclid(t), (cx + fw - 1).div_euclid(t));
    let (mut row0, mut row1) = (cy.div_euclid(t), (cy + fh - 1).div_euclid(t));
    if sky.is_none() {
        col0 = col0.max(0);
        col1 = col1.min(mw - 1);
        row0 = row0.max(0);
        row1 = row1.min(mh - 1);
    }
    for row in row0..=row1 {
        // The band of pixel rows this tile row covers inside the frame (rows outside it are skipped by an empty range).
        let y0 = (row * t - cy).clamp(0, fh) as usize;
        let y1 = ((row + 1) * t - cy).clamp(0, fh) as usize;
        // With a sky the runs cover the whole row, so they are filled into the band's first row only and the other rows are
        // copies of it: one long copy a row in place of every run again.
        let fill_to = if sky.is_some() { y1.min(y0 + 1) } else { y1 };
        let in_map_row = (0..mh).contains(&row);
        let ty = (mh - 1 - row) as i32;
        let mut run: Option<(i64, u8)> = None;
        for col in col0..=col1 {
            let drawn = if in_map_row && (0..mw).contains(&col) {
                colour(col as i32, ty)
            } else {
                None
            };
            let c = drawn.or(sky);
            match (run, c) {
                (Some((_, rc)), Some(nc)) if rc == nc => {}
                _ => {
                    if let Some((start, rc)) = run {
                        fill_run(frame, w, cx, fw, start, col, y0, fill_to, rc);
                    }
                    run = c.map(|c| (col, c));
                }
            }
        }
        if let Some((start, rc)) = run {
            fill_run(frame, w, cx, fw, start, col1 + 1, y0, fill_to, rc);
        }
        for y in fill_to..y1 {
            let (done, rest) = frame.split_at_mut(y * w);
            rest[..fw as usize].copy_from_slice(&done[y0 * w..y0 * w + fw as usize]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprites_data::{SPRITES, sprite};
    use crate::{follow, screen_y};

    const W: usize = 32;
    const H: usize = 20;

    fn blank() -> [u8; W * H] {
        [200; W * H]
    }

    fn tiny(pixels: &'static [u8], w: u8, h: u8, ax: i8, ay: i8) -> Sprite {
        Sprite {
            name: "tiny",
            w,
            h,
            ax,
            ay,
            pixels,
        }
    }

    #[test]
    fn a_sprite_is_placed_by_its_anchor() {
        // 3 wide, 2 high, feet and centre: anchor (1, 2). Position (10, 5): left 9, top 3.
        let s = tiny(&[1, 2, 3, 4, 5, 6], 3, 2, 1, 2);
        let mut f = blank();
        blit_sprite(&mut f, W, H, &s, 10, 5, false);
        assert_eq!(&f[3 * W + 9..3 * W + 12], &[1, 2, 3]);
        assert_eq!(&f[4 * W + 9..4 * W + 12], &[4, 5, 6]);
        assert_eq!(f.iter().filter(|&&p| p != 200).count(), 6);
    }

    #[test]
    fn flipping_mirrors_about_the_anchor_column() {
        let s = tiny(&[1, 2, 3, 4, 5, 6], 3, 2, 1, 2);
        let mut f = blank();
        blit_sprite(&mut f, W, H, &s, 10, 5, true);
        // The anchor is the line at the left edge of column 10, so an odd width lands one column to the left.
        assert_eq!(&f[3 * W + 8..3 * W + 11], &[3, 2, 1]);
        assert_eq!(&f[4 * W + 8..4 * W + 11], &[6, 5, 4]);
        // An even width with a centred anchor covers the same box, mirrored.
        let s = tiny(&[1, 2, 3, 4], 4, 1, 2, 1);
        let mut f = blank();
        blit_sprite(&mut f, W, H, &s, 10, 5, true);
        assert_eq!(&f[4 * W + 8..4 * W + 12], &[4, 3, 2, 1]);
        // An off-centre anchor (column 1 of 4) mirrors about the anchor, not the box.
        let s = tiny(&[1, 2, 3, 4], 4, 1, 1, 1);
        let mut f = blank();
        blit_sprite(&mut f, W, H, &s, 10, 5, false);
        assert_eq!(&f[4 * W + 9..4 * W + 13], &[1, 2, 3, 4]);
        let mut f = blank();
        blit_sprite(&mut f, W, H, &s, 10, 5, true);
        assert_eq!(&f[4 * W + 7..4 * W + 11], &[4, 3, 2, 1]);
    }

    /// The definition of a blit, one pixel at a time: the anchored, optionally mirrored position of each opaque pixel, kept when it
    /// is inside the frame.
    fn reference_blit(frame: &mut [u8], w: usize, h: usize, s: &Sprite, x: i32, y: i32, flip: bool) {
        for sy in 0..usize::from(s.h) {
            for sx in 0..usize::from(s.w) {
                let p = s.pixels[sy * usize::from(s.w) + sx];
                if p == TRANSPARENT {
                    continue;
                }
                let dy = i64::from(y) - i64::from(s.ay) + sy as i64;
                let dx = if flip {
                    i64::from(x) + i64::from(s.ax) - sx as i64 - 1
                } else {
                    i64::from(x) - i64::from(s.ax) + sx as i64
                };
                if (0..h as i64).contains(&dy) && (0..w as i64).contains(&dx) {
                    frame[dy as usize * w + dx as usize] = p;
                }
            }
        }
    }

    #[test]
    fn a_blit_equals_the_pixel_by_pixel_definition_at_every_position() {
        // Every sprite, both facings, from well outside the frame on every side across it.
        for s in SPRITES {
            for flip in [false, true] {
                for y in (-40..70).step_by(3) {
                    for x in (-40..70).step_by(3) {
                        let mut want = [200u8; 40 * 30];
                        let mut got = [200u8; 40 * 30];
                        reference_blit(&mut want, 40, 30, s, x, y, flip);
                        blit_sprite(&mut got, 40, 30, s, x, y, flip);
                        assert_eq!(got, want, "{} at ({x}, {y}) flip {flip}", s.name);
                    }
                }
            }
        }
    }

    #[test]
    fn transparent_is_255_and_black_is_drawn() {
        let s = tiny(&[0, 255, 7], 3, 1, 0, 0);
        let mut f = blank();
        blit_sprite(&mut f, W, H, &s, 0, 0, false);
        assert_eq!(&f[..3], &[0, 200, 7]);
    }

    #[test]
    fn a_sprite_is_clipped_at_every_edge() {
        let s = tiny(&[1, 2, 3, 4], 2, 2, 0, 0);
        let mut f = blank();
        blit_sprite(&mut f, W, H, &s, -1, -1, false);
        assert_eq!(f[0], 4);
        assert_eq!(f.iter().filter(|&&p| p != 200).count(), 1);
        let mut f = blank();
        blit_sprite(&mut f, W, H, &s, W as i32 - 1, H as i32 - 1, false);
        assert_eq!(f[W * H - 1], 1);
        assert_eq!(f.iter().filter(|&&p| p != 200).count(), 1);
    }

    #[test]
    fn extreme_positions_do_not_panic_or_write_past_the_frame() {
        let s = sprite("ben_stand").unwrap();
        for x in [i32::MIN, i32::MIN + 1, -100, 0, 31, i32::MAX - 1, i32::MAX] {
            for y in [i32::MIN, -100, 0, 19, i32::MAX] {
                for flip in [false, true] {
                    let mut g = [200u8; W * H + 8];
                    blit_sprite(&mut g, W, H, s, x, y, flip);
                    assert!(g[W * H..].iter().all(|&p| p == 200));
                }
            }
        }
    }

    #[test]
    fn every_sprite_stays_inside_the_frame_at_any_position() {
        let mut guard = [200u8; 40 * 30 + 16];
        for s in SPRITES {
            for (x, y) in [(-30, -30), (0, 0), (20, 15), (39, 29), (70, 70)] {
                for flip in [false, true] {
                    blit_sprite(&mut guard, 40, 30, s, x, y, flip);
                }
            }
        }
        assert!(guard[40 * 30..].iter().all(|&p| p == 200));
    }

    #[test]
    fn a_flipped_sprite_is_the_mirror_image_about_the_anchor_line() {
        let s = sprite("ben_stand").unwrap();
        let mut a = [200u8; 64 * 64];
        let mut b = [200u8; 64 * 64];
        blit_sprite(&mut a, 64, 64, s, 32, 40, false);
        blit_sprite(&mut b, 64, 64, s, 32, 40, true);
        // The feet row is the anchor row: nothing is drawn at or below it.
        assert!(a[40 * 64..].iter().all(|&p| p == 200));
        // The mirror really is a mirror.
        for y in 0..64 {
            for x in 0..64 {
                assert_eq!(a[y * 64 + x], b[y * 64 + (63 - x)], "{x} {y}");
            }
        }
    }

    #[test]
    fn fill_rect_clips_and_ignores_empty_sizes() {
        let mut f = blank();
        fill_rect(&mut f, W, H, -2, -2, 4, 4, 9);
        assert_eq!(f.iter().filter(|&&p| p == 9).count(), 4);
        let mut f = blank();
        fill_rect(&mut f, W, H, 5, 5, 0, 3, 9);
        fill_rect(&mut f, W, H, 5, 5, -3, 3, 9);
        fill_rect(&mut f, W, H, 5, 5, 3, -3, 9);
        fill_rect(&mut f, W, H, i32::MIN, i32::MIN, i32::MAX, i32::MAX, 9);
        assert!(f.iter().all(|&p| p == 200));
        fill_rect(&mut f, W, H, -5, -5, i32::MAX, i32::MAX, 9);
        assert!(f.iter().all(|&p| p == 9));
        let mut f = blank();
        fill_rect(&mut f, W, H, 30, 18, i32::MAX, i32::MAX, 9);
        assert_eq!(f.iter().filter(|&&p| p == 9).count(), 4);
    }

    #[test]
    fn a_short_or_empty_frame_is_never_overrun() {
        let s = sprite("bubble").unwrap();
        // Ten bytes hold two whole rows of four.
        let mut f = [200u8; 10];
        fill_rect(&mut f, 4, 100, 0, 0, 100, 100, 1);
        assert_eq!(f, [1, 1, 1, 1, 1, 1, 1, 1, 200, 200]);
        blit_sprite(&mut f, 4, 100, s, 4, 8, false);
        draw_tilemap(&mut f, 4, 100, 0, 0, 5, 5, |_, _| Some(2));
        assert_eq!(f[8..], [200, 200]);
        let mut empty: [u8; 0] = [];
        blit_sprite(&mut empty, 4, 4, s, 0, 0, false);
        fill_rect(&mut empty, 0, 0, 0, 0, 5, 5, 1);
        draw_tilemap(&mut empty, 0, 0, 0, 0, 5, 5, |_, _| Some(2));
    }

    // A 3 by 2 map (y-up): bottom row 1 2 3, top row 4 . 6.
    fn colour(tx: i32, ty: i32) -> Option<u8> {
        match (tx, ty) {
            (0, 0) => Some(1),
            (1, 0) => Some(2),
            (2, 0) => Some(3),
            (0, 1) => Some(4),
            (1, 1) => None,
            (2, 1) => Some(6),
            _ => panic!("tile ({tx}, {ty}) is outside the map"),
        }
    }

    #[test]
    fn tiles_draw_y_up_with_the_top_row_first_on_screen() {
        const FW: usize = 48;
        const FH: usize = 32;
        let mut f = [200u8; FW * FH];
        draw_tilemap(&mut f, FW, FH, 0, 0, 3, 2, colour);
        assert_eq!(f[0], 4);
        assert_eq!(f[16], 200);
        assert_eq!(f[32], 6);
        assert_eq!(f[16 * FW], 1);
        assert_eq!(f[16 * FW + 16], 2);
        assert_eq!(f[16 * FW + 32 + 15], 3);
        assert_eq!(f[31 * FW + 47], 3);
        assert_eq!(f[15 * FW + 16], 200);
        assert_eq!(f[15 * FW + 15], 4);
    }

    #[test]
    fn the_camera_scrolls_the_tiles_by_whole_pixels() {
        const FW: usize = 20;
        const FH: usize = 20;
        let mut f = [200u8; FW * FH];
        draw_tilemap(&mut f, FW, FH, 10, 12, 3, 2, colour);
        // The top map row spans screen rows -12..4 and tile (0, 1) spans screen columns -10..6; tile (1, 1) is undrawn.
        assert_eq!(f[0], 4);
        assert_eq!(f[5], 4);
        assert_eq!(f[6], 200);
        // The bottom map row begins at screen row 16 - 12 = 4.
        assert_eq!(f[3 * FW], 4);
        assert_eq!(f[4 * FW], 1);
        assert_eq!(f[4 * FW + 5], 1);
        assert_eq!(f[4 * FW + 6], 2);
    }

    #[test]
    fn the_callback_only_sees_tiles_inside_the_map_that_touch_the_view() {
        let mut f = [0u8; 20 * 20];
        let cams = [i32::MIN, -1000, -17, -16, -1, 0, 1, 47, 48, 1000, i32::MAX];
        for cam_x in cams {
            for cam_y in cams {
                let mut calls = 0;
                draw_tilemap(&mut f, 20, 20, cam_x, cam_y, 3, 2, |tx, ty| {
                    assert!((0..3).contains(&tx) && (0..2).contains(&ty));
                    calls += 1;
                    Some(1)
                });
                assert!(calls <= 6);
            }
        }
        let mut calls = 0;
        draw_tilemap(&mut f, 20, 20, 0, 0, 0, 5, |_, _| {
            calls += 1;
            None
        });
        draw_tilemap(&mut f, 20, 20, 0, 0, 5, -1, |_, _| {
            calls += 1;
            None
        });
        assert_eq!(calls, 0);
    }

    // A 5 by 3 map with runs, gaps and a lone tile (y-up): top row `1 1 . 2 2`, middle `. 3 3 3 .`, bottom `4 4 4 4 4`.
    fn runs(tx: i32, ty: i32) -> Option<u8> {
        const MAP: [[u8; 5]; 3] = [[4, 4, 4, 4, 4], [0, 3, 3, 3, 0], [1, 1, 0, 2, 2]];
        match MAP[ty as usize][tx as usize] {
            0 => None,
            c => Some(c),
        }
    }

    #[test]
    fn tiles_on_sky_equal_a_sky_fill_then_the_tiles() {
        // Frames of several shapes, with the camera on, beside and well outside the map, so the sky shows on every side.
        let cams = [
            i32::MIN,
            -300,
            -40,
            -17,
            -16,
            -1,
            0,
            1,
            5,
            16,
            33,
            47,
            64,
            100,
            1000,
            i32::MAX,
        ];
        for (fw, fh) in [(48, 32), (20, 20), (1, 1), (7, 5), (80, 64), (33, 17)] {
            for cam_x in cams {
                for cam_y in cams {
                    let mut want_buf = [200u8; 80 * 64];
                    let want = &mut want_buf[..fw * fh];
                    want.fill(9);
                    draw_tilemap(want, fw, fh, cam_x, cam_y, 5, 3, runs);
                    let mut got_buf = [0xEEu8; 80 * 64];
                    let got = &mut got_buf[..fw * fh];
                    draw_tilemap_on_sky(got, fw, fh, cam_x, cam_y, 5, 3, 9, runs);
                    assert_eq!(&*got, &*want, "{fw}x{fh} at ({cam_x}, {cam_y})");
                }
            }
        }
    }

    #[test]
    fn tiles_on_sky_fill_the_whole_view_without_a_map_and_never_overrun() {
        let mut f = [0xEEu8; 12 * 4 + 6];
        draw_tilemap_on_sky(&mut f, 12, 4, 3, 3, 0, 0, 7, |_, _| {
            panic!("no tile exists")
        });
        assert!(f[..48].iter().all(|&p| p == 7));
        assert!(f[48..].iter().all(|&p| p == 0xEE));
        // A short buffer holds two whole rows of four.
        let mut f = [0xEEu8; 10];
        draw_tilemap_on_sky(&mut f, 4, 100, 0, 0, 5, 3, 7, runs);
        assert_eq!(f[8..], [0xEE, 0xEE]);
        let mut empty: [u8; 0] = [];
        draw_tilemap_on_sky(&mut empty, 0, 0, 0, 0, 5, 3, 7, runs);
    }

    #[test]
    fn a_huge_map_visits_only_the_visible_tiles() {
        let mut f = [0u8; 320 * 200];
        let mut calls = 0u32;
        draw_tilemap(&mut f, 320, 200, 1000, 500, i32::MAX, i32::MAX, |_, _| {
            calls += 1;
            Some(1)
        });
        assert!(calls <= 21 * 14, "{calls}");
    }

    #[test]
    fn a_map_smaller_than_the_view_is_drawn_centred() {
        const FW: usize = 64;
        const FH: usize = 64;
        let mut f = [200u8; FW * FH];
        let cx = follow(0, 48, FW as i32);
        let cy = follow(0, 32, FH as i32);
        assert_eq!((cx, cy), (-8, -16));
        draw_tilemap(&mut f, FW, FH, cx, cy, 3, 2, colour);
        assert_eq!(f[15 * FW + 8], 200);
        assert_eq!(f[16 * FW + 7], 200);
        assert_eq!(f[16 * FW + 8], 4);
        assert_eq!(f[47 * FW + 55], 3);
        assert_eq!(f[48 * FW + 55], 200);
    }

    #[test]
    fn a_sprite_on_a_tile_agrees_with_the_tile_edge() {
        // A 3 by 2 map: the top row (ty = 1) begins at map row 0, so feet at height 2.0 are at row 0 and feet standing on
        // tile row 0 (height 1.0) are at row 16, the row where the bottom tiles begin.
        assert_eq!(screen_y(2.0, 2), 0);
        assert_eq!(screen_y(1.0, 2), 16);
    }
}
