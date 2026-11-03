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
//! 0. The cell window ([`Window`]): the tiles of the cells the view reaches, and for each its look and its background, read
//!    once for the whole draw so that no later step looks a neighbour up again.
//! 1. The background of every cell the view touches: the back wall (`WALLBG`, and the cells under a solid or ledge that has a
//!    room above), or open air ([`draw_open_air`], the parallax stack of the slice's theme). A cell that four solid cells will cover is skipped.
//! 2. Portholes, then the contact shadows and brackets under ledges, the railings on ledges open to the sky, and the exit.
//! 3. Terrain, from the bottom row up and from left to right within a row, so a cell to the upper right hides the far parts of
//!    a lower left cell's faces without a depth buffer. A face is a top (26 rows), a front (16 px, or the 6 px fascia of a
//!    plank) and a right side. Runs of cells that look the same are drawn as one run of rows.
//! 4. The spikes, which stand on the feet plane.
//!
//! Cells outside the map repeat the nearest edge cell, so a view larger than the map is filled the same way.

use crate::oblique_data::*;
use fizz_ep2::tiles::{BLOCK, CHOC, DOOR_B, DOOR_R, EMPTY, EXIT, FILL, PLAT, SPIKE, WALLBG};
use fizz_view::parallax::{Clip, Layer, Sky};

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
const DARK_GREY: u8 = 8;
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

    /// The class bits that make a tile solid: fill, a block, and a door that is shut.
    fn solid_mask(&self) -> u8 {
        C_FILL
            | C_BLOCK
            | if self.red_closed { C_DOOR_R } else { 0 }
            | if self.blue_closed { C_DOOR_B } else { 0 }
    }
}

// ---------- tile classes ----------

/// Fill (stone).
const C_FILL: u8 = 1;
/// A brick block.
const C_BLOCK: u8 = 2;
/// The red door, shut or open.
const C_DOOR_R: u8 = 4;
/// The blue door, shut or open.
const C_DOOR_B: u8 = 8;
/// Open sky: empty or the exit.
const C_OPEN: u8 = 16;
/// A one-way plank.
const C_PLAT: u8 = 32;
/// Spikes or lethal chocolate, which stand on the feet plane.
const C_STAND: u8 = 64;
/// The exit.
const C_EXIT: u8 = 128;
/// The solid masses that stone depth is measured from. Doors count whether shut or open, so the stone round a door does not
/// change its look when the door opens.
const C_MASS: u8 = C_FILL | C_BLOCK | C_DOOR_R | C_DOOR_B;

/// The classes of every tile id, so a test of a tile is one load and a mask.
static CLASS: [u8; 256] = {
    let mut t = [0u8; 256];
    t[FILL as usize] = C_FILL;
    t[BLOCK as usize] = C_BLOCK;
    t[DOOR_R as usize] = C_DOOR_R;
    t[DOOR_B as usize] = C_DOOR_B;
    t[EMPTY as usize] = C_OPEN;
    t[EXIT as usize] = C_OPEN | C_EXIT;
    t[PLAT as usize] = C_PLAT;
    t[SPIKE as usize] = C_STAND;
    t[CHOC as usize] = C_STAND;
    t
};

/// Whether a tile shows the open sky.
#[inline]
fn open_air(t: u8) -> bool {
    CLASS[usize::from(t)] & C_OPEN != 0
}

