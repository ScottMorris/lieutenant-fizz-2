// The night: a 256-entry palette of brightness banks, a small integer point-light model and the shade pass over a frame.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Fizz 1 lights a level at night (its Options screen's Night row) with a darker ambient and point lights: Ben's lantern, the
//! level's lanterns, glowing hazards and shots. Its Web build shades each sprite pixel in a fragment shader
//! (`packages/engine/src/renderer.ts`); its DOS port (fizz-dos, `m2/dos-rt/src/video.rs`) does the same in a 256-colour mode with
//! a palette of brightness levels and a pass over the frame. This module is that DOS technique, ported to Fizz 2's indexed
//! picture and used by both presenters, so the Web and DOS show the same pixels. Nothing here is part of the rules: the lights
//! are view-side, the state hash never sees them, and by day none of this runs.
//!
//! # The palette
//!
//! A frame byte is `bank * 16 + colour`: the low nibble is the EGA colour the art chose, the high nibble the light on it. A
//! level `L` from 1 (black) to 14 (full) shows `(L - 1) / 13` of the colour, rounded to the nearest 6-bit DAC step, as in
//! fizz-dos.
//!
//! | Bank | Bytes | Level |
//! | --- | --- | --- |
//! | 0 | 0 to 15 | the picture as drawn: full (14) by day, the level's night ambient at night ([`palette`]'s `base`) |
//! | 1 | 16 to 31 | 5, the menus' and the HUD box's dim (`DIM_BASE` in `fizz-present-dos`) |
//! | 2 to 15 | 32 to 255 | 14 down to 1: bank `16 - L` |
//!
//! Bank 0 keeps every byte that was drawn before this module the same (each day frame and every pinned digest stand), and bank
//! 1 keeps the HUD's and the menus' dimmed copy where it was. Fizz 1's DOS port puts a full-brightness marker at level 0 and
//! the user interface at level 14; here the picture's own bank 0 is the one whose meaning changes between day and night, so
//! the lights touch only the cells they reach: a cell at the ambient is never written, because the palette already shows
//! it at that level ([`shade`]).
//!
//! # Markers
//!
//! The art drawn at night carries its marker in the bank it is drawn in, as fizz-dos's sprites carry theirs in the high
//! nibble: [`LIT`] (bank 0) takes its cell's light; [`ACTOR`] (bank 7, which is level 9) never goes below level 9, so Ben and the
//! chicken stay readable (Fizz 1's shader floors actors at 0.62 of full, and 8/13 is 0.62); [`GLOW`] (bank 2, level 14) is
//! final, for keys, pickups, shots, the door badges, the exit's glow, the lamps' glass and the captions. Because each marker
//! is a real bank, a cell the shade pass skips already shows the right levels.
//!
//! # Light
//!
//! A light is a point at a height `z` over the picture's plane, with a radius and a strength. What it adds to a cell is Fizz 1's
//! shader term for a sprite facing the screen, `strength * z / |d| * (1 - |d| / r)^2` with `d` from the light to the cell's
//! centre including the `z` lift, in fizz-dos's integer form: positions in quarter pixels, and `(1 - u)^2 / u` from the baked
//! [`FALLOFF`] table over `|d|^2 / r^2`, so a cell costs a few integer steps and no square root. A cell of 8 by 8 pixels gets
//! one light value; a 2 by 2 ordered dither between quarter levels picks each pixel's level, so a gradient has four steps
//! between two levels.

use crate::oblique_data::FALLOFF;

/// Brightness levels: 1 is black, 14 full.
pub const LEVELS: u8 = 14;
/// The level the menus and the HUD box dim to (bank 1): fizz-dos's HUD lowers by nine of fourteen, leaving level 5.
pub const DIM_LEVEL: u8 = 5;
/// The lowest level an actor shows at: about 0.62 of full, Fizz 1's shader floor for characters (`vAct`).
pub const ACTOR_FLOOR: u8 = 9;

/// The bank byte (high nibble set, low nibble clear) of a lit pixel: the picture as drawn.
pub const LIT: u8 = 0x00;
/// The bank byte of the dim bank, level 5.
pub const DIM: u8 = 0x10;
/// The bank byte of a glowing pixel: level 14, final.
pub const GLOW: u8 = bank(LEVELS);
/// The bank byte of an actor's pixel: level 9, raised with its cell's light and never lowered.
pub const ACTOR: u8 = bank(ACTOR_FLOOR);

