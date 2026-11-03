// Draws the tile map in oblique projection: wooden ledges, stone and brick walls, doors and the back wall behind them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The drawing rule is specified in `docs/OBLIQUE.md`; this module is its renderer. It is integer-only and copies runs of baked
//! rows (`oblique_data`, written by `tools/tiles/emit-oblique.ts`), so the Web and DOS presenters draw the same pixels and a
//! 486 spends its time on `memcpy` rather than on arithmetic. Everything is clipped to the frame; no camera, size or map can
//! make it write outside it or panic.
//!
//! # Geometry
//!
//! A tile is 16 px. The collision geometry is the *feet plane*; a surface's strip is drawn from 16 px above it to 10 px below it
//! (26 rows), the *near plane* is 4 px left and 10 px down, and the *far plane* 6 px right and 16 px up. Strip row `r` is
//! shifted [`O`]`[r]` px right of the near edge, so an end's slant crosses the feet line (row 16) exactly on the tile edge.
//!
//! # Order
//!
//! 1. The background of every cell the view touches: the back wall (`WALLBG`, and the cells under a solid or ledge that has a
//!    room above), or open air ([`draw_open_air`]). A cell that four solid cells will cover is skipped.
//! 2. Portholes, then the contact shadows and brackets under ledges, the railings on ledges open to the sky, and the exit.
//! 3. Terrain, from the bottom row up and from left to right within a row, so a cell to the upper right hides the far parts of
//!    a lower left cell's faces without a depth buffer. A face is a top (26 rows), a front (16 px, or the 6 px fascia of a
//!    plank) and a right side. Runs of cells that look the same are drawn as one run of rows.
//! 4. The spikes, which stand on the feet plane.
//!
//! Cells outside the map repeat the nearest edge cell, so a view larger than the map is filled the same way.

use crate::oblique_data::*;
use fizz_ep2::tiles::{BLOCK, CHOC, DOOR_B, DOOR_R, EMPTY, EXIT, FILL, PLAT, SPIKE, WALLBG};

/// The side of a tile in pixels.
const T: i32 = 16;
/// Rows in a strip: the far edge to the near edge.
pub const STRIP: i32 = 26;
/// How far the near plane lies left of the feet plane.
pub const FEET_DX: i32 = 4;
/// Rows from the far edge to the feet line.
pub const BACK: i32 = 16;
/// The height of a plank's fascia.
const PLANK_FH: i32 = 6;

/// Palette indices the renderer names itself.
const BLACK: u8 = 0;
const BLUE: u8 = 1;
const BROWN: u8 = 6;
const LIGHT_BLUE: u8 = 9;
const CYAN: u8 = 3;

/// The cells that hold a porthole in the back wall, as `(column, row)` in the slice level (the row counts up from the bottom).
/// A porthole is slice dressing, not a tile: it is drawn only where the map still has back wall.
pub const PORTHOLES: [(i32, i32); 2] = [(4, 19), (19, 33)];

/// The map row where the sea meets the sky in open air: rows below its top edge are sea. It is the slice level's gallery
/// height (the exit stands on row 38 and the horizon is the top of row 39).
pub const HORIZON_ROW: i32 = 39;

/// A plank run at least this many tiles long gets a bracket under its middle as well as under its ends.
const BRACKET_MID_RUN: i32 = 6;

/// The tile map as the renderer reads it: the tile ids (row-major, bottom row first), its size, and which doors are shut.
#[derive(Clone, Copy)]
pub struct Ground<'a> {
    pub data: &'a [u8],
    pub w: i32,
    pub h: i32,
    pub red_closed: bool,
    pub blue_closed: bool,
}

impl Ground<'_> {
    /// Whether the map can be read at all.
    fn usable(&self) -> bool {
        self.w > 0 && self.h > 0 && self.data.len() >= self.w as usize * self.h as usize
    }

    /// The tile at column `c` and screen row `r` (0 at the top of the map; the map is `h` rows). Outside the map it is the
    /// nearest edge cell.
    fn cell(&self, c: i32, r: i32) -> u8 {
        let tx = c.clamp(0, self.w - 1) as usize;
        let ty = (self.h - 1 - r).clamp(0, self.h - 1) as usize;
        self.data
            .get(ty * self.w as usize + tx)
            .copied()
            .unwrap_or(EMPTY)
    }

    /// Whether Ben can not walk through the tile: fill, a block, or a door that is shut.
    fn solid(&self, t: u8) -> bool {
        t == FILL
            || t == BLOCK
            || (t == DOOR_R && self.red_closed)
            || (t == DOOR_B && self.blue_closed)
    }
}