/// Whether a tile is part of the solid masses that stone depth is measured from.
#[inline]
fn mass(t: u8) -> bool {
    CLASS[usize::from(t)] & C_MASS != 0
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

/// Whether a fixed move of 32 bytes or more is a call. On 32-bit x86 (the DOS build, which has no vector registers and is built
/// for size) the compiler inlines a fixed move of up to 16 bytes as dword moves and makes anything larger a call to `memcpy` or
/// `memset`, so two overlapping fixed moves would cost two calls and their set-up; there one call moves a run of 32 or more. Where
/// the moves are vector registers (x86-64, WebAssembly) two of them are cheaper than any call. The pixels are the same either way.
const ONE_CALL_FROM_32: bool = cfg!(target_arch = "x86");

/// Moves `N` bytes from the start (or the end) of `src` to the start (or the end) of `dst`, as one fixed-size assignment.
#[inline(always)]
fn mov<const N: usize>(dst: &mut [u8], src: &[u8], end: bool) {
    if end {
        if let (Some(d), Some(s)) = (dst.last_chunk_mut::<N>(), src.last_chunk::<N>()) {
            *d = *s;
        }
    } else if let (Some(d), Some(s)) = (dst.first_chunk_mut::<N>(), src.first_chunk::<N>()) {
        *d = *s;
    }
}

/// Copies `src` over `dst`, which are the same length. A run of up to 128 pixels is two moves of fixed size that overlap when
/// the length is not a power of two, because a call to `memcpy` costs more than the bytes it moves. The short runs, which are
/// the common ones, are the first to be tried.
#[inline(always)]
fn copy_to(dst: &mut [u8], src: &[u8]) {
    let n = dst.len();
    let src = &src[..n];
    if ONE_CALL_FROM_32 && n >= 32 {
        dst.copy_from_slice(src);
    } else if n < 8 {
        if n >= 4 {
            mov::<4>(dst, src, false);
            mov::<4>(dst, src, true);
        } else if n >= 2 {
            mov::<2>(dst, src, false);
            mov::<2>(dst, src, true);
        } else if n == 1 {
            dst[0] = src[0];
        }
    } else if n < 16 {
        mov::<8>(dst, src, false);
        mov::<8>(dst, src, true);
    } else if n < 32 {
        mov::<16>(dst, src, false);
        mov::<16>(dst, src, true);
    } else if n < 64 {
        mov::<32>(dst, src, false);
        mov::<32>(dst, src, true);
    } else if n <= 128 {
        mov::<64>(dst, src, false);
        mov::<64>(dst, src, true);
    } else {
        dst.copy_from_slice(src);
    }
}

/// Sets `N` bytes at the start (or the end) of `dst` to `c`, as one fixed-size assignment.
#[inline(always)]
fn set<const N: usize>(dst: &mut [u8], c: u8, end: bool) {
    if let Some(d) = if end {
        dst.last_chunk_mut::<N>()
    } else {
        dst.first_chunk_mut::<N>()
    } {
        *d = [c; N];
    }
}

/// Fills `dst` with `c`, as [`copy_to`] copies.
#[inline(always)]
fn fill_to(dst: &mut [u8], c: u8) {
    let n = dst.len();
    if n > 64 || (ONE_CALL_FROM_32 && n >= 32) {
        dst.fill(c);
    } else if n >= 32 {
        set::<32>(dst, c, false);
        set::<32>(dst, c, true);
    } else if n >= 16 {
        set::<16>(dst, c, false);
        set::<16>(dst, c, true);
    } else if n >= 8 {
        set::<8>(dst, c, false);
        set::<8>(dst, c, true);
    } else if n >= 4 {
        set::<4>(dst, c, false);
        set::<4>(dst, c, true);
    } else {
        for p in dst {
            *p = c;
        }
    }
}

/// Runs the loop `$body` with `$copy` bound to the copy that suits runs of `$n` pixels, so the choice is made once for all the
/// rows of a run and not again for each.
macro_rules! by_length {
    ($n:expr, |$copy:ident| $body:expr) => {
        match $n {
            32.. if ONE_CALL_FROM_32 => {
                let $copy = |d: &mut [u8], s: &[u8]| d.copy_from_slice(s);
                $body
            }
            129.. => {
                let $copy = |d: &mut [u8], s: &[u8]| d.copy_from_slice(s);
                $body
            }
            64..=128 => {
                let $copy = |d: &mut [u8], s: &[u8]| {
                    mov::<64>(d, s, false);
                    mov::<64>(d, s, true);
                };
                $body
            }
            32..=63 => {
                let $copy = |d: &mut [u8], s: &[u8]| {
                    mov::<32>(d, s, false);
                    mov::<32>(d, s, true);
                };
                $body
            }
            16..=31 => {
                let $copy = |d: &mut [u8], s: &[u8]| {
                    mov::<16>(d, s, false);
                    mov::<16>(d, s, true);
                };
                $body
            }
            _ => {
                let $copy = |d: &mut [u8], s: &[u8]| copy_to(d, s);
                $body
            }
        }
    };
}

/// Copies `dst.len()` pixels of a table row into `dst`, starting at column `u` of the row, which repeats every [`PERIOD`].
#[inline(always)]
fn blit(dst: &mut [u8], row: &[u8], u: usize) {
    let n = dst.len();
    if u + n <= PERIOD {
        copy_to(dst, &row[u..u + n]);
        return;
    }
    let first = PERIOD - u;
    copy_to(&mut dst[..first], &row[u..PERIOD]);
    let mut done = first;
    while done < n {
        let c = (n - done).min(PERIOD);
        copy_to(&mut dst[done..done + c], &row[..c]);
        done += c;
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
    fn fill(&mut self, x: i32, y: i32, len: i32, c: u8) {
        if let Some((i, _, n)) = self.span(x, y, len) {
            fill_to(&mut self.px[i..i + n], c);
            count(n);
        }
    }

    /// Fills the rectangle `len` wide and `rows` tall with its top left at (`x`, `y`), clipped, one fill for each row.
    fn fill_rows(&mut self, x: i32, y: i32, rows: i32, len: i32, c: u8) {
        let (k0, k1) = ((-y).clamp(0, rows), (self.h - y).clamp(0, rows));
        let (a, b) = (x.max(0), x.saturating_add(len).min(self.w));
        if k0 >= k1 || a >= b {
            return;
        }
        let (n, w) = ((b - a) as usize, self.w as usize);
        let mut i = (y + k0) as usize * w + a as usize;
        for _ in k0..k1 {
            fill_to(&mut self.px[i..i + n], c);
            count(n);
            i += w;
        }
    }

    /// Copies `rows` rows of a baked table, `len` pixels wide from `x`: pixel row `y + k` takes table row `(first + k) %
    /// modulus` from world column `u0` (the left edge of the unclipped run), which repeats every 256. The clip is worked out
    /// once for all the rows.
    #[allow(clippy::too_many_arguments)]
    fn table_rows(
        &mut self,
        x: i32,
        y: i32,
        rows: i32,
        len: i32,
        table: &[u8],
        first: usize,
        modulus: usize,
        u0: i32,
    ) {
        let (k0, k1) = ((-y).clamp(0, rows), (self.h - y).clamp(0, rows));
        let (a, b) = (x.max(0), x.saturating_add(len).min(self.w));
        if k0 >= k1 || a >= b {
            return;
        }
        let (n, w) = ((b - a) as usize, self.w as usize);
        let u = (u0.wrapping_add(a - x) & (PERIOD as i32 - 1)) as usize;
        let mut ri = (first + k0 as usize) % modulus;
        let mut di = (y + k0) as usize * w + a as usize;
        let mut left = (k1 - k0) as usize;
        if u + n > PERIOD {
            // The run crosses the end of the period: each row is two copies.
            for _ in 0..left {
                blit(&mut self.px[di..di + n], row_of(table, ri), u);
                count(n);
                di += w;
                ri = if ri + 1 == modulus { 0 } else { ri + 1 };
            }
            return;
        }
        by_length!(n, |copy| while left > 0 {
            // The rows up to the end of the table, with no check for it in the loop.
            let chunk = left.min(modulus - ri);
            let mut si = ri * PERIOD + u;
            for _ in 0..chunk {
                copy(&mut self.px[di..di + n], &table[si..si + n]);
                count(n);
                di += w;
                si += PERIOD;
            }
            left -= chunk;
            ri = 0;
        });
    }

    /// Draws the top face of a run: the 26 strip rows of `table` from the far edge at row `y`, each shifted right by its
    /// slant `O[s]` from `left`, `len` pixels wide from world column `u0`, with the black caps at either end where asked.
    #[allow(clippy::too_many_arguments)]
    fn top_face(
        &mut self,
        left: i32,
        y: i32,
        len: i32,
        table: &[u8],
        u0: i32,
        cap_l: bool,
        cap_r: bool,
    ) {
        let (s0, s1) = ((-y).clamp(0, STRIP), (self.h - y).clamp(0, STRIP));
        if s0 >= s1 {
            return;
        }
        let w = self.w as usize;
        if left >= 0 && left.saturating_add(len + i32::from(O[0])) <= self.w {
            // No row reaches either edge of the frame: every row is one copy and its caps.
            let (n, u) = (len as usize, (u0 & (PERIOD as i32 - 1)) as usize);
            let mut i = (y + s0) as usize * w + left as usize;
            let rows = s0 as usize..s1 as usize;
            if u + n <= PERIOD {
                by_length!(n, |copy| for s in rows.clone() {
                    let x = i + usize::from(O[s]);
                    let si = s * PERIOD + u;
                    copy(&mut self.px[x..x + n], &table[si..si + n]);
                    count(n);
                    if cap_l {
                        self.px[x] = BLACK;
                        count(1);
                    }
                    if cap_r {
                        self.px[x + n - 1] = BLACK;
                        count(1);
                    }
                    i += w;
                });
                return;
            }
            for s in rows {
                let x = i + usize::from(O[s]);
                blit(&mut self.px[x..x + n], row_of(table, s), u);
                count(n);
                if cap_l {
                    self.px[x] = BLACK;
                    count(1);
                }
                if cap_r {
                    self.px[x + n - 1] = BLACK;
                    count(1);
                }
                i += w;
            }
            return;
        }
        for s in s0..s1 {
            let x = left + i32::from(O[s as usize]);
            let (a, b) = (x.max(0), x.saturating_add(len).min(self.w));
            let base = (y + s) as usize * w;
            if a < b {
                let n = (b - a) as usize;
                let u = (u0.wrapping_add(a - x) & (PERIOD as i32 - 1)) as usize;
                blit(
                    &mut self.px[base + a as usize..base + a as usize + n],
                    row_of(table, s as usize),
                    u,
                );
                count(n);
            }
            // The caps are single pixels, written when they lie in the frame.
            if cap_l && x >= 0 && x < self.w {
                self.px[base + x as usize] = BLACK;
                count(1);
            }
            let xr = x.saturating_add(len - 1);
            if cap_r && xr >= 0 && xr < self.w {
                self.px[base + xr as usize] = BLACK;
                count(1);
            }
        }
    }

    /// Draws a side face with its reference point at (`x`, `y`): one copy for each row.
    fn side(&mut self, p: &Side, x: i32, y: i32) {
        let left = x + i32::from(p.ox);
        let top = y + i32::from(p.oy);
        let first = (-top).clamp(0, i32::from(p.h)) as usize;
        let last = (self.h - top).clamp(0, i32::from(p.h)) as usize;
        if first >= last {
            return;
        }
        let w = self.w as usize;
        if left >= 0 && left + i32::from(p.w) <= self.w {
            // The whole face is inside the frame sideways.
            let mut i = (top + first as i32) as usize * w + left as usize;
            for &(start, len, off) in &p.spans[first..last] {
                let (start, n, off) = (usize::from(start), usize::from(len), usize::from(off));
                copy_to(
                    &mut self.px[i + start..i + start + n],
                    &p.pixels[off..off + n],
                );
                count(n);
                i += w;
            }
            return;
        }
        for (sy, &(start, len, off)) in p.spans.iter().enumerate().take(last).skip(first) {
            let rx = left + i32::from(start);
            let (a, b) = (rx.max(0), (rx + i32::from(len)).min(self.w));
            if a >= b {
                continue;
            }
            let n = (b - a) as usize;
            let src = usize::from(off) + (a - rx) as usize;
            let i = (top + sy as i32) as usize * w + a as usize;
            copy_to(&mut self.px[i..i + n], &p.pixels[src..src + n]);
            count(n);
        }
    }

    /// Draws a piece with its reference point at (`x`, `y`): its opaque runs, row by row.
    fn piece(&mut self, p: &Piece, x: i32, y: i32) {
        let pw = usize::from(p.w);
        let left = x + i32::from(p.ox);
        let top = y + i32::from(p.oy);
        let first = (-top).clamp(0, i32::from(p.h)) as usize;
        let last = (self.h - top).clamp(0, i32::from(p.h)) as usize;
        let w = self.w as usize;
        let inside = left >= 0 && left + i32::from(p.w) <= self.w;
        for sy in first..last {
            let base = (top + sy as i32) as usize * w;
            for &(rx, len) in &p.runs[usize::from(p.rows[sy])..usize::from(p.rows[sy + 1])] {
                let (src, n) = (sy * pw + usize::from(rx), usize::from(len));
                if inside {
                    let i = base + left as usize + usize::from(rx);
                    copy_to(&mut self.px[i..i + n], &p.pixels[src..src + n]);
                    count(n);
                    continue;
                }
                let xa = left + i32::from(rx);
                let (a, b) = (xa.max(0), (xa + i32::from(len)).min(self.w));
                if a >= b {
                    continue;
                }
                let n = (b - a) as usize;
                let src = src + (a - xa) as usize;
                let i = base + a as usize;
                copy_to(&mut self.px[i..i + n], &p.pixels[src..src + n]);
                count(n);
            }
        }
    }
}

/// The `PERIOD` bytes of row `row` of a table.
#[inline]
fn row_of(table: &[u8], row: usize) -> &[u8] {
    &table[row * PERIOD..(row + 1) * PERIOD]
}

// ---------- what a cell is drawn as ----------

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

const BACKS: [Back; 4] = [Back::Skip, Back::Open, Back::Wall, Back::WallShadow];

/// What a cell is drawn as. Cells with equal looks and no door art draw as one run.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
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

const KINDS: [Kind; 8] = [
    Kind::None,
    Kind::Stone1,
    Kind::Stone2,
    Kind::Stone3,
    Kind::Brick,
    Kind::Plank,
    Kind::DoorRed,
    Kind::DoorBlue,
];

impl Kind {
    /// Whether neighbouring cells of this kind can be drawn as one run (a door's art differs by its place in the door).
    fn merges(self) -> bool {
        !matches!(self, Kind::None | Kind::DoorRed | Kind::DoorBlue)
    }
}

/// A cell's look is its kind (bits 0 to 2) and whether it has a top face (bit 3); its background is bits 4 and 5. A cell's
/// `info` byte holds both, so a run of equal looks is a run of equal low nibbles.
const LOOK_MASK: u8 = 0x0F;
const TOP_BIT: u8 = 8;

/// The background of a cell: see [`Back`] and [`back_is_wall`].
fn back_info(sm: u8, t: u8, tr: u8, tu: u8, tur: u8) -> u8 {
    let (ct, cu) = (CLASS[usize::from(t)], CLASS[usize::from(tu)]);
    let back = if ct & sm != 0
        && CLASS[usize::from(tr)] & sm != 0
        && cu & sm != 0
        && CLASS[usize::from(tur)] & sm != 0
    {
        Back::Skip
    } else if !(t == WALLBG || (ct & C_OPEN == 0 && cu & C_OPEN == 0)) {
        Back::Open
    } else if t == WALLBG && cu & sm != 0 {
        Back::WallShadow
    } else {
        Back::Wall
    };
    (back as u8) << 4
}

/// The look of a cell whose tile is `t`, with the tile above it `tu`, given the depth of its stone if it is stone.
fn look_info(g: &Ground, sm: u8, t: u8, tu: u8, depth: impl FnOnce() -> u8) -> u8 {
    let kind = match t {
        FILL => match depth() {
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
    let top = kind == Kind::Plank || (kind != Kind::None && CLASS[usize::from(tu)] & sm == 0);
    kind as u8 | if top { TOP_BIT } else { 0 }
}

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

// ---------- the cell window ----------

/// The window's arrays come in two sizes, in cells: the 320 by 200 picture of the DOS presenter and the Web's default view
/// reach 29 by 23 cells with the margin, and the widest Web view (1280 by 416 pixels) 89 by 34. A view too large for the larger
/// (more than [`LARGE_CELLS`] cells, [`MAX_COLS`] across or [`MAX_ROWS`] down) is drawn without a window.
const SMALL_CELLS: usize = 768;
const LARGE_CELLS: usize = 3840;
const MAX_COLS: usize = 96;
const MAX_ROWS: usize = 40;
/// The cells the window holds beyond those the view reaches, in each direction: the neighbours that drawing a cell reads.
const MARGIN: i32 = 3;

/// The map as one draw sees it: the tile of every cell the view reaches and of a margin round them (cells outside the map
/// repeat the nearest edge cell), and for each cell the view reaches its look and its background, worked out once. Drawing
/// reads this and never asks the map for a neighbour twice. A view too large for the arrays has no window (`ok` is false) and
/// every question is answered from the map directly, which is slower and gives the same answers.
struct Window<'a, 'b> {
    g: Ground<'a>,
    /// The class bits that make a tile solid now (the doors shut or open).
    sm: u8,
    ok: bool,
    /// The column and row of the window's top left cell, and its size in cells.
    c0: i32,
    r0: i32,
    nc: usize,
    nr: usize,
    /// The tile of each cell, `nc` to a row.
    tile: &'b mut [u8],
    /// The look and background of each cell the view reaches, laid out like `tile`.
    info: &'b mut [u8],
    /// The classes found in each row of the window.
    classes: [u8; MAX_ROWS],
}

impl<'a, 'b> Window<'a, 'b> {
    /// The window over the cells `c_lo..=c_hi` of the screen rows `r_lo..=r_hi`, which are the cells the view reaches, held in
    /// the two arrays if they are big enough.
    fn new(
        g: &Ground<'a>,
        tile: &'b mut [u8],
        info: &'b mut [u8],
        c_lo: i32,
        c_hi: i32,
        r_lo: i32,
        r_hi: i32,
    ) -> Self {
        let (nc, nr) = (
            (c_hi - c_lo + 1 + 2 * MARGIN).max(0) as usize,
            (r_hi - r_lo + 1 + 2 * MARGIN).max(0) as usize,
        );
        let ok = nc <= MAX_COLS
            && nr <= MAX_ROWS
            && nc > 2 * MARGIN as usize
            && nr > 2 * MARGIN as usize
            && nc * nr <= tile.len().min(info.len());
        let mut win = Window {
            g: *g,
            sm: g.solid_mask(),
            ok,
            c0: c_lo - MARGIN,
            r0: r_lo - MARGIN,
            nc,
            nr,
            tile,
            info,
            classes: [0; MAX_ROWS],
        };
        if ok {
            win.fill(c_lo, c_hi, r_lo, r_hi);
        }
        win
    }

    /// Reads the tiles, then works out the look and background of the cells the view reaches.
    fn fill(&mut self, c_lo: i32, c_hi: i32, r_lo: i32, r_hi: i32) {
        let g = self.g;
        let (gw, gh) = (g.w as usize, g.h);
        let (nc, nr) = (self.nc, self.nr);
        // The cells left of the map and right of it, and the columns of the map between.
        let left = (-self.c0).clamp(0, nc as i32) as usize;
        let right = (self.c0 + nc as i32 - g.w).clamp(0, (nc - left) as i32) as usize;
        let mid = nc - left - right;
        let from = self.c0.clamp(0, g.w) as usize;
        let mut fill_seen = false;
        for j in 0..nr {
            let ty = (gh - 1 - (self.r0 + j as i32)).clamp(0, gh - 1) as usize;
            let src = &g.data[ty * gw..ty * gw + gw];
            let dst = &mut self.tile[j * nc..(j + 1) * nc];
            dst[..left].fill(src[0]);
            dst[left..left + mid].copy_from_slice(&src[from..from + mid]);
            dst[left + mid..].fill(src[gw - 1]);
            let cls = dst.iter().fold(0, |acc, &t| acc | CLASS[usize::from(t)]);
            self.classes[j] = cls;
            fill_seen |= cls & C_FILL != 0;
        }
        // Stone depth, for every cell of fill: whether an opening lies within one cell and within two, as bits of a mask for
        // each column of the view (bit `p` is map row `tya - 2 + p`). A column's `near` mask is the openings in it and the
        // columns either side, `far` those within two columns; a cell is at distance 1 if any of three bits of `near` round
        // it is set, and at distance 2 if any of five of `far` are. Cells outside the map are not openings, and a column or
        // row outside the map has the depth of the nearest edge cell.
        let ncore = (c_hi - c_lo + 1) as usize;
        let (txa, txb) = (c_lo.clamp(0, g.w - 1), c_hi.clamp(0, g.w - 1));
        let (tya, tyb) = (
            (gh - 1 - r_hi).clamp(0, gh - 1),
            (gh - 1 - r_lo).clamp(0, gh - 1),
        );
        let mut near = [0u64; MAX_COLS];
        let mut far = [0u64; MAX_COLS];
        if fill_seen {
            let mut m = [0u64; MAX_COLS + 4];
            let (ca, cb) = ((txa - 2).max(0), (txb + 2).min(g.w - 1));
            for ty in (tya - 2).max(0)..=(tyb + 2).min(gh - 1) {
                let bit = (ty - tya + 2) as u32;
                let src = &g.data[ty as usize * gw + ca as usize..=ty as usize * gw + cb as usize];
                for (tx, &t) in (ca..).zip(src) {
                    if !mass(t) {
                        m[(tx - txa + 2) as usize] |= 1 << bit;
                    }
                }
            }
            for (i, (n, f)) in near.iter_mut().zip(far.iter_mut()).enumerate().take(ncore) {
                let x = (c_lo + i as i32).clamp(0, g.w - 1) - txa;
                let x = x as usize;
                *n = m[x + 1] | m[x + 2] | m[x + 3];
                *f = *n | m[x] | m[x + 4];
            }
        }
        let sm = self.sm;
        let dc0 = MARGIN as usize;
        for r in r_lo..=r_hi {
            let j = (r - self.r0) as usize;
            let (cur, up) = (j * nc + dc0, (j - 1) * nc + dc0);
            let (cur_row, up_row) = (
                &self.tile[cur..cur + ncore + 1],
                &self.tile[up..up + ncore + 1],
            );
            let p = ((gh - 1 - r).clamp(0, gh - 1) - tya + 2) as u32;
            let out = &mut self.info[cur..cur + ncore];
            // A cell's look and background depend only on its tile, the tile to its right and the two above them (and, for
            // fill, on its depth), so a cell that matches the one before it is not worked out again.
            let (mut last_key, mut last) = (u32::MAX, 0u8);
            for (i, o) in out.iter_mut().enumerate() {
                let (t, tr, tu, tur) = (cur_row[i], cur_row[i + 1], up_row[i], up_row[i + 1]);
                let key = u32::from_le_bytes([t, tr, tu, tur]);
                if key != last_key {
                    last_key = key;
                    // Fill is looked up as the deepest stone; its depth is put in below.
                    last = look_info(&g, sm, t, tu, || 4) | back_info(sm, t, tr, tu, tur);
                }
                *o = if t == FILL {
                    let kind = if (near[i] >> (p - 1)) & 7 != 0 {
                        Kind::Stone1
                    } else if (far[i] >> (p - 2)) & 31 != 0 {
                        Kind::Stone2
                    } else {
                        Kind::Stone3
                    };
                    last & !7 | kind as u8
                } else {
                    last
                };
            }
        }
    }

    /// The index of cell (`c`, `r`) in the window's arrays, if it lies in the window.
    #[inline(always)]
    fn at(&self, c: i32, r: i32) -> Option<usize> {
        let (dc, dr) = (
            c.wrapping_sub(self.c0) as u32,
            r.wrapping_sub(self.r0) as u32,
        );
        (self.ok && dc < self.nc as u32 && dr < self.nr as u32)
            .then(|| dr as usize * self.nc + dc as usize)
    }

    /// The tile at column `c` and screen row `r` (0 at the top of the map; the map is `h` rows). Outside the map it is the
    /// nearest edge cell.
    #[inline(always)]
    fn tile(&self, c: i32, r: i32) -> u8 {
        match self.at(c, r) {
            Some(i) => self.tile[i],
            None => self.g.cell(c, r),
        }
    }

    #[inline(always)]
    fn solid(&self, t: u8) -> bool {
        CLASS[usize::from(t)] & self.sm != 0
    }

    /// The `info` byte of a cell the view reaches: from the window, or worked out from the map when there is none.
    #[inline(always)]
    fn info(&self, c: i32, r: i32) -> u8 {
        match self.at(c, r) {
            Some(i) => self.info[i],
            None => self.info_direct(c, r),
        }
    }

    /// The `info` bytes of the cells `c_lo..=c_hi` of screen row `r`, if the window holds them all.
    #[inline(always)]
    fn info_row(&self, r: i32, c_lo: i32, c_hi: i32) -> Option<&[u8]> {
        let (a, b) = (self.at(c_lo, r)?, self.at(c_hi, r)?);
        self.info.get(a..=b)
    }

    /// The `info` byte of a cell from the map alone, as [`Window::fill`] works it out.
    #[inline(never)]
    fn info_direct(&self, c: i32, r: i32) -> u8 {
        let g = &self.g;
        let (t, tr) = (g.cell(c, r), g.cell(c + 1, r));
        let (tu, tur) = (g.cell(c, r - 1), g.cell(c + 1, r - 1));
        look_info(g, self.sm, t, tu, || depth_direct(g, c, g.h - 1 - r))
            | back_info(self.sm, t, tr, tu, tur)
    }

    /// Whether the background behind cell (`c`, `r`) is the back wall. A wall tile is; open air is not; any other tile (a
    /// ledge, spikes, a door, the exposed edge of a solid) takes the background of the cell above if that is open air and the
    /// back wall otherwise.
    #[inline]
    fn back_is_wall(&self, c: i32, r: i32) -> bool {
        let (t, tu) = match self.at(c, r) {
            Some(i) if i >= self.nc => (self.tile[i], self.tile[i - self.nc]),
            _ => (self.tile(c, r), self.tile(c, r - 1)),
        };
        t == WALLBG || (!open_air(t) && !open_air(tu))
    }

    /// The first column from `c` to `c_end` inclusive of screen row `r` whose tile has one of the class bits `any`.
    fn find(&self, r: i32, c: i32, c_end: i32, any: u8) -> Option<i32> {
        if c > c_end {
            return None;
        }
        let j = r.wrapping_sub(self.r0) as u32;
        if self.ok && j < self.nr as u32 && c >= self.c0 && c_end < self.c0 + self.nc as i32 {
            // A row of the window with none of the classes needs no scan; one with some is scanned as a slice.
            if self.classes[j as usize] & any == 0 {
                return None;
            }
            let from = j as usize * self.nc + (c - self.c0) as usize;
            let row = &self.tile[from..from + (c_end - c + 1) as usize];
            return row
                .iter()
                .position(|&t| CLASS[usize::from(t)] & any != 0)
                .map(|k| c + k as i32);
        }
        (c..=c_end).find(|&x| CLASS[usize::from(self.tile(x, r))] & any != 0)
    }
}

// ---------- backgrounds ----------

/// The camera height, in map pixels, the Lighthouse stack's horizon is calibrated at: the view's top when Ben stands on the
/// gallery floor (his centre is 38.75 tiles up, so a 200 px view starts 48 px down the map). At this camera the sea's
/// horizon lies on the top of map row [`HORIZON_ROW`], as the gallery was designed; from lower in the level the layers have
/// scrolled by their factors and the horizon has risen on the screen, and indoors none of it shows.
pub const HORIZON_CAM_Y: i32 = 48;

/// Layer factors out of 256: the Lighthouse Point board's 0.04, 0.06, 0.12 and 0.18 (Fizz 1's `Layer` vocabulary: a layer at
/// factor `f` is offset by `camera * (1 - f)`, so 0 stays on the screen and 1 moves with the tiles).
const F_STARS: u16 = 10;
const F_CLOUDS: u16 = 15;
const F_HEADLANDS_X: u16 = 31;
const F_SEA_X: u16 = 46;
/// The vertical factor of every layer but the stars: 0.06.
const F_Y: u16 = 15;

/// The far headlands' tile: filled D where `y >= |x - 7.5| * 1.8 + 2`, in the integer form `10 y >= 9 |2x - 15| + 20`, 255
/// elsewhere. Row 0 is the surface.
const MTN_TOP: [u8; 256] = {
    let mut t = [255u8; 256];
    let mut y = 0;
    while y < 16 {
        let mut x = 0;
        while x < 16 {
            let d = 2 * x as i32 - 15;
            if 10 * y as i32 >= 9 * (if d < 0 { -d } else { d }) + 20 {
                t[y * 16 + x] = DARK_GREY;
            }
            x += 1;
        }
        y += 1;
    }
    t
};

/// The sea's tile: row 10 is a C crest where `x % 6 < 4`, row 11 C, rows 12 and 13 a B and k checker, rows 14 and 15 B, and
/// the rows above it are 255. Row [`SEA_SURFACE_ROW`] is the surface.
const SEA_TOP: [u8; 256] = {
    let mut t = [255u8; 256];
    let mut y = 10;
    while y < 16 {
        let mut x = 0;
        while x < 16 {
            t[y * 16 + x] = match y {
                10 => {
                    if x % 6 < 4 {
                        CYAN
                    } else {
                        255
                    }
                }
                11 => CYAN,
                12 | 13 => {
                    if (x + y) % 2 == 0 {
                        BLUE
                    } else {
                        BLACK
                    }
                }
                _ => BLUE,
            };
            x += 1;
        }
        y += 1;
    }
    t
};
const SEA_SURFACE_ROW: i32 = 10;

/// The slice's open-air theme, back to front: the Lighthouse Point night. Stars, drifting clouds, the far headlands (D, stepped
/// heights from the baked [`HEADLAND_HEIGHTS`]) and a flat sea whose crest is the horizon. This is the slice's theme as
/// constants in this module; a later level and theme format will carry the stack, and nothing here invents one.
fn lighthouse<'a>(g: &Ground) -> [Layer<'a>; 4] {
    // The horizon in layer space: the top of map row `HORIZON_ROW`, less what the layers have scrolled at the calibration camera.
    let horizon = (g.h - 1 - HORIZON_ROW) * T - fizz_view::parallax::offset(HORIZON_CAM_Y, F_Y);
    [
        Layer::Stars {
            fx: F_STARS,
            fy: F_STARS,
        },
        Layer::Clouds {
            fx: F_CLOUDS,
            fy: F_Y,
            rows: 2,
        },
        Layer::Band {
            fx: F_HEADLANDS_X,
            fy: F_Y,
            base_y: horizon,
            heights: &HEADLAND_HEIGHTS,
            top: &MTN_TOP,
            surface_row: 0,
            fill: DARK_GREY,
        },
        Layer::Band {
            fx: F_SEA_X,
            fy: F_Y,
            base_y: horizon,
            heights: &[0],
            top: &SEA_TOP,
            surface_row: SEA_SURFACE_ROW,
            fill: BLUE,
        },
    ]
}

/// The open-air stack at a level of detail: the whole [`lighthouse`] night for Full, and for Light only its stars and its sea,
/// which keep the horizon and the night where the drifting clouds and the far headlands cost the most to draw (about 55,000 of
/// the 80,000 instructions the night adds to a gallery frame on the DOS build).
fn stack<'a>(g: &Ground, full_sky: bool) -> ([Layer<'a>; 4], usize) {
    let [stars, clouds, headlands, sea] = lighthouse(g);
    if full_sky {
        ([stars, clouds, headlands, sea], 4)
    } else {
        ([stars, sea, sea, sea], 2)
    }
}

/// Fills a rectangle of the view (`x0..x1`, `y0..y1`) with open air: the parallax stack of the slice's theme, clipped to the
/// rectangle, so a layer's pixel appears only where a cell is open air and interior cells show the back wall.
#[allow(clippy::too_many_arguments)]
fn draw_open_air(
    cv: &mut Canvas,
    g: &Ground,
    sky: &mut Option<Sky<'static>>,
    (cam_x, cam_y, tick, full_sky): (i32, i32, u32, bool),
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
) {
    let clip = Clip {
        x0: x0.max(0),
        y0: y0.max(0),
        x1: x1.min(cv.w),
        y1: y1.min(cv.h),
    };
    if clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
        return;
    }
    // The layers write each pixel of the rectangle once (the clear) and the headlands and sea once more where they are; the
    // overdraw test counts it as twice, which is above what the sea and headlands cover.
    count(2 * ((clip.x1 - clip.x0) * (clip.y1 - clip.y0)) as usize);
    let (w, h) = (cv.w as usize, cv.h as usize);
    // The stack is prepared for the frame once, when its first rectangle is drawn, and each rectangle then only composes rows.
    let sky = sky.get_or_insert_with(|| {
        let (layers, n) = stack(g, full_sky);
        Sky::new(w, h, cam_x, cam_y, &layers[..n], tick)
    });
    sky.draw(cv.px, w, h, clip);
}