/// The bank byte that shows level `level` (clamped to 1 to 14): bank `16 - level`, never 0 or 1.
pub const fn bank(level: u8) -> u8 {
    let l = if level < 1 {
        1
    } else if level > LEVELS {
        LEVELS
    } else {
        level
    };
    (16 - l) << 4
}

/// The level byte `p` shows when bank 0 shows level `base`.
pub const fn level_of(p: u8, base: u8) -> u8 {
    match p >> 4 {
        0 => base,
        1 => DIM_LEVEL,
        b => 16 - b,
    }
}

/// The sixteen EGA colours in standard order as 6-bit DAC values (0x55 is 21, 0xAA is 42, 0xFF is 63).
pub const EGA6: [[u8; 3]; 16] = [
    [0, 0, 0],
    [0, 0, 42],
    [0, 42, 0],
    [0, 42, 42],
    [42, 0, 0],
    [42, 0, 42],
    [42, 21, 0],
    [42, 42, 42],
    [21, 21, 21],
    [21, 21, 63],
    [21, 63, 21],
    [21, 63, 63],
    [63, 21, 21],
    [63, 21, 63],
    [63, 63, 21],
    [63, 63, 63],
];

/// A 6-bit DAC value at level `level`: `(level - 1) / 13` of it, to the nearest step (`(v * (level - 1) + 6) / 13`).
pub const fn at_level(v: u8, level: u8) -> u8 {
    ((v as u16 * (level as u16 - 1) + 6) / 13) as u8
}

/// The 256-entry palette, as 6-bit DAC values, with bank 0 at level `base`: [`DAY`] for 14, a level's night for its ambient.
/// Every entry is worked out here once, in integers; DOS writes it to the DAC and the Web expands it to RGBA ([`rgba`]).
pub const fn palette(base: u8) -> [[u8; 3]; 256] {
    let mut out = [[0u8; 3]; 256];
    let mut i = 0;
    while i < 256 {
        let level = level_of(i as u8, base);
        let c = EGA6[i & 15];
        out[i] = [
            at_level(c[0], level),
            at_level(c[1], level),
            at_level(c[2], level),
        ];
        i += 1;
    }
    out
}

/// The day palette: bank 0 at full brightness.
pub const DAY: [[u8; 3]; 256] = palette(LEVELS);

/// A palette as opaque 8-bit RGBA, each 6-bit value `v` scaled to `v * 255 / 63` rounded (21 to 0x55, 42 to 0xAA, 63 to 0xFF), the
/// table the Web presenter turns a frame's bytes into canvas pixels with.
pub fn rgba(p: &[[u8; 3]; 256]) -> [[u8; 4]; 256] {
    let mut out = [[0u8; 4]; 256];
    for (o, c) in out.iter_mut().zip(p) {
        let w = |v: u8| ((u16::from(v) * 255 + 31) / 63) as u8;
        *o = [w(c[0]), w(c[1]), w(c[2]), 0xFF];
    }
    out
}

// ---------- lights ----------

/// Quarter pixels to a pixel: light positions, heights and radii are in quarter pixels, as fizz-dos's are.
pub const SUB: i32 = 4;
/// Lit cells are this many pixels square.
pub const CELL: usize = 8;
/// Full brightness in light units: a light's strength and a cell's added light are counted in these.
pub const LIGHT_ONE: i32 = 1024;
/// Quarter levels from black to full: a cell's light is `QMAX / 4` levels and a fraction.
pub const QMAX: u8 = (LEVELS - 1) * 4;
/// The most lights a picture uses: the nearest to the camera, as Fizz 1's `LightPool` writes the nearest 16 to its shader.
pub const MAX_ACTIVE: usize = 16;
/// The most lights a [`Lights`] list takes before it selects.
pub const POOL: usize = 32;

/// One point light, in map pixels times [`SUB`] with y down (the picture's own orientation): its centre, its height over the
/// picture's plane, its radius, and its strength in [`LIGHT_ONE`] units (Fizz 1's colour folded into one brightness).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Light {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub r: i32,
    pub strength: i32,
}

/// A frame's candidate lights, fixed size, no allocation: Fizz 1's `LightPool` (`crates/sim/src/lights.rs`) in integers. Lights are
/// added with their squared distance from the camera; [`Lights::select`] puts the nearest [`MAX_ACTIVE`] first, and a pinned light
/// (Ben's lantern) before all of them.
#[derive(Clone, Copy, Debug)]
pub struct Lights {
    list: [Light; POOL],
    d: [i64; POOL],
    n: usize,
}

impl Default for Lights {
    fn default() -> Self {
        Self::new()
    }
}