/// Whether a tile shows the open sky.
const fn open_air(t: u8) -> bool {
    t == EMPTY || t == EXIT
}

/// Whether a tile is part of the solid masses that stone depth is measured from. Doors count whether shut or open, so the
/// stone round a door does not change its look when the door opens.
const fn mass(t: u8) -> bool {
    t == FILL || t == BLOCK || t == DOOR_R || t == DOOR_B
}

// ---------- the frame ----------

/// The frame, with every write clipped. In tests it also counts the pixels it writes.
struct Canvas<'a> {
    px: &'a mut [u8],
    w: i32,
    h: i32,
}

/// The phases of a draw, for the cost accounting of test builds.
#[cfg_attr(not(test), allow(dead_code))]
const PHASES: usize = 6;
const PHASE_BACK: usize = 0;
const PHASE_DRESS: usize = 1;
const PHASE_TOP: usize = 2;
const PHASE_FRONT: usize = 3;
const PHASE_SIDE: usize = 4;
const PHASE_STAND: usize = 5;

#[cfg(test)]
std::thread_local! {
    /// Pixels written and runs written, by phase, on this thread since the last reset, for the overdraw test.
    pub(crate) static WRITES: core::cell::Cell<[(u64, u64); PHASES]> = const { core::cell::Cell::new([(0, 0); PHASES]) };
    static PHASE: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Names the phase the writes that follow belong to (test builds only).
#[inline]
fn phase(_p: usize) {
    #[cfg(test)]
    PHASE.with(|c| c.set(_p));
}

/// Notes a run of `n` pixels written (test builds only).
#[inline]
fn count(_n: usize) {
    #[cfg(test)]
    {
        let p = PHASE.with(|c| c.get());
        WRITES.with(|w| {
            let mut all = w.get();
            all[p] = (all[p].0 + _n as u64, all[p].1 + 1);
            w.set(all);
        });
    }
}

impl<'a> Canvas<'a> {
    fn new(px: &'a mut [u8], w: usize, h: usize) -> Self {
        let rows = px.len().checked_div(w).map_or(0, |r| r.min(h));
        Canvas {
            px,
            w: w as i32,
            h: rows as i32,
        }
    }

    /// The clipped range of `len` pixels starting at `x` on row `y`: `(start index, first source offset, count)`.
    #[inline]
    fn span(&self, x: i32, y: i32, len: i32) -> Option<(usize, usize, usize)> {
        if y < 0 || y >= self.h {
            return None;
        }
        let a = x.max(0);
        let b = x.saturating_add(len).min(self.w);
        if a >= b {
            return None;
        }
        Some((
            y as usize * self.w as usize + a as usize,
            (a - x) as usize,
            (b - a) as usize,
        ))
    }

    /// The pixels of row `y` that lie in the frame out of the `len` from `x`, and the offset of the first from `x`; the
    /// caller writes them all, so the run counts as written.
    #[inline]
    fn row_mut(&mut self, x: i32, y: i32, len: i32) -> Option<(&mut [u8], i32)> {
        let (i, off, n) = self.span(x, y, len)?;
        count(n);
        Some((&mut self.px[i..i + n], off as i32))
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, c: u8) {
        if let Some((i, _, _)) = self.span(x, y, 1) {
            self.px[i] = c;
            count(1);
        }
    }

    #[inline]
    fn fill(&mut self, x: i32, y: i32, len: i32, c: u8) {
        if let Some((i, _, n)) = self.span(x, y, len) {
            self.px[i..i + n].fill(c);
            count(n);
        }
    }

    /// Copies `len` pixels of a table row to (`x`, `y`), starting at world column `u0` of the row, which repeats every 256.
    fn wrapped(&mut self, x: i32, y: i32, len: i32, row: &[u8], u0: i32) {
        let Some((mut i, off, mut n)) = self.span(x, y, len) else {
            return;
        };
        count(n);
        let mut u = (u0.wrapping_add(off as i32) & (PERIOD as i32 - 1)) as usize;
        while n > 0 {
            let c = n.min(PERIOD - u);
            self.px[i..i + c].copy_from_slice(&row[u..u + c]);
            i += c;
            n -= c;
            u = 0;
        }
    }

    /// Draws a side face with its reference point at (`x`, `y`): one copy for each row.
    fn side(&mut self, p: &Side, x: i32, y: i32) {
        let left = x + i32::from(p.ox);
        let top = y + i32::from(p.oy);
        let first = (-top).clamp(0, i32::from(p.h)) as usize;
        let last = (self.h - top).clamp(0, i32::from(p.h)) as usize;
        for (sy, &(start, len, off)) in p.spans.iter().enumerate().take(last).skip(first) {
            let (rx, rl) = (left + i32::from(start), i32::from(len));
            let Some((i, skip, n)) = self.span(rx, top + sy as i32, rl) else {
                continue;
            };
            count(n);
            let src = usize::from(off) + skip;
            self.px[i..i + n].copy_from_slice(&p.pixels[src..src + n]);
        }
    }

    /// Draws a piece with its reference point at (`x`, `y`); the pixel 255 is transparent.
    fn piece(&mut self, p: &Piece, x: i32, y: i32) {
        let (pw, ph) = (usize::from(p.w), usize::from(p.h));
        if pw == 0 || p.pixels.len() < pw * ph {
            return;
        }
        let left = x + i32::from(p.ox);
        let top = y + i32::from(p.oy);
        for (sy, src) in p.pixels.chunks_exact(pw).enumerate().take(ph) {
            let Some((i, off, n)) = self.span(left, top + sy as i32, pw as i32) else {
                continue;
            };
            let mut wrote = 0;
            for (d, &s) in self.px[i..i + n].iter_mut().zip(&src[off..off + n]) {
                if s != 255 {
                    *d = s;
                    wrote += 1;
                }
            }
            count(wrote);
        }
    }
}

/// A row of a table `width` wide... the `PERIOD` bytes of row `row`.
#[inline]
fn row_of(table: &[u8], row: usize) -> &[u8] {
    &table[row * PERIOD..(row + 1) * PERIOD]
}

// ---------- stone depth ----------

/// How many tiles, up to 4, a stone cell lies from the nearest opening, as a Chebyshev distance: 1 is next to an opening,
/// 4 is three or more cells clear on every side (deep rock). The openings are the map's own cells that are not fill, block or
/// door; cells outside the map take the answer of the nearest edge cell.
fn depth_direct(g: &Ground, tc: i32, ty: i32) -> u8 {
    let (tc, ty) = (tc.clamp(0, g.w - 1), ty.clamp(0, g.h - 1));
    for d in 1..=3i32 {
        for dy in -d..=d {
            for dx in -d..=d {
                if dx.abs().max(dy.abs()) != d {
                    continue;
                }
                let (x, y) = (tc + dx, ty + dy);
                if x >= 0 && x < g.w && y >= 0 && y < g.h {
                    let t = g.data[y as usize * g.w as usize + x as usize];
                    if !mass(t) {
                        return d as u8;
                    }
                }
            }
        }
    }
    4
}

/// Columns of the depth cache: one 64-bit mask of open rows for each map column the view reaches, plus three either side.
const DEPTH_COLS: usize = 128;

/// The openings round the view as one mask for each map column, so a cell's depth is a few shifts and not 49 reads. A view too
/// large for the masks (more than 64 map rows or [`DEPTH_COLS`] columns) falls back to [`depth_direct`], so the answer is the
/// same either way.
struct Depth {
    col0: i32,
    base: i32,
    cols: usize,
    masks: [u64; DEPTH_COLS],
    ok: bool,
}

impl Depth {
    /// The masks for the map columns `tc0..=tc1` and map rows `ty0..=ty1` that the view's stone cells lie in.
    fn new(g: &Ground, tc0: i32, tc1: i32, ty0: i32, ty1: i32) -> Self {
        let col0 = (tc0.max(0) - 3).max(0);
        let col1 = (tc1.min(g.w - 1) + 3).min(g.w - 1);
        let base = ty0.max(0) - 3;
        let top = ty1.min(g.h - 1) + 3;
        let cols = (col1 - col0 + 1).max(0) as usize;
        let mut d = Depth {
            col0,
            base,
            cols,
            masks: [0; DEPTH_COLS],
            ok: cols <= DEPTH_COLS && top - base < 64 && col1 >= col0,
        };
        if !d.ok {
            return d;
        }
        for i in 0..cols {
            let mut m = 0u64;
            for ty in base.max(0)..=top.min(g.h - 1) {
                let t = g.data[ty as usize * g.w as usize + (col0 as usize + i)];
                if !mass(t) {
                    m |= 1 << (ty - base);
                }
            }
            d.masks[i] = m;
        }
        d
    }

    fn at(&self, g: &Ground, tc: i32, ty: i32) -> u8 {
        let (tc, ty) = (tc.clamp(0, g.w - 1), ty.clamp(0, g.h - 1));
        let col1 = self.col0 + self.cols as i32 - 1;
        let columns_held =
            (tc - 3 >= self.col0 || self.col0 == 0) && (tc + 3 <= col1 || col1 == g.w - 1);
        if !self.ok || !columns_held || ty - self.base < 3 || ty - self.base > 60 {
            return depth_direct(g, tc, ty);
        }
        let p = ty - self.base;
        let mut best = 4u8;
        for dx in -3..=3i32 {
            let i = tc + dx - self.col0;
            if i < 0 || i as usize >= self.cols {
                continue;
            }
            let m = self.masks[i as usize];
            let mut r = dx.abs().max(1);
            while r < i32::from(best) {
                let win = ((1u64 << (2 * r + 1)) - 1) << (p - r);
                if m & win != 0 {
                    best = r as u8;
                    break;
                }
                r += 1;
            }
        }
        best
    }
}

// ---------- backgrounds ----------

/// What a run of cells shows behind the terrain.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Back {
    /// Four solid cells will cover it; nothing is drawn.
    Skip,
    /// The open sky.
    Open,
    /// The back wall.
    Wall,
    /// The back wall under a solid cell, which casts a shadow.
    WallShadow,
}