/// The most cells of another background between two runs of open air in a row of cells that the sky is drawn across, so that
/// the two are one rectangle: four cells, whose sky costs less than the rows of a second rectangle.
const OPEN_GAP: i32 = 4;

/// Draws the background of every cell the view touches.
fn backgrounds(
    cv: &mut Canvas,
    g: &Ground,
    win: &Window<'_, '_>,
    (cam_x, cam_y, tick, full_sky): (i32, i32, u32, bool),
) {
    let (r0, r1) = (cam_y.div_euclid(T), (cam_y + cv.h - 1).div_euclid(T));
    let (c0, c1) = (cam_x.div_euclid(T), (cam_x + cv.w - 1).div_euclid(T));
    let mut sky = None;
    for r in r0..=r1 {
        let ya = (r * T).max(cam_y) - cam_y;
        let yb = ((r + 1) * T).min(cam_y + cv.h) - cam_y;
        let row = win.info_row(r, c0, c1);
        let back = |c: i32| match row {
            Some(row) => row[(c - c0) as usize] >> 4,
            None => win.info(c, r) >> 4,
        };
        // Open air first, as one rectangle from the first open-air cell of the row to the last, across gaps of up to
        // `OPEN_GAP` other cells: the sky's cost is mostly for each row of a rectangle, and what it draws in a gap is drawn
        // over next by the gap's own background (or, for a cell that needs none, by the solid cells that cover it).
        let mut c = c0;
        while c <= c1 {
            if BACKS[usize::from(back(c))] != Back::Open {
                c += 1;
                continue;
            }
            let (mut e, mut end) = (c + 1, c + 1);
            while e <= c1 && e - end < OPEN_GAP {
                if BACKS[usize::from(back(e))] == Back::Open {
                    end = e + 1;
                }
                e += 1;
            }
            let (x, len) = (c * T - cam_x, (end - c) * T);
            draw_open_air(
                cv,
                g,
                &mut sky,
                (cam_x, cam_y, tick, full_sky),
                x,
                x + len,
                ya,
                yb,
            );
            c = end;
        }
        let mut c = c0;
        while c <= c1 {
            let kind = back(c);
            if matches!(BACKS[usize::from(kind)], Back::Skip | Back::Open) {
                c += 1;
                continue;
            }
            // A run takes in the cells of the same background and, between them, cells that need none: four solid cells cover
            // those, so what is drawn under them is never seen.
            let (mut e, mut last) = (c + 1, c + 1);
            while e <= c1 {
                let k = back(e);
                if k == kind {
                    last = e + 1;
                } else if BACKS[usize::from(k)] != Back::Skip {
                    break;
                }
                e += 1;
            }
            let e = last;
            let x = c * T - cam_x;
            let len = (e - c) * T;
            match BACKS[usize::from(kind)] {
                Back::Skip | Back::Open => {}
                Back::Wall => {
                    let first = (ya + cam_y).rem_euclid(96) as usize;
                    cv.table_rows(x, ya, yb - ya, len, &BACK_WALL, first, 96, c * T);
                }
                Back::WallShadow => {
                    // The shadow of the solid cell above: black on the first 13 rows of the cell, thinning to a dither on
                    // the next two.
                    let in_cell = ya + cam_y - r * T;
                    let black = (13 - in_cell).clamp(0, yb - ya);
                    cv.fill_rows(x, ya, black, len, BLACK);
                    let wall = ya + black;
                    let first = (wall + cam_y).rem_euclid(96) as usize;
                    cv.table_rows(x, wall, yb - wall, len, &BACK_WALL, first, 96, c * T);
                    for k in in_cell.max(13)..(in_cell + yb - ya).min(15) {
                        let y = r * T + k - cam_y;
                        if let Some((row, off)) = cv.row_mut(x, y, len) {
                            // The last two rows of the shadow are a dither: black where the world column and row add to odd.
                            let odd = (x + off + cam_x + y + cam_y) & 1;
                            for p in row.iter_mut().skip(usize::from(odd == 0)).step_by(2) {
                                *p = BLACK;
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

/// The contact shadows under the plank run `c0..c1` of screen row `r`: three rows on the back wall under the run, shortening
/// towards the near edge and dithered, from the near plane's left edge to its right (and a depth further at the end of a run,
/// if `end`: the cell after the run is not a plank). Each cell's shadow is the same as if it were drawn by itself, and a shadow
/// reaches no pixel that a bracket of the run covers, so the run is drawn in one piece.
fn plank_shadows(
    cv: &mut Canvas,
    win: &Window<'_, '_>,
    c0: i32,
    c1: i32,
    r: i32,
    end: bool,
    cam: (i32, i32),
) {
    // The shadow lies on the back wall only under the cells whose background is the wall: the cell to the left of the run (its
    // first four pixels), the cells of the run, and the cell after it.
    let mut wall = [false; MAX_COLS + 4];
    let cells = ((c1 - c0 + 2) as usize).min(wall.len());
    for (i, w) in wall.iter_mut().enumerate().take(cells) {
        *w = win.back_is_wall(c0 - 1 + i as i32, r + 1);
    }
    for k in 0..3 {
        let y = r * T + T + k - cam.1;
        let a = c0 * T - FEET_DX;
        let b = if end {
            c1 * T + 6 - 2 * k
        } else {
            c1 * T - FEET_DX
        };
        let mut start = None;
        let mut x = a;
        while x < b {
            let cell = x.div_euclid(T);
            let lit = wall
                .get((cell - (c0 - 1)) as usize)
                .copied()
                .unwrap_or_else(|| win.back_is_wall(cell, r + 1));
            match (lit, start) {
                (true, None) => start = Some(x),
                (false, Some(s)) => {
                    shadow_row(cv, s, x, y, k, cam.0);
                    start = None;
                }
                _ => {}
            }
            x = ((cell + 1) * T).min(b).max(x + 1);
        }
        if let Some(s) = start {
            shadow_row(cv, s, b, y, k, cam.0);
        }
    }
}

/// One row of a contact shadow, over the world columns `x0..x1`: black on its first row, then every other pixel, where the
/// world column plus the row of the shadow is even.
fn shadow_row(cv: &mut Canvas, x0: i32, x1: i32, y: i32, k: i32, cam_x: i32) {
    if k == 0 {
        cv.fill(x0 - cam_x, y, x1 - x0, BLACK);
    } else if let Some((row, off)) = cv.row_mut(x0 - cam_x, y, x1 - x0) {
        let wx0 = x0 + off;
        for p in row.iter_mut().skip(((wx0 + k) & 1) as usize).step_by(2) {
            *p = BLACK;
        }
    }
}

/// The brackets under a plank run: at its left end, its right end and (for a long run) its middle, where the neighbouring cell
/// is not solid. `c` is the cell being drawn and `s..e` the cells of its run; only the brackets that lie in `c` are drawn, so
/// each is drawn once.
fn plank_brackets(
    cv: &mut Canvas,
    win: &Window<'_, '_>,
    c: i32,
    r: i32,
    run: (i32, i32),
    cam: (i32, i32),
) {
    let (s, e) = run;
    let mut spot = |bx: i32| {
        if bx.div_euclid(T) == c {
            cv.piece(&BRACKET, bx - cam.0, r * T + T - cam.1);
        }
    };
    if !win.solid(win.tile(s - 1, r)) {
        spot(s * T + 6);
    }
    if !win.solid(win.tile(e, r)) {
        spot(e * T - 10);
    }
    if e - s >= BRACKET_MID_RUN {
        spot((s + e) * T / 2);
    }
}

/// The cells of the plank run that holds cell `c` of screen row `r`, as `(first, one past the last)`; outside the map there is
/// no plank.
fn plank_run(win: &Window<'_, '_>, c: i32, r: i32) -> (i32, i32) {
    let tile = |x: i32| -> u8 {
        if x < 0 || x >= win.g.w {
            EMPTY
        } else {
            win.tile(x, r)
        }
    };
    let mut s = c;
    while s > 0 && tile(s - 1) == PLAT {
        s -= 1;
    }
    let mut e = c + 1;
    while e < win.g.w && tile(e) == PLAT {
        e += 1;
    }
    (s, e)
}

/// Everything drawn on the far plane before the terrain, over the cells the view touches.
fn dressing(cv: &mut Canvas, win: &Window<'_, '_>, cam_x: i32, cam_y: i32) {
    let (c_lo, c_hi, r_lo, r_hi) = cell_range(cv, cam_x, cam_y);
    let g = &win.g;
    for &(pc, pr) in &PORTHOLES {
        let r = g.h - 1 - pr;
        if pc >= 0 && pc < g.w && pr >= 0 && pr < g.h && win.tile(pc, r) == WALLBG {
            cv.piece(&PORTHOLE, pc * T + 8 - cam_x, r * T + 8 - cam_y);
        }
    }
    for r in r_lo - 1..=r_hi + 1 {
        let mut next = c_lo - 2;
        while let Some(c) = win.find(r, next, c_hi + 2, C_PLAT) {
            // The planks from `c` to the end of the stretch of them that the view reaches.
            let mut e = c + 1;
            while e <= c_hi + 2 && win.tile(e, r) == PLAT {
                e += 1;
            }
            next = e;
            plank_shadows(cv, win, c, e, r, win.tile(e, r) != PLAT, (cam_x, cam_y));
            let mut run = (0, i32::MIN);
            for c in c..e {
                if win.back_is_wall(c, r) {
                    // A run is shared by its cells inside the map; a cell outside it is its own run.
                    if c < 0 || run.0 < 0 || c >= run.1 || c < run.0 {
                        run = plank_run(win, c, r);
                    }
                    plank_brackets(cv, win, c, r, run, (cam_x, cam_y));
                } else {
                    cv.piece(&RAIL, c * T - cam_x, r * T - cam_y);
                }
            }
        }
    }
    // The exit stands on the far edge of the floor: its sill is the top of its lowest cell.
    for r in r_lo - 1..=r_hi + 1 {
        let mut next = c_lo - 2;
        while let Some(c) = win.find(r, next, c_hi + 2, C_EXIT) {
            next = c + 1;
            if win.tile(c, r) == EXIT && win.tile(c, r + 1) != EXIT {
                cv.piece(&EXIT_DOORWAY, c * T - cam_x, r * T - cam_y);
            }
        }
    }
}

// ---------- terrain ----------

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

/// Draws the cells `c0..c1` of screen row `r`, which all have the look `look` (the low nibble of their `info`), as one run.
#[allow(clippy::too_many_arguments)]
fn draw_run(
    cv: &mut Canvas,
    win: &Window<'_, '_>,
    cam_x: i32,
    cam_y: i32,
    c0: i32,
    c1: i32,
    r: i32,
    look: u8,
) {
    let kind = KINDS[usize::from(look & 7)];
    let n = c1 - c0;
    let left = c0 * T - FEET_DX - cam_x;
    let y0 = r * T - cam_y;
    let u0 = c0 * T;
    let last = c1 - 1;
    let plank = kind == Kind::Plank;

    phase(PHASE_TOP);
    if look & TOP_BIT != 0 {
        // The caps are the black outline at a ledge's ends. An interior joint between two cells never has one.
        let (tl, tr) = (win.tile(c0 - 1, r), win.tile(c1, r));
        let (cap_l, cap_r) = if plank {
            (tl != PLAT && !win.solid(tl), tr != PLAT)
        } else {
            (
                !win.solid(tl) || win.solid(win.tile(c0 - 1, r - 1)),
                !win.solid(tr) || win.solid(win.tile(c1, r - 1)),
            )
        };
        let table: &[u8] = if plank { &PLANK_TOP } else { &STONE_TOP };
        cv.top_face(left, y0 - BACK, n * T, table, u0, cap_l, cap_r);
    }

    phase(PHASE_FRONT);
    let wy = r * T;
    let (fx, fy, fl) = (left, y0 + (STRIP - BACK), n * T);
    match kind {
        Kind::Stone1 => cv.table_rows(
            fx,
            fy,
            T,
            fl,
            &STONE_SHALLOW,
            wy.rem_euclid(64) as usize,
            64,
            u0,
        ),
        Kind::Stone2 => cv.table_rows(
            fx,
            fy,
            T,
            fl,
            &STONE_MID,
            wy.rem_euclid(64) as usize,
            64,
            u0,
        ),
        Kind::Stone3 => cv.table_rows(
            fx,
            fy,
            T,
            fl,
            &STONE_DEEP,
            wy.rem_euclid(64) as usize,
            64,
            u0,
        ),
        Kind::Brick => cv.table_rows(
            fx,
            fy,
            T,
            fl,
            &BRICK_FRONT,
            wy.rem_euclid(32) as usize,
            32,
            u0,
        ),
        Kind::Plank => cv.table_rows(fx, fy, PLANK_FH, fl, &PLANK_FRONT, 0, PLANK_FH as usize, u0),
        Kind::DoorRed | Kind::DoorBlue => {
            let t = win.tile(c0, r);
            let (above, below) = (win.tile(c0, r - 1) == t, win.tile(c0, r + 1) == t);
            let (top, mid, bot) = if kind == Kind::DoorRed {
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
    let right = win.tile(c1, r);
    let side = match kind {
        Kind::Plank => (right != PLAT).then_some(&SIDE_PLANK),
        Kind::Brick => (!win.solid(right)).then_some(&SIDE_BRICK),
        Kind::DoorRed => (!win.solid(right)).then_some(&SIDE_DOOR_RED),
        Kind::DoorBlue => (!win.solid(right)).then_some(&SIDE_DOOR_BLUE),
        Kind::None => None,
        _ => (!win.solid(right)).then_some(&SIDE_STONE),
    };
    if let Some(p) = side {
        cv.side(p, last * T + T - FEET_DX - cam_x, y0 - BACK);
    }
}

fn terrain(cv: &mut Canvas, win: &Window<'_, '_>, cam_x: i32, cam_y: i32) {
    let (c_lo, c_hi, r_lo, r_hi) = cell_range(cv, cam_x, cam_y);
    for r in (r_lo..=r_hi).rev() {
        let row = win.info_row(r, c_lo, c_hi);
        let look = |c: i32| {
            LOOK_MASK
                & match row {
                    Some(row) => row[(c - c_lo) as usize],
                    None => win.info(c, r),
                }
        };
        let mut c = c_lo;
        while c <= c_hi {
            let l = look(c);
            let kind = KINDS[usize::from(l & 7)];
            if kind == Kind::None {
                c += 1;
                continue;
            }
            let mut e = c + 1;
            if kind.merges() {
                while e <= c_hi && look(e) == l {
                    e += 1;
                }
            }
            draw_run(cv, win, cam_x, cam_y, c, e, r, l);
            c = e;
        }
    }
}

/// The spikes, and any other thing that stands on the floor strip without being a surface.
fn standing(cv: &mut Canvas, win: &Window<'_, '_>, cam_x: i32, cam_y: i32) {
    let (c_lo, c_hi, r_lo, r_hi) = cell_range(cv, cam_x, cam_y);
    for r in (r_lo..=r_hi).rev() {
        let mut next = c_lo;
        while let Some(c) = win.find(r, next, c_hi, C_STAND) {
            next = c + 1;
            match win.tile(c, r) {
                SPIKE => cv.piece(&SPIKES, c * T - cam_x, r * T - cam_y),
                CHOC => {
                    // Lethal chocolate: the slice has none, so it is a flat brown square on the feet plane.
                    cv.fill_rows(c * T - cam_x, r * T - cam_y, T, T, BROWN);
                }
                _ => {}
            }
        }
    }
}

/// Draws the map into the `w` by `h` frame, with the view's top left at (`cam_x`, `cam_y`) in map pixels (y down, as
/// `fizz_view::follow` gives them). Every pixel of the frame is written.
pub fn draw_ground(frame: &mut [u8], w: usize, h: usize, g: &Ground, cam_x: i32, cam_y: i32) {
    draw_ground_at(frame, w, h, g, cam_x, cam_y, 0);
}

/// [`draw_ground`] at animation tick `tick`, which drifts the clouds of the open air.
pub fn draw_ground_at(
    frame: &mut [u8],
    w: usize,
    h: usize,
    g: &Ground,
    cam_x: i32,
    cam_y: i32,
    tick: u32,
) {
    draw_ground_sky(frame, w, h, g, cam_x, cam_y, tick, true);
}

/// [`draw_ground_at`] with the open air at a level of detail: `full_sky` draws the theme's whole parallax stack (the Sky
/// setting's Full), and otherwise its stars and sea only (Light; see `stack`). Nothing but open air differs between the two.
#[allow(clippy::too_many_arguments)]
pub fn draw_ground_sky(
    frame: &mut [u8],
    w: usize,
    h: usize,
    g: &Ground,
    cam_x: i32,
    cam_y: i32,
    tick: u32,
    full_sky: bool,
) {
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
    let range = cell_range(&cv, cam_x, cam_y);
    let cells = ((range.1 - range.0 + 1 + 2 * MARGIN).max(0) as usize)
        .saturating_mul((range.3 - range.2 + 1 + 2 * MARGIN).max(0) as usize);
    // The window's arrays are on the stack, the smaller size when the view is the DOS one so that little has to be cleared.
    if cells <= SMALL_CELLS {
        draw_cells(
            &mut cv,
            g,
            &mut [0; SMALL_CELLS],
            &mut [0; SMALL_CELLS],
            (cam_x, cam_y, tick, full_sky),
        );
    } else if cells <= LARGE_CELLS {
        draw_cells(
            &mut cv,
            g,
            &mut [0; LARGE_CELLS],
            &mut [0; LARGE_CELLS],
            (cam_x, cam_y, tick, full_sky),
        );
    } else {
        draw_cells(&mut cv, g, &mut [], &mut [], (cam_x, cam_y, tick, full_sky));
    }
}

// ---------- the night ----------

/// The slice theme's night: the level bank 0 shows at night, of 14 (a third of full), which the palette carries
/// (`light::palette`). Fizz 1 gives each level theme a `night` ambient beside its day one (`LevelTheme` in
/// `episodes/episode-1/game/src/render.rs`); a later level and theme format will carry this with the Sky stack's constants.
pub const NIGHT_AMBIENT: u8 = 6;
const _: () = assert!(NIGHT_AMBIENT >= 1 && NIGHT_AMBIENT <= crate::light::ACTOR_FLOOR);

/// The lanterns over the doors, lit at night: the `(column, row)` of the pillar cell above each door that each hangs from (the
/// red door's at column 15, the blue door's at column 22). Slice dressing, like the portholes: drawn only where that cell is a
/// block.
pub const LANTERNS: [(i32, i32); 2] = [(15, 23), (22, 41)];
/// The lamps in the shafts, lit at night: the back-wall cell each is centred on, one in each shaft, clear of the ledges and
/// their shadows. Drawn only where the map still has back wall there.
pub const LAMPS: [(i32, i32); 2] = [(4, 12), (27, 30)];

/// The lights' sizes and strengths, in tiles and in `light::LIGHT_ONE` units. A lantern is Fizz 1's static light (`update_lights`,
/// a warm colour times a flicker of about 1.6), with a radius of 6 tiles where Fizz 1's is 8.5, which keeps the light in the
/// slice's narrow shafts and the cells it visits fewer; a lamp is a little weaker; the exit's doorway glows green like Fizz 1's
/// terminals (`[0.3, 1.2, 0.3]`, radius 4, height 1).
const LANTERN_LIGHT: (i32, i32, i32) = (T * 7 / 5, T * 6, 768);
const LAMP_LIGHT: (i32, i32, i32) = (T * 6 / 5, T * 5, 640);
const EXIT_LIGHT: (i32, i32, i32) = (T, T * 4, 850);

/// The lighthouse beam over the gallery: a wide, soft light at `BEAM_ROW` (screen row, 4 tiles above the gallery's floor)
/// that sweeps across the open air as the beam turns. It turns once every `BEAM_PERIOD` ticks: it is `BEAM_X` plus `BEAM_SWING`
/// times the sine of its turn across, and its strength is the cosine, so it crosses the view facing the player and is dark for
/// the half turn it faces the sea. Presentation only, from the draw tick.
const BEAM_ROW: i32 = 4;
const BEAM_X: i32 = 16 * T;
const BEAM_SWING: i32 = 22 * T;
const BEAM_PERIOD: u32 = 512;
const BEAM_LIGHT: (i32, i32, i32) = (T * 5 / 2, T * 7, 1100);

/// Colour masks, a bit for each EGA index whose pixels glow: a lamp's yellow glass and white bulb, a door badge's white, and the
/// exit's green and light green opening and white arrow.
const GLOW_LAMP: u16 = 1 << 14 | 1 << 15;
const GLOW_BADGE: u16 = 1 << 15;
const GLOW_EXIT: u16 = 1 << 2 | 1 << 10 | 1 << 15;

/// Fizz 1's flicker of a static light, `1.6 + 0.12 sin(9 t + x) + 0.08 sin(23 t + 3 x)` with `t` in seconds and `x` in tiles,
/// times 256, from the baked `SINE` table: a tick is 6 and 16 steps of its 256 (9.2 and 24.5 radians a second, the nearest whole
/// steps), and the light's x in pixels times 163 / 64 is its phase in steps (`x` radians).
pub fn flicker(tick: u32, x_px: i32) -> i32 {
    let ph = (x_px.wrapping_mul(163) >> 6) as u32;
    let a = i32::from(SINE[(tick.wrapping_mul(6).wrapping_add(ph) & 255) as usize]);
    let b = i32::from(SINE[(tick.wrapping_mul(16).wrapping_add(ph.wrapping_mul(3)) & 255) as usize]);
    410 + (31 * a + 20 * b) / 256
}

/// A light at map pixel (`x`, `y`) (y down) of `(height, radius, strength)` with its strength scaled by `k / 256`.
fn light_at(x: i32, y: i32, (z, r, s): (i32, i32, i32), k: i32) -> crate::light::Light {
    use crate::light::SUB;
    crate::light::Light {
        x: x * SUB,
        y: y * SUB,
        z: z * SUB,
        r: r * SUB,
        strength: s * k / 256,
    }
}

/// Adds the slice's own lights at animation tick `tick` to `lights`, measured from the camera's centre `cam` (map pixels times
/// `light::SUB`): the door lanterns and shaft lamps, flickering; the exit's green glow; and the lighthouse beam. Each is added
/// only where its dressing is drawn ([`draw_night`]), so a map without the cells has no lights.
pub fn night_lights(lights: &mut crate::light::Lights, g: &Ground, cam: (i32, i32), tick: u32) {
    if !g.usable() {
        return;
    }
    for &(c, row) in &LANTERNS {
        let r = g.h - 1 - row;
        if mass(g.cell(c, r)) {
            let (x, y) = lantern_at(c, r);
            lights.add(cam, light_at(x, y + 8, LANTERN_LIGHT, flicker(tick, x)));
        }
    }
    for &(c, row) in &LAMPS {
        let r = g.h - 1 - row;
        if g.cell(c, r) == WALLBG {
            let (x, y) = (c * T + 8, r * T + 8);
            lights.add(cam, light_at(x, y, LAMP_LIGHT, flicker(tick, x) * 7 / 8));
        }
    }
    for r in 0..g.h {
        for c in 0..g.w {
            if g.cell(c, r) == EXIT && g.cell(c, r + 1) != EXIT {
                // The middle of the two-tile opening, which stands on the far plane above the lower exit cell.
                lights.add(cam, light_at(c * T + 14, r * T - 12, EXIT_LIGHT, 256));
            }
        }
    }
    let turn = (tick.wrapping_mul(256) / BEAM_PERIOD) & 255;
    let facing = i32::from(SINE[((turn + 64) & 255) as usize]);
    if facing > 0 {
        let x = BEAM_X + BEAM_SWING * i32::from(SINE[turn as usize]) / 256;
        lights.add(cam, light_at(x, BEAM_ROW * T + 8, BEAM_LIGHT, facing));
    }
}

/// Where a lantern's hook hangs for the pillar cell at column `c` and screen row `r`: the middle of the cell's front face (on
/// the near plane, 4 pixels left of the tile and 10 down), two rows below its top edge.
fn lantern_at(c: i32, r: i32) -> (i32, i32) {
    (c * T - FEET_DX + 8, r * T + (STRIP - BACK) + 2)
}

impl Canvas<'_> {
    /// Sets the glow bank on the pixels of `p` placed as [`Canvas::piece`] places it whose colour is in `mask` and which still
    /// show that colour (so nothing drawn over the piece since is marked).
    fn glow_piece(&mut self, p: &Piece, x: i32, y: i32, mask: u16) {
        let pw = usize::from(p.w);
        let (left, top) = (x + i32::from(p.ox), y + i32::from(p.oy));
        for sy in 0..usize::from(p.h) {
            for &(rx, len) in &p.runs[usize::from(p.rows[sy])..usize::from(p.rows[sy + 1])] {
                let Some((i, src, n)) = self.span(left + i32::from(rx), top + sy as i32, i32::from(len)) else {
                    continue;
                };
                let src = &p.pixels[sy * pw + usize::from(rx) + src..][..n];
                for (d, &c) in self.px[i..i + n].iter_mut().zip(src) {
                    if *d == c && mask & (1 << (c & 15)) != 0 {
                        *d = c | crate::light::GLOW;
                    }
                }
            }
        }
    }
}

/// Draws the slice's night dressing over a ground [`draw_ground_sky`] drew with the same camera: the lamps on the back wall
/// and the lanterns over the doors with their glass and flames glowing, and the glow marker on the door badges and the exit's
/// opening, so those stay at full brightness when the picture is shaded. Only at night: by day none of it is drawn, and the
/// day picture is the one it always was.
pub fn draw_night(frame: &mut [u8], w: usize, h: usize, g: &Ground, cam_x: i32, cam_y: i32) {
    let mut cv = Canvas::new(frame, w, h);
    if cv.w == 0 || cv.h == 0 || !g.usable() {
        return;
    }
    let (cam_x, cam_y) = (
        cam_x.clamp(-(1 << 24), 1 << 24),
        cam_y.clamp(-(1 << 24), 1 << 24),
    );
    for &(c, row) in &LAMPS {
        let r = g.h - 1 - row;
        if g.cell(c, r) == WALLBG {
            cv.piece(&LAMP, c * T + 8 - cam_x, r * T + 8 - cam_y);
            cv.glow_piece(&LAMP, c * T + 8 - cam_x, r * T + 8 - cam_y, GLOW_LAMP);
        }
    }
    let (c_lo, c_hi, r_lo, r_hi) = cell_range(&cv, cam_x, cam_y);
    for r in r_lo.max(0)..=r_hi.min(g.h - 1) {
        for c in c_lo.max(0)..=c_hi.min(g.w - 1) {
            let t = g.cell(c, r);
            let badge = match t {
                DOOR_R if g.red_closed => &DOOR_RED_TOP,
                DOOR_B if g.blue_closed => &DOOR_BLUE_TOP,
                EXIT if g.cell(c, r + 1) != EXIT => {
                    cv.glow_piece(&EXIT_DOORWAY, c * T - cam_x, r * T - cam_y, GLOW_EXIT);
                    continue;
                }
                _ => continue,
            };
            if r > 0 && g.cell(c, r - 1) == t {
                continue;
            }
            let (x, y) = (c * T - FEET_DX - cam_x, r * T - cam_y + (STRIP - BACK));
            cv.glow_piece(badge, x, y, GLOW_BADGE);
        }
    }
    for &(c, row) in &LANTERNS {
        let r = g.h - 1 - row;
        if mass(g.cell(c, r)) {
            let (x, y) = lantern_at(c, r);
            cv.piece(&LANTERN, x - cam_x, y - cam_y);
            cv.glow_piece(&LANTERN, x - cam_x, y - cam_y, GLOW_LAMP);
        }
    }
}

/// Draws the map into the canvas through a window held in `tile` and `info`.
#[inline(never)]
fn draw_cells(
    cv: &mut Canvas,
    g: &Ground,
    tile: &mut [u8],
    info: &mut [u8],
    (cam_x, cam_y, tick, full_sky): (i32, i32, u32, bool),
) {
    let (c_lo, c_hi, r_lo, r_hi) = cell_range(cv, cam_x, cam_y);
    let win = Window::new(g, tile, info, c_lo, c_hi, r_lo, r_hi);
    phase(PHASE_BACK);
    backgrounds(cv, g, &win, (cam_x, cam_y, tick, full_sky));
    phase(PHASE_DRESS);
    dressing(cv, &win, cam_x, cam_y);
    terrain(cv, &win, cam_x, cam_y);
    phase(PHASE_STAND);
    standing(cv, &win, cam_x, cam_y);
}

#[cfg(test)]
mod tests;