impl Lights {
    pub const fn new() -> Self {
        Lights {
            list: [Light {
                x: 0,
                y: 0,
                z: 0,
                r: 0,
                strength: 0,
            }; POOL],
            d: [0; POOL],
            n: 0,
        }
    }

    /// How many lights have been added.
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Adds `light`, measured from the camera's centre `cam` (in the light's units); `None` when the list is full.
    pub fn add(&mut self, cam: (i32, i32), light: Light) -> Option<usize> {
        if self.n >= POOL {
            return None;
        }
        let (dx, dy) = (
            i64::from(light.x) - i64::from(cam.0),
            i64::from(light.y) - i64::from(cam.1),
        );
        self.list[self.n] = light;
        self.d[self.n] = dx * dx + dy * dy;
        self.n += 1;
        Some(self.n - 1)
    }

    /// Makes light `i` sort before every other (Ben's lantern, Fizz 1's `pin_first`).
    pub fn pin_first(&mut self, i: usize) {
        if i < self.n {
            self.d[i] = -1;
        }
    }

    /// The lights a picture uses: the nearest [`MAX_ACTIVE`], nearest first (a selection sort, as Fizz 1's), the pinned one before them.
    pub fn select(&mut self) -> &[Light] {
        let cnt = self.n.min(MAX_ACTIVE);
        for k in 0..cnt {
            let mut mi = k;
            for j in k + 1..self.n {
                if self.d[j] < self.d[mi] {
                    mi = j;
                }
            }
            self.list.swap(k, mi);
            self.d.swap(k, mi);
        }
        &self.list[..cnt]
    }
}

/// The cell grid of a `w` by `h` view: `(columns, rows)` of [`CELL`] pixels, the last of each partly outside when the size is not
/// a multiple (40 by 25 at 320 by 200).
pub const fn grid_size(w: usize, h: usize) -> (usize, usize) {
    (w.div_ceil(CELL), h.div_ceil(CELL))
}

/// Adds the light of `lights` to each cell of a `cols` by `rows` grid, in light units (saturating at `u16::MAX`). `cam` is the
/// view's top left in the lights' units (map pixels times [`SUB`]). Each light visits only the cells its radius reaches, inside
/// the grid; a light with no radius, no height, a height at or past its radius, or no strength adds nothing.
pub fn add_lights(add: &mut [u16], cols: usize, rows: usize, lights: &[Light], cam: (i32, i32)) {
    const CELL_SUB: i32 = CELL as i32 * SUB;
    if add.len() < cols * rows || cols == 0 || rows == 0 {
        return;
    }
    for l in lights {
        let r = l.r.clamp(0, 4096);
        let z = l.z.clamp(0, 4096);
        if r == 0 || z == 0 || l.strength <= 0 || z >= r {
            continue;
        }
        // The light's place in the view, in quarter pixels from the top left.
        let sx = i64::from(l.x) - i64::from(cam.0);
        let sy = i64::from(l.y) - i64::from(cam.1);
        let reach = i64::from(r);
        let (gw, gh) = (
            cols as i64 * i64::from(CELL_SUB),
            rows as i64 * i64::from(CELL_SUB),
        );
        if sx + reach < 0 || sy + reach < 0 || sx - reach >= gw || sy - reach >= gh {
            continue;
        }
        let (sx, sy) = (sx as i32, sy as i32);
        let (r2, z2) = (r * r, z * z);
        let cx0 = (sx - r).div_euclid(CELL_SUB).max(0);
        let cx1 = (sx + r).div_euclid(CELL_SUB).min(cols as i32 - 1);
        let cy0 = (sy - r).div_euclid(CELL_SUB).max(0);
        let cy1 = (sy + r).div_euclid(CELL_SUB).min(rows as i32 - 1);
        // A cell gets `k * falloff >> 8` with `k` the strength times `z / r`; the table index is the squared distance in steps of
        // `r^2 / 1024`, and as `d^2 < r^2 <= 2^24` the product `d^2 * step` stays under 2^26.
        let k = l.strength.min(16 * LIGHT_ONE) * z / r;
        let step = ((FALLOFF.len() as u32) << 16) / r2 as u32;
        for cy in cy0..=cy1 {
            let dy = cy * CELL_SUB + CELL_SUB / 2 - sy;
            let dyz = dy * dy + z2;
            if dyz >= r2 {
                continue;
            }
            let row = &mut add[cy as usize * cols..][..cols];
            // Along the row the squared distance grows by additions: `(dx + 32)^2 = dx^2 + 64 dx + 1024`.
            let mut dx = cx0 * CELL_SUB + CELL_SUB / 2 - sx;
            let mut d2 = dx * dx + dyz;
            for cell in &mut row[cx0 as usize..=cx1 as usize] {
                if d2 < r2 {
                    let f = i32::from(FALLOFF[((d2 as u32 * step) >> 16) as usize]);
                    let v = (k * f + 128) >> 8;
                    *cell = cell.saturating_add(v.clamp(0, i32::from(u16::MAX)) as u16);
                }
                d2 += 2 * dx * CELL_SUB + CELL_SUB * CELL_SUB;
                dx += CELL_SUB;
            }
        }
    }
}