/// Whether the background behind cell (`c`, `r`) is the back wall. A wall tile is; open air is not; any other tile (a ledge,
/// spikes, a door, the exposed edge of a solid) takes the background of the cell above if that is open air and the back wall
/// otherwise.
fn back_is_wall(g: &Ground, c: i32, r: i32) -> bool {
    match g.cell(c, r) {
        WALLBG => true,
        t if open_air(t) => false,
        _ => !open_air(g.cell(c, r - 1)),
    }
}

fn back_of(g: &Ground, c: i32, r: i32) -> Back {
    let t = g.cell(c, r);
    if g.solid(t)
        && g.solid(g.cell(c + 1, r))
        && g.solid(g.cell(c, r - 1))
        && g.solid(g.cell(c + 1, r - 1))
    {
        return Back::Skip;
    }
    if !back_is_wall(g, c, r) {
        Back::Open
    } else if t == WALLBG && g.solid(g.cell(c, r - 1)) {
        Back::WallShadow
    } else {
        Back::Wall
    }
}

/// Fills a rectangle of the view (`x0..x1`, `y0..y1`) with open air. Sky is k with a k and B dither where it meets the
/// horizon, and below the horizon a B and k sea with a C crest. `cam_x` and `cam_y` place the view in the map so the dither is
/// fixed to the world.
#[allow(clippy::too_many_arguments)]
fn draw_open_air(
    cv: &mut Canvas,
    g: &Ground,
    cam_x: i32,
    cam_y: i32,
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
) {
    let horizon = (g.h - 1 - HORIZON_ROW) * T;
    for y in y0.max(0)..y1.min(cv.h) {
        let wy = y + cam_y;
        let d = horizon - wy;
        if d >= 10 {
            cv.fill(x0, y, x1 - x0, BLACK);
            continue;
        }
        let Some((row, off)) = cv.row_mut(x0, y, x1 - x0) else {
            continue;
        };
        let wx0 = x0 + off + cam_x;
        if d > 0 {
            // Sky above the horizon: a checker for four rows, then a sparse dot grid for six.
            if d < 4 {
                for (i, p) in row.iter_mut().enumerate() {
                    *p = if (wx0 + i as i32 + wy) & 1 == 1 {
                        BLACK
                    } else {
                        BLUE
                    };
                }
            } else if wy & 1 == 0 {
                for (i, p) in row.iter_mut().enumerate() {
                    *p = if (wx0 + i as i32) & 1 == 0 {
                        BLUE
                    } else {
                        BLACK
                    };
                }
            } else {
                row.fill(BLACK);
            }
        } else {
            // Sea: a C crest, then B, then a B and k checker.
            let s = -d;
            if s == 0 {
                row.fill(CYAN);
            } else if s < 3 {
                for (i, p) in row.iter_mut().enumerate() {
                    *p = if (((wx0 + i as i32).div_euclid(6) + 2 * s) % 3) == 0 {
                        CYAN
                    } else {
                        BLUE
                    };
                }
            } else if s < 6 {
                row.fill(BLUE);
            } else {
                for (i, p) in row.iter_mut().enumerate() {
                    *p = if (wx0 + i as i32 + wy) & 1 == 1 {
                        BLUE
                    } else {
                        BLACK
                    };
                }
            }
        }
    }
}