/// A cell's light in quarter levels, from the light added to it and the ambient level: `(ambient - 1) * 4` plus the added light
/// over [`LIGHT_ONE`] in quarter levels, rounded down, at most [`QMAX`].
pub const fn cell_q(add: u16, ambient: u8) -> u8 {
    let base = (ambient as u32 - 1) * 4;
    let q = base + ((add as u32 * QMAX as u32) >> 10);
    if q > QMAX as u32 { QMAX } else { q as u8 }
}

/// The least added light that makes a cell a quarter level brighter than the ambient: [`cell_q`] rounds down, so a cell with
/// less is at the ambient and is not shaded. Rounding down rather than to the nearest keeps a light's faint edge, a cell or two
/// wide all round it, out of the pass.
pub const LIT_FROM: u16 = (LIGHT_ONE as u32).div_ceil(QMAX as u32) as u16;
const _: () = assert!(cell_q(LIT_FROM - 1, 6) == 20 && cell_q(LIT_FROM, 6) == 21);

/// The 2 by 2 ordered dither, by the pixel's row and column parity: what is added to a cell's quarter level before it is
/// divided by four (fizz-dos's `BAYER`).
pub const BAYER: [[u8; 2]; 2] = [[0, 2], [3, 1]];

/// The level of a pixel at row parity `py` and column parity `px` in a cell at quarter level `q`.
pub const fn dithered(q: u8, py: usize, px: usize) -> u8 {
    1 + ((q + BAYER[py & 1][px & 1]) >> 2)
}

/// For each level, what a byte of the picture becomes in a cell lit to that level: a lit byte (bank 0) takes the level, an
/// actor's byte (bank 7) takes the level or 9, whichever is higher, and any other byte (glowing, dimmed, already shaded) stays.
const SHADE: [[u8; 256]; 15] = {
    let mut t = [[0u8; 256]; 15];
    let mut l = 1;
    while l <= LEVELS as usize {
        let mut p = 0;
        while p < 256 {
            let (hi, c) = ((p as u8) & 0xF0, (p as u8) & 15);
            t[l][p] = if hi == LIT {
                bank(l as u8) | c
            } else if hi == ACTOR {
                bank(if (l as u8) < ACTOR_FLOOR {
                    ACTOR_FLOOR
                } else {
                    l as u8
                }) | c
            } else {
                p as u8
            };
            p += 1;
        }
        l += 1;
    }
    t
};

/// What byte `p` of the picture becomes at level `level` (see [`shade`]).
pub const fn shade_byte(p: u8, level: u8) -> u8 {
    SHADE[level as usize][p as usize]
}

/// The most cells a row of the grid holds: 1280 pixels, the widest view the Web draws, is 160.
pub const MAX_COLS: usize = 160;