/// Draws the background of every cell the view touches.
fn backgrounds(cv: &mut Canvas, g: &Ground, cam_x: i32, cam_y: i32) {
    let (r0, r1) = (cam_y.div_euclid(T), (cam_y + cv.h - 1).div_euclid(T));
    let (c0, c1) = (cam_x.div_euclid(T), (cam_x + cv.w - 1).div_euclid(T));
    for r in r0..=r1 {
        let ya = (r * T).max(cam_y) - cam_y;
        let yb = ((r + 1) * T).min(cam_y + cv.h) - cam_y;
        let mut c = c0;
        while c <= c1 {
            let kind = back_of(g, c, r);
            let mut e = c + 1;
            while e <= c1 && back_of(g, e, r) == kind {
                e += 1;
            }
            let x = c * T - cam_x;
            let len = (e - c) * T;
            match kind {
                Back::Skip => {}
                Back::Open => draw_open_air(cv, g, cam_x, cam_y, x, x + len, ya, yb),
                Back::Wall | Back::WallShadow => {
                    for y in ya..yb {
                        let wy = y + cam_y;
                        let in_cell = wy.rem_euclid(T);
                        if kind == Back::WallShadow && in_cell < 13 {
                            // The shadow of the solid cell above: black, thinning to a dither on its last two rows.
                            cv.fill(x, y, len, BLACK);
                            continue;
                        }
                        let src = row_of(&BACK_WALL, wy.rem_euclid(96) as usize);
                        cv.wrapped(x, y, len, src, c * T);
                        if kind == Back::WallShadow && in_cell < 15 {
                            // The last two rows of the shadow are a dither: black where the world column and row add to odd.
                            if let Some((row, off)) = cv.row_mut(x, y, len) {
                                let wx0 = x + off + cam_x;
                                for (i, p) in row.iter_mut().enumerate() {
                                    if (wx0 + i as i32 + wy) & 1 == 1 {
                                        *p = BLACK;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            c = e;
        }
    }
}

// ---------- dressing ----------

/// The contact shadow under a ledge: three rows on the back wall under the plank cell (`c`, `r`), shortening towards the
/// near edge and dithered, from the near plane's left edge to its right (and a depth further at the end of a run).
fn plank_shadow(cv: &mut Canvas, g: &Ground, c: i32, r: i32, cam_x: i32, cam_y: i32, end: bool) {
    for k in 0..3 {
        let y = r * T + T + k - cam_y;
        let a = c * T - FEET_DX;
        let b = if end {
            c * T + T + 6 - 2 * k
        } else {
            c * T + T - FEET_DX
        };
        let mut x = a;
        while x < b {
            let cell = x.div_euclid(T);
            let seg_end = ((cell + 1) * T).min(b);
            if back_is_wall(g, cell, r + 1) {
                if k == 0 {
                    cv.fill(x - cam_x, y, seg_end - x, BLACK);
                } else if let Some((row, off)) = cv.row_mut(x - cam_x, y, seg_end - x) {
                    // Every other pixel, where the world column plus the row is even.
                    let wx0 = x + off;
                    for (i, p) in row.iter_mut().enumerate() {
                        if (wx0 + i as i32 + k) & 1 == 0 {
                            *p = BLACK;
                        }
                    }
                }
            }
            x = seg_end;
        }
    }
}

/// The brackets under a plank run: at its left end, its right end and (for a long run) its middle, where the neighbouring cell
/// is not solid. `c` is the cell being drawn; only the brackets that lie in it are drawn, so each is drawn once.
fn plank_brackets(cv: &mut Canvas, g: &Ground, c: i32, r: i32, cam_x: i32, cam_y: i32) {
    let tile = |x: i32| -> u8 {
        if x < 0 || x >= g.w {
            EMPTY
        } else {
            g.cell(x, r)
        }
    };
    let mut s = c;
    while s > 0 && tile(s - 1) == PLAT {
        s -= 1;
    }
    let mut e = c + 1;
    while e < g.w && tile(e) == PLAT {
        e += 1;
    }
    let mut spot = |bx: i32| {
        if bx.div_euclid(T) == c {
            cv.piece(&BRACKET, bx - cam_x, r * T + T - cam_y);
        }
    };
    if !g.solid(g.cell(s - 1, r)) {
        spot(s * T + 6);
    }
    if !g.solid(g.cell(e, r)) {
        spot(e * T - 10);
    }
    if e - s >= BRACKET_MID_RUN {
        spot((s + e) * T / 2);
    }
}

/// Everything drawn on the far plane before the terrain, over the cells the view touches.
fn dressing(cv: &mut Canvas, g: &Ground, cam_x: i32, cam_y: i32) {
    let (c_lo, c_hi, r_lo, r_hi) = cell_range(cv, cam_x, cam_y);
    for &(pc, pr) in &PORTHOLES {
        let r = g.h - 1 - pr;
        if pc >= 0 && pc < g.w && pr >= 0 && pr < g.h && g.cell(pc, r) == WALLBG {
            cv.piece(&PORTHOLE, pc * T + 8 - cam_x, r * T + 8 - cam_y);
        }
    }
    for r in r_lo - 1..=r_hi + 1 {
        for c in c_lo - 2..=c_hi + 2 {
            if g.cell(c, r) != PLAT {
                continue;
            }
            plank_shadow(cv, g, c, r, cam_x, cam_y, g.cell(c + 1, r) != PLAT);
            if back_is_wall(g, c, r) {
                plank_brackets(cv, g, c, r, cam_x, cam_y);
            } else {
                cv.piece(&RAIL, c * T - cam_x, r * T - cam_y);
            }
        }
    }
    // The exit stands on the far edge of the floor: its sill is the top of its lowest cell.
    for r in r_lo - 1..=r_hi + 1 {
        for c in c_lo - 2..=c_hi + 2 {
            if g.cell(c, r) == EXIT && g.cell(c, r + 1) != EXIT {
                cv.piece(&EXIT_DOORWAY, c * T - cam_x, r * T - cam_y);
            }
        }
    }
}

// ---------- terrain ----------

/// What a cell is drawn as. Cells with equal looks and no door art draw as one run.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Look {
    kind: Kind,
    /// Whether the cell has a top face.
    top: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    None,
    Stone1,
    Stone2,
    Stone3,
    Brick,
    Plank,
    DoorRed,
    DoorBlue,
}

impl Kind {
    /// Whether neighbouring cells of this kind can be drawn as one run (a door's art differs by its place in the door).
    fn merges(self) -> bool {
        !matches!(self, Kind::None | Kind::DoorRed | Kind::DoorBlue)
    }
}

fn look_of(g: &Ground, depth: &Depth, c: i32, r: i32) -> Look {
    let t = g.cell(c, r);
    let up_solid = g.solid(g.cell(c, r - 1));
    let kind = match t {
        FILL => match depth.at(g, c, g.h - 1 - r) {
            1 => Kind::Stone1,
            2 => Kind::Stone2,
            _ => Kind::Stone3,
        },
        BLOCK => Kind::Brick,
        PLAT => Kind::Plank,
        DOOR_R if g.red_closed => Kind::DoorRed,
        DOOR_B if g.blue_closed => Kind::DoorBlue,
        _ => Kind::None,
    };
    Look {
        kind,
        top: kind == Kind::Plank || (kind != Kind::None && !up_solid),
    }
}

/// The cells whose faces can reach the view: columns and screen rows, with the faces' reach (4 px left, 6 px right, 16 px
/// up and 26 px down) allowed for.
fn cell_range(cv: &Canvas, cam_x: i32, cam_y: i32) -> (i32, i32, i32, i32) {
    (
        (cam_x - 22).div_euclid(T) + 1,
        (cam_x + cv.w + 3).div_euclid(T),
        (cam_y - 26).div_euclid(T) + 1,
        (cam_y + cv.h + 15).div_euclid(T),
    )
}

/// Draws the cells `c0..c1` of screen row `r`, which all look like `look`, as one run.
#[allow(clippy::too_many_arguments)]
fn draw_run(
    cv: &mut Canvas,
    g: &Ground,
    cam_x: i32,
    cam_y: i32,
    c0: i32,
    c1: i32,
    r: i32,
    look: Look,
) {
    let n = c1 - c0;
    let left = c0 * T - FEET_DX - cam_x;
    let y0 = r * T - cam_y;
    let u0 = c0 * T;
    let last = c1 - 1;
    let plank = look.kind == Kind::Plank;

    phase(PHASE_TOP);
    if look.top {
        // The caps are the black outline at a ledge's ends. An interior joint between two cells never has one.
        let (cap_l, cap_r) = if plank {
            (
                g.cell(c0 - 1, r) != PLAT && !g.solid(g.cell(c0 - 1, r)),
                g.cell(c1, r) != PLAT,
            )
        } else {
            (
                !g.solid(g.cell(c0 - 1, r)) || g.solid(g.cell(c0 - 1, r - 1)),
                !g.solid(g.cell(c1, r)) || g.solid(g.cell(c1, r - 1)),
            )
        };
        let table: &[u8] = if plank { &PLANK_TOP } else { &STONE_TOP };
        for s in 0..STRIP {
            let y = y0 - BACK + s;
            if y < 0 || y >= cv.h {
                continue;
            }
            let x = left + i32::from(O[s as usize]);
            cv.wrapped(x, y, n * T, row_of(table, s as usize), u0);
            if cap_l {
                cv.put(x, y, BLACK);
            }
            if cap_r {
                cv.put(x + n * T - 1, y, BLACK);
            }
        }
    }

    phase(PHASE_FRONT);
    let wy = r * T;
    match look.kind {
        Kind::Stone1 | Kind::Stone2 | Kind::Stone3 | Kind::Brick => {
            for k in 0..T {
                let y = y0 + (STRIP - BACK) + k;
                let src = match look.kind {
                    Kind::Stone1 => row_of(&STONE_SHALLOW, (wy + k).rem_euclid(64) as usize),
                    Kind::Stone2 => row_of(&STONE_MID, (wy + k).rem_euclid(64) as usize),
                    Kind::Stone3 => row_of(&STONE_DEEP, (wy + k).rem_euclid(64) as usize),
                    _ => row_of(&BRICK_FRONT, (wy + k).rem_euclid(32) as usize),
                };
                cv.wrapped(left, y, n * T, src, u0);
            }
        }
        Kind::Plank => {
            for k in 0..PLANK_FH {
                let y = y0 + (STRIP - BACK) + k;
                cv.wrapped(left, y, n * T, row_of(&PLANK_FRONT, k as usize), u0);
            }
        }
        Kind::DoorRed | Kind::DoorBlue => {
            let t = g.cell(c0, r);
            let (above, below) = (g.cell(c0, r - 1) == t, g.cell(c0, r + 1) == t);
            let (top, mid, bot) = if look.kind == Kind::DoorRed {
                (&DOOR_RED_TOP, &DOOR_RED_MID, &DOOR_RED_BOT)
            } else {
                (&DOOR_BLUE_TOP, &DOOR_BLUE_MID, &DOOR_BLUE_BOT)
            };
            let piece = match (above, below) {
                (false, _) => top,
                (true, true) => mid,
                (true, false) => bot,
            };
            cv.piece(piece, left, y0 + (STRIP - BACK));
        }
        Kind::None => {}
    }

    phase(PHASE_SIDE);
    // The right side face, where the cell to the right does not stop it.
    let right = g.cell(c1, r);
    let side = match look.kind {
        Kind::Plank => (right != PLAT).then_some(&SIDE_PLANK),
        Kind::Brick => (!g.solid(right)).then_some(&SIDE_BRICK),
        Kind::DoorRed => (!g.solid(right)).then_some(&SIDE_DOOR_RED),
        Kind::DoorBlue => (!g.solid(right)).then_some(&SIDE_DOOR_BLUE),
        Kind::None => None,
        _ => (!g.solid(right)).then_some(&SIDE_STONE),
    };
    if let Some(p) = side {
        cv.side(p, last * T + T - FEET_DX - cam_x, y0 - BACK);
    }
}

fn terrain(cv: &mut Canvas, g: &Ground, cam_x: i32, cam_y: i32) {
    let (c_lo, c_hi, r_lo, r_hi) = cell_range(cv, cam_x, cam_y);
    // Stone depth is read for the map rows these screen rows hold.
    let depth = Depth::new(g, c_lo, c_hi, g.h - 1 - r_hi, g.h - 1 - r_lo);
    for r in (r_lo..=r_hi).rev() {
        let mut c = c_lo;
        while c <= c_hi {
            let look = look_of(g, &depth, c, r);
            if look.kind == Kind::None {
                c += 1;
                continue;
            }
            let mut e = c + 1;
            if look.kind.merges() {
                while e <= c_hi && look_of(g, &depth, e, r) == look {
                    e += 1;
                }
            }
            draw_run(cv, g, cam_x, cam_y, c, e, r, look);
            c = e;
        }
    }
}

/// The spikes, and any other thing that stands on the floor strip without being a surface.
fn standing(cv: &mut Canvas, g: &Ground, cam_x: i32, cam_y: i32) {
    let (c_lo, c_hi, r_lo, r_hi) = cell_range(cv, cam_x, cam_y);
    for r in (r_lo..=r_hi).rev() {
        for c in c_lo..=c_hi {
            match g.cell(c, r) {
                SPIKE => cv.piece(&SPIKES, c * T - cam_x, r * T - cam_y),
                CHOC => {
                    // Lethal chocolate: the slice has none, so it is a flat brown square on the feet plane.
                    for y in 0..T {
                        cv.fill(c * T - cam_x, r * T + y - cam_y, T, BROWN);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Draws the map into the `w` by `h` frame, with the view's top left at (`cam_x`, `cam_y`) in map pixels (y down, as
/// `fizz_view::follow` gives them). Every pixel of the frame is written.
pub fn draw_ground(frame: &mut [u8], w: usize, h: usize, g: &Ground, cam_x: i32, cam_y: i32) {
    let mut cv = Canvas::new(frame, w, h);
    if cv.w == 0 || cv.h == 0 {
        return;
    }
    // A camera this far from the map shows only its repeated edge; clamping keeps the arithmetic below in range.
    let (cam_x, cam_y) = (
        cam_x.clamp(-(1 << 24), 1 << 24),
        cam_y.clamp(-(1 << 24), 1 << 24),
    );
    if !g.usable() {
        for y in 0..cv.h {
            cv.fill(0, y, cv.w, LIGHT_BLUE);
        }
        return;
    }
    phase(PHASE_BACK);
    backgrounds(&mut cv, g, cam_x, cam_y);
    phase(PHASE_DRESS);
    dressing(&mut cv, g, cam_x, cam_y);
    terrain(&mut cv, g, cam_x, cam_y);
    phase(PHASE_STAND);
    standing(&mut cv, g, cam_x, cam_y);
}

#[cfg(test)]
mod tests;