/// Shades a `w` by `h` frame by the cells' light: `add` holds each cell's added light ([`add_lights`]), the grid
/// [`grid_size`] gives, and `ambient` is the level bank 0 shows. A cell whose light rounds to the ambient is not touched; in
/// the others every pixel takes its dithered level through [`shade_byte`], four at a time where all four are lit. The levels of
/// a row of cells are worked out once for its eight rows of pixels, as two words for each cell (one for each row parity of the
/// dither), with the runs of cells that are lit. Pure, and the same on every platform; any size is safe, and a grid wider than
/// [`MAX_COLS`] is shaded one byte at a time.
pub fn shade(frame: &mut [u8], w: usize, h: usize, add: &[u16], ambient: u8) {
    let (cols, rows) = grid_size(w, h);
    if w == 0 || frame.len() < w * h || add.len() < cols * rows {
        return;
    }
    let base_q = (ambient - 1) * 4;
    // For each lit cell of a row, the word that sets its four pixels' banks on even rows and on odd ones (the dither's two rows).
    let mut words = [[0u32; MAX_COLS]; 2];
    // The lit cells, as runs `(first, end)`; a row has at most half its cells' worth of runs.
    let mut runs = [(0usize, 0usize); MAX_COLS / 2 + 1];
    // The cells wholly inside the frame's width.
    let whole = w / CELL;
    for cy in 0..rows {
        let cells = &add[cy * cols..][..cols];
        let y0 = cy * CELL;
        let y1 = (y0 + CELL).min(h);
        if cols > MAX_COLS {
            for y in y0..y1 {
                shade_row_slow(&mut frame[y * w..][..w], y, cells, base_q, ambient);
            }
            continue;
        }
        let mut n = 0;
        let mut open = false;
        for (cx, &a) in cells.iter().enumerate() {
            // Under a quarter level of light is the ambient (`cell_q` rounds down), so most cells end here.
            if a < LIT_FROM {
                open = false;
                continue;
            }
            let q = cell_q(a, ambient);
            if q == base_q {
                open = false;
                continue;
            }
            let word = |py: usize| {
                let (a, b) = (
                    u32::from(bank(dithered(q, py, 0))),
                    u32::from(bank(dithered(q, py, 1))),
                );
                a | b << 8 | a << 16 | b << 24
            };
            words[0][cx] = word(0);
            words[1][cx] = word(1);
            if open {
                runs[n - 1].1 = cx + 1;
            } else {
                runs[n] = (cx, cx + 1);
                n += 1;
                open = true;
            }
        }
        for y in y0..y1 {
            let row = &mut frame[y * w..][..w];
            let adds = &words[y & 1];
            for &(c0, c1) in &runs[..n] {
                let c_end = c1.min(whole);
                if c0 < c_end {
                    // A cell's row is two words: both lit (the usual case) take the cell's word in one OR each.
                    let span = row[c0 * CELL..c_end * CELL].as_chunks_mut::<4>().0;
                    for (pair, &add) in span.as_chunks_mut::<2>().0.iter_mut().zip(&adds[c0..c_end])
                    {
                        let (a, b) = (u32::from_le_bytes(pair[0]), u32::from_le_bytes(pair[1]));
                        if (a | b) & 0xF0F0_F0F0 == 0 {
                            pair[0] = (a | add).to_le_bytes();
                            pair[1] = (b | add).to_le_bytes();
                        } else {
                            shade_marked(&mut pair[..], add);
                        }
                    }
                }
                // The last cell of a frame whose width is not a multiple of 8.
                if c1 > whole {
                    let add = adds[c1 - 1];
                    let x0 = whole * CELL;
                    for (i, p) in row[x0..].iter_mut().enumerate() {
                        *p = shade_byte(*p, level_of_add(add, i));
                    }
                }
            }
        }
    }
}

/// The level byte `i & 1` of a cell's word sets (see [`shade`]).
#[inline]
const fn level_of_add(add: u32, i: usize) -> u8 {
    16 - ((add >> (4 + 8 * (i & 1) as u32)) & 15) as u8
}

/// A cell's row of eight pixels, as words, that holds a marker (an actor, a glow): a byte at a time through the table. Out of the
/// lit cells' loop, which it would slow.
#[cold]
#[inline(never)]
fn shade_marked(words: &mut [[u8; 4]], add: u32) {
    let (l0, l1) = (level_of_add(add, 0), level_of_add(add, 1));
    for word in words {
        *word = [
            shade_byte(word[0], l0),
            shade_byte(word[1], l1),
            shade_byte(word[2], l0),
            shade_byte(word[3], l1),
        ];
    }
}

/// Shades one row of pixels at frame row `y` by its row of cells, a byte at a time.
fn shade_row_slow(row: &mut [u8], y: usize, cells: &[u16], base_q: u8, ambient: u8) {
    for (cx, &a) in cells.iter().enumerate() {
        let q = cell_q(a, ambient);
        if a == 0 || q == base_q {
            continue;
        }
        let x0 = cx * CELL;
        if let Some(px) = row.get_mut(x0..(x0 + CELL).min(row.len())) {
            for (i, p) in px.iter_mut().enumerate() {
                *p = shade_byte(*p, dithered(q, y, i));
            }
        }
    }
}

/// Lowers each pixel of a shaded `span` by `by` levels, to black at most: the translucency of a shadow or fog over the finished
/// picture, as fizz-dos's `ui::dim` lowers the level nibble. `base` is the level bank 0 shows (14 by day). A pixel's colour is
/// kept; only its bank changes.
pub fn lower_span(span: &mut [u8], by: u8, base: u8) {
    for p in span {
        let l = level_of(*p, base).saturating_sub(by).max(1);
        *p = bank(l) | (*p & 15);
    }
}

#[cfg(test)]
mod tests;
