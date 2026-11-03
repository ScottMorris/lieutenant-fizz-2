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
//!
//! A caller that draws the same stack into many rectangles of one frame (the open-air runs of a level, one for each row of cells)
//! prepares it once with [`Sky::new`]: the shifts, the stars and clouds that can be in the frame, and the heights of every band
//! column, grouped where they are equal. [`Sky::draw`] then composes each row of a rectangle from the prepared lists, which is
//! the picture [`draw_layers_clipped`] gives for that rectangle and costs a few copies for a row and no hash or modulo. A stack
//! or frame too large for the fixed lists is drawn by [`draw_layers_clipped`]'s direct path instead, which gives the same picture.

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

/// The multipliers of [`hash`]: the hash is linear in its three arguments before it is mixed, so a row of cells steps by `HASH_X`.
const HASH_X: u32 = 374_761_393;
const HASH_Y: u32 = 668_265_263;
const HASH_S: u32 = 2_246_822_519;

/// The mixing step of [`hash`], from the sum of the arguments times their multipliers to 16 bits.
fn mix(mut h: u32) -> u32 {
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) & 0xffff
}

/// A 32-bit hash of a layer cell: the design prototype's `hsh`, wrapping, reduced to 16 bits.
fn hash(x: i32, y: i32, s: i32) -> u32 {
    mix((x as u32)
        .wrapping_mul(HASH_X)
        .wrapping_add((y as u32).wrapping_mul(HASH_Y))
        .wrapping_add((s as u32).wrapping_mul(HASH_S)))
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
    Sky::new(w, h, cam_x, cam_y, layers, tick).draw(frame, w, h, clip);
}

/// [`draw_layers_clipped`] with each layer drawn over the whole of the clip in turn, working out everything for the clip. It is
/// what a stack too large to prepare is drawn by, and the picture a prepared [`Sky`] must give.
#[allow(clippy::too_many_arguments)]
fn draw_layers_direct(
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

// ---------- a prepared stack ----------

/// The most layers a [`Sky`] prepares, and the room it has for what they put in the frame; a stack or frame that needs more is
/// drawn by the direct path. The slice's 320 by 200 stack needs about 110 stars, 10 clouds and 14 groups of band columns.
const MAX_LAYERS: usize = 4;
const MAX_STARS: usize = 144;
const MAX_STAR_ROWS: usize = 26;
const MAX_CLOUDS: usize = 24;
const MAX_GROUPS: usize = 48;
const MAX_RUNS: usize = 96;
/// The most cells across a row of stars, clouds or band columns that is prepared.
const MAX_CELLS_ACROSS: i32 = 64;
/// The most rows of clouds that is prepared.
const MAX_CLOUD_ROWS: i32 = 16;

/// A star as it lies in the frame: its top left, its colour and its size (1 or 2 pixels square).
#[derive(Clone, Copy, Default)]
struct Star {
    x: i16,
    y: i16,
    colour: u8,
    size: u8,
}

/// Neighbouring band columns of the same height, as the frame pixels they span and the screen row their surface tile starts on.
#[derive(Clone, Copy, Default)]
struct Group {
    x0: i32,
    x1: i32,
    top: i32,
}

/// A prepared band: its groups (a range of [`Sky::groups`]), its fill, the row from which every column is fill, and the opaque
/// runs of each of the 16 rows of its top sprite (`rows` indexes [`Sky::runs`] from `runs_at`).
#[derive(Clone, Copy)]
struct BandPrep {
    groups: (usize, usize),
    fill: u8,
    /// The first row any column's surface tile starts on, and the row from which every column is fill.
    min_top: i32,
    full_y: i32,
    /// The rows of the sprite that are 16 opaque pixels, one bit for each.
    opaque: u16,
    runs_at: usize,
    rows: [u8; 17],
}

/// The columns of a cloud that are opaque in each row of [`CLOUD`], as the first and the last; between them every other pixel is
/// (the sprite is a checker inside an ellipse).
const CLOUD_SPAN: [(u8, u8); CLOUD_H] = {
    let mut t = [(0u8, 0u8); CLOUD_H];
    let mut y = 0;
    while y < CLOUD_H {
        let (mut first, mut last) = (CLOUD_W, 0);
        let mut x = 0;
        while x < CLOUD_W {
            if CLOUD[y * CLOUD_W + x] != 255 {
                if x < first {
                    first = x;
                }
                last = x;
            }
            x += 1;
        }
        t[y] = (first as u8, last as u8);
        y += 1;
    }
    t
};

/// A stack of layers prepared for one frame: see the module documentation.
pub struct Sky<'a> {
    layers: [Layer<'a>; MAX_LAYERS],
    n: usize,
    /// Whether the stack was prepared; if not, [`Sky::draw`] draws it directly.
    ok: bool,
    w: usize,
    h: usize,
    cam: (i32, i32),
    tick: u32,
    stars: [Star; MAX_STARS],
    n_stars: usize,
    star_cell_row0: i32,
    star_shift_y: i32,
    star_rows: usize,
    /// `star_start[i]..star_start[i + 1]` are the stars of the layer's cell row `star_cell_row0 + i`.
    star_start: [u16; MAX_STAR_ROWS + 1],
    clouds: [(i16, i16); MAX_CLOUDS],
    n_clouds: usize,
    groups: [Group; MAX_GROUPS],
    n_groups: usize,
    bands: [BandPrep; 2],
    n_bands: usize,
    /// For each layer that is a band, the index of its [`BandPrep`].
    band_of: [u8; MAX_LAYERS],
    runs: [(u8, u8); MAX_RUNS],
    n_runs: usize,
}

impl<'a> Sky<'a> {
    /// Prepares `layers` for a `w` by `h` frame with its top left at (`cam_x`, `cam_y`) in map pixels at animation tick `tick`.
    /// At most one layer of stars, one of clouds and two bands are prepared, in any order; anything else, or a frame that puts
    /// more in the lists than they hold, leaves a [`Sky`] that draws directly.
    pub fn new(
        w: usize,
        h: usize,
        cam_x: i32,
        cam_y: i32,
        layers: &[Layer<'a>],
        tick: u32,
    ) -> Self {
        let mut sky = Sky {
            layers: [Layer::Stars { fx: 0, fy: 0 }; MAX_LAYERS],
            n: layers.len().min(MAX_LAYERS),
            ok: layers.len() <= MAX_LAYERS
                && (1..1 << 20).contains(&w)
                && (1..1 << 20).contains(&h),
            w,
            h,
            cam: (cam_x, cam_y),
            tick,
            stars: [Star::default(); MAX_STARS],
            n_stars: 0,
            star_cell_row0: 0,
            star_shift_y: 0,
            star_rows: 0,
            star_start: [0; MAX_STAR_ROWS + 1],
            clouds: [(0, 0); MAX_CLOUDS],
            n_clouds: 0,
            groups: [Group::default(); MAX_GROUPS],
            n_groups: 0,
            bands: [BandPrep {
                groups: (0, 0),
                fill: 0,
                min_top: i32::MAX,
                full_y: i32::MAX,
                opaque: 0,
                runs_at: 0,
                rows: [0; 17],
            }; 2],
            n_bands: 0,
            band_of: [0; MAX_LAYERS],
            runs: [(0, 0); MAX_RUNS],
            n_runs: 0,
        };
        let n = sky.n;
        sky.layers[..n].copy_from_slice(&layers[..n]);
        if !sky.ok {
            return sky;
        }
        let (fw, fh) = (w as i32, h as i32);
        // Camera values this far out show nothing but the layer's repeated pattern; clamping keeps the arithmetic in range.
        let (cam_x, cam_y) = (
            cam_x.clamp(-(1 << 24), 1 << 24),
            cam_y.clamp(-(1 << 24), 1 << 24),
        );
        let (mut seen_stars, mut seen_clouds) = (false, false);
        for i in 0..n {
            let ok = match sky.layers[i] {
                Layer::Stars { fx, fy } => {
                    let first = !seen_stars;
                    seen_stars = true;
                    first && sky.prepare_stars(fw, fh, shift(cam_x, fx), shift(cam_y, fy))
                }
                Layer::Clouds { fx, fy, rows } => {
                    let first = !seen_clouds;
                    seen_clouds = true;
                    first
                        && sky.prepare_clouds(
                            fw,
                            fh,
                            shift(cam_x, fx),
                            shift(cam_y, fy),
                            rows,
                            tick,
                        )
                }
                Layer::Band {
                    fx,
                    fy,
                    base_y,
                    heights,
                    top,
                    surface_row,
                    fill,
                } => sky.prepare_band(
                    i,
                    fw,
                    shift(cam_x, fx),
                    shift(cam_y, fy),
                    (base_y, heights, top, surface_row, fill),
                ),
            };
            if !ok {
                sky.ok = false;
                break;
            }
        }
        sky
    }

    /// The stars of the frame, by the rows of 16 pixels of layer space they lie in. A star lies in the cell row it comes from
    /// (up to 14 pixels into it, at most 2 square), so a pixel row needs the stars of one cell row only.
    fn prepare_stars(&mut self, w: i32, h: i32, sx: i32, sy: i32) -> bool {
        let (cx0, cx1) = (sx.div_euclid(16) - 1, (w + sx).div_euclid(16));
        let (cy0, cy1) = (sy.div_euclid(16) - 1, (h + sy).div_euclid(16));
        let rows = (cy1 - cy0 + 1) as usize;
        if rows > MAX_STAR_ROWS || cx1 - cx0 + 1 > MAX_CELLS_ACROSS {
            return false;
        }
        self.star_cell_row0 = cy0;
        self.star_shift_y = sy;
        self.star_rows = rows;
        for (i, cy) in (cy0..=cy1).enumerate() {
            self.star_start[i] = self.n_stars as u16;
            // `hash(cx, cy, 31)` for each cell, stepping from the first by the multiplier of `cx`.
            let mut sum = (cx0 as u32)
                .wrapping_mul(HASH_X)
                .wrapping_add((cy as u32).wrapping_mul(HASH_Y))
                .wrapping_add(31u32.wrapping_mul(HASH_S));
            for cx in cx0..=cx1 {
                let hv = mix(sum);
                sum = sum.wrapping_add(HASH_X);
                if !hv.is_multiple_of(3) {
                    continue;
                }
                if self.n_stars == MAX_STARS {
                    return false;
                }
                self.stars[self.n_stars] = Star {
                    x: (cx * 16 + ((hv >> 3) % 14) as i32 - sx) as i16,
                    y: (cy * 16 + ((hv >> 7) % 14) as i32 - sy) as i16,
                    colour: [WHITE, LIGHT_CYAN, YELLOW][((hv >> 2) % 3) as usize],
                    size: if (hv >> 9).is_multiple_of(5) { 2 } else { 1 },
                };
                self.n_stars += 1;
            }
        }
        self.star_start[rows] = self.n_stars as u16;
        true
    }

    /// The clouds that can lie in the frame, as the top left of each.
    fn prepare_clouds(&mut self, w: i32, h: i32, sx: i32, sy: i32, rows: i32, tick: u32) -> bool {
        // `tick * DRIFT_PER_75_TICKS / 75` without a 64-bit division: 75 q + r ticks drift 4 q + 4 r / 75 pixels.
        let drift =
            ((tick / 75) * DRIFT_PER_75_TICKS + (tick % 75) * DRIFT_PER_75_TICKS / 75) as i32;
        let (u0, u1) = (sx - drift, w + sx - drift);
        let (cx0, cx1) = ((u0 - CLOUD_W as i32 - 32).div_euclid(56), u1.div_euclid(56));
        if rows > MAX_CLOUD_ROWS || cx1 - cx0 + 1 > MAX_CELLS_ACROSS {
            return false;
        }
        for cy in 0..rows.max(0) {
            for cx in cx0..=cx1 {
                let hv = hash(cx, cy, 17);
                if hv % 100 >= 65 {
                    continue;
                }
                let x = cx * 56 + ((hv >> 4) % 32) as i32 + drift - sx;
                let y = cy * 32 + ((hv >> 9) % 16) as i32 - sy;
                // A cloud wholly outside the frame draws nothing.
                if x + CLOUD_W as i32 <= 0 || x >= w || y + CLOUD_H as i32 <= 0 || y >= h {
                    continue;
                }
                if self.n_clouds == MAX_CLOUDS {
                    return false;
                }
                self.clouds[self.n_clouds] = (x as i16, y as i16);
                self.n_clouds += 1;
            }
        }
        true
    }

    /// A band: the columns of the frame in groups of equal height, and the runs of its top sprite.
    fn prepare_band(
        &mut self,
        layer: usize,
        w: i32,
        sx: i32,
        sy: i32,
        (base_y, heights, top, surface_row, fill): (i32, &[i8], &[u8; 256], i32, u8),
    ) -> bool {
        if self.n_bands == 2 {
            return false;
        }
        let at = self.n_bands;
        self.n_bands += 1;
        self.band_of[layer] = at as u8;
        let first = self.n_groups;
        let mut band = BandPrep {
            groups: (first, first),
            fill,
            min_top: i32::MAX,
            full_y: i32::MAX,
            opaque: 0,
            runs_at: self.n_runs,
            rows: [0; 17],
        };
        // A band with no heights draws nothing.
        if !heights.is_empty() {
            let (ix0, ix1) = (sx.div_euclid(TILE), (w - 1 + sx).div_euclid(TILE));
            if ix1 - ix0 + 1 > MAX_CELLS_ACROSS {
                return false;
            }
            let (mut min_top, mut full_y) = (i32::MAX, i32::MIN);
            for ix in ix0..=ix1 {
                let height = i32::from(heights[ix.rem_euclid(heights.len() as i32) as usize]);
                let t = base_y - height * TILE - surface_row - sy;
                min_top = min_top.min(t);
                full_y = full_y.max(t + TILE);
                let (x0, x1) = (ix * TILE - sx, (ix + 1) * TILE - sx);
                if self.n_groups > first && self.groups[self.n_groups - 1].top == t {
                    self.groups[self.n_groups - 1].x1 = x1;
                    continue;
                }
                if self.n_groups == MAX_GROUPS {
                    return false;
                }
                self.groups[self.n_groups] = Group { x0, x1, top: t };
                self.n_groups += 1;
            }
            band.min_top = min_top;
            band.full_y = full_y;
            band.groups = (first, self.n_groups);
            // The opaque runs of each row of the 16 by 16 top sprite.
            for r in 0..16 {
                let row = &top[r * 16..r * 16 + 16];
                let mut x = 0;
                while x < 16 {
                    if row[x] == 255 {
                        x += 1;
                        continue;
                    }
                    let mut e = x;
                    while e < 16 && row[e] != 255 {
                        e += 1;
                    }
                    if self.n_runs == MAX_RUNS {
                        return false;
                    }
                    self.runs[self.n_runs] = (x as u8, (e - x) as u8);
                    self.n_runs += 1;
                    x = e;
                }
                band.rows[r + 1] = (self.n_runs - band.runs_at) as u8;
                if row.iter().all(|&p| p != 255) {
                    band.opaque |= 1 << r;
                }
            }
        }
        self.bands[at] = band;
        true
    }

    /// Draws the prepared stack into the part of the `w` by `h` frame inside `clip` (cleared to black first); nothing outside
    /// it is written. It is the picture [`draw_layers_clipped`] gives for the same arguments.
    pub fn draw(&self, frame: &mut [u8], w: usize, h: usize, clip: Clip) {
        let rows = frame.len().checked_div(w).map_or(0, |r| r.min(h));
        let (fw, fh) = (
            w.min(i32::MAX as usize) as i32,
            rows.min(i32::MAX as usize) as i32,
        );
        let clip = Clip {
            x0: clip.x0.max(0),
            y0: clip.y0.max(0),
            x1: clip.x1.min(fw),
            y1: clip.y1.min(fh),
        };
        if clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
            return;
        }
        if !self.ok || w != self.w || h < self.h {
            let (cam, layers) = (self.cam, &self.layers[..self.n]);
            draw_layers_direct(frame, w, h, cam.0, cam.1, layers, self.tick, clip);
            return;
        }
        // A band hides everything before it on the rows from which each of its columns is fill, so a layer shows only on the
        // rows above those of every band after it, and the rows below all the bands are the fill of the last of them.
        let mut y_end = [clip.y1; MAX_LAYERS];
        let mut limit = clip.y1;
        for i in (0..self.n).rev() {
            y_end[i] = limit;
            if let Layer::Band { .. } = self.layers[i] {
                let b = &self.bands[usize::from(self.band_of[i])];
                if b.groups.0 < b.groups.1 {
                    limit = limit.min(b.full_y);
                }
            }
        }
        let mut f = Rows {
            px: frame,
            w,
            x0: clip.x0 as usize,
            x1: clip.x1 as usize,
        };
        for y in clip.y0..limit.max(clip.y0) {
            f.row(y).fill(BLACK);
        }
        for (i, &y1) in y_end.iter().enumerate().take(self.n) {
            let y0 = clip.y0;
            if y1 <= y0 {
                continue;
            }
            match self.layers[i] {
                Layer::Stars { .. } => self.stars_layer(&mut f, y0, y1),
                Layer::Clouds { .. } => self.clouds_layer(&mut f, y0, y1),
                Layer::Band { top, .. } => self.band_layer(
                    &mut f,
                    &self.bands[usize::from(self.band_of[i])],
                    top,
                    y0,
                    y1,
                ),
            }
        }
    }

    /// The stars on rows `y0..y1`, each over a pixel that is still the clear colour.
    fn stars_layer(&self, f: &mut Rows, y0: i32, y1: i32) {
        let (x0, x1) = (f.x0 as i32, f.x1 as i32);
        // A star lies in the cell row it comes from, so the rows need the stars of two cell rows at most.
        let first = (y0 + self.star_shift_y).div_euclid(16) - self.star_cell_row0;
        let last = (y1 - 1 + self.star_shift_y).div_euclid(16) - self.star_cell_row0;
        for i in first.max(0)..=last.min(self.star_rows as i32 - 1) {
            let i = i as usize;
            for s in
                &self.stars[usize::from(self.star_start[i])..usize::from(self.star_start[i + 1])]
            {
                let (sx, sy, size) = (i32::from(s.x), i32::from(s.y), i32::from(s.size));
                if sx + size <= x0 || sx >= x1 || sy + size <= y0 || sy >= y1 {
                    continue;
                }
                for py in sy.max(y0)..(sy + size).min(y1) {
                    let row = f.row(py);
                    for px in sx.max(x0)..(sx + size).min(x1) {
                        let p = &mut row[(px - x0) as usize];
                        if *p == BLACK {
                            *p = s.colour;
                        }
                    }
                }
            }
        }
    }

    /// The clouds on rows `y0..y1`: the opaque pixels of each row of the sprite, which are every other one between two columns.
    fn clouds_layer(&self, f: &mut Rows, y0: i32, y1: i32) {
        let (x0, x1) = (f.x0 as i32, f.x1 as i32);
        for &(cx, cy) in &self.clouds[..self.n_clouds] {
            let (cx, cy) = (i32::from(cx), i32::from(cy));
            if cx + CLOUD_W as i32 <= x0 || cx >= x1 || cy + CLOUD_H as i32 <= y0 || cy >= y1 {
                continue;
            }
            for y in cy.max(y0)..(cy + CLOUD_H as i32).min(y1) {
                let r = (y - cy) as usize;
                let (first, last) = CLOUD_SPAN[r];
                let (first, last) = (i32::from(first), i32::from(last));
                let row = f.row(y);
                // The columns first, first + 2, ... last that lie in the row.
                let mut c = first;
                if cx + c < x0 {
                    c += (x0 - cx - c + 1) / 2 * 2;
                }
                while c <= last && cx + c < x1 {
                    row[(cx + c - x0) as usize] = CLOUD[r * CLOUD_W + c as usize];
                    c += 2;
                }
            }
        }
    }

    /// A band on rows `y0..y1`: the columns below their surface tile are fill, those on it show a row of the sprite.
    fn band_layer(&self, f: &mut Rows, band: &BandPrep, top: &[u8; 256], y0: i32, y1: i32) {
        if band.groups.0 == band.groups.1 {
            return;
        }
        let (x0, x1) = (f.x0 as i32, f.x1 as i32);
        // From `full_y` every column is fill.
        for y in band.full_y.max(y0)..y1 {
            f.row(y).fill(band.fill);
        }
        let groups = &self.groups[band.groups.0..band.groups.1];
        for y in band.min_top.max(y0)..band.full_y.min(y1) {
            let row = f.row(y);
            // The fill of neighbouring columns is one run, `(a, b)`; it is empty when a == b.
            let (mut a, mut b) = (0, 0);
            for g in groups {
                if g.x1 <= x0 {
                    continue;
                }
                if g.x0 >= x1 {
                    break;
                }
                let d = y - g.top;
                if d >= TILE {
                    if b == g.x0 && a != b {
                        b = g.x1;
                    } else {
                        fill_run(row, x0, x1, a, b, band.fill);
                        (a, b) = (g.x0, g.x1);
                    }
                    continue;
                }
                fill_run(row, x0, x1, a, b, band.fill);
                (a, b) = (0, 0);
                if d < 0 {
                    continue;
                }
                // The columns of the group on the surface tile show its row `d` of the sprite.
                let r = d as usize;
                let k0 = (x0 - g.x0).div_euclid(TILE).max(0);
                let k1 = (x1 - 1 - g.x0)
                    .div_euclid(TILE)
                    .min((g.x1 - g.x0) / TILE - 1);
                if band.opaque & (1 << r) != 0 {
                    // A row of 16 opaque pixels: each column is one move, or a copy where the clip cuts it.
                    let src: &[u8; 16] = top[r * 16..r * 16 + 16].try_into().unwrap();
                    for k in k0..=k1 {
                        let cx = g.x0 + k * TILE;
                        if cx >= x0 && cx + TILE <= x1 {
                            if let Some(d) = row[(cx - x0) as usize..].first_chunk_mut::<16>() {
                                *d = *src;
                            }
                        } else {
                            let (lo, hi) = (cx.max(x0), (cx + TILE).min(x1));
                            row[(lo - x0) as usize..(hi - x0) as usize]
                                .copy_from_slice(&src[(lo - cx) as usize..(hi - cx) as usize]);
                        }
                    }
                    continue;
                }
                let at = band.runs_at;
                let runs =
                    &self.runs[at + usize::from(band.rows[r])..at + usize::from(band.rows[r + 1])];
                for k in k0..=k1 {
                    let cx = g.x0 + k * TILE;
                    for &(rx, len) in runs {
                        let lo = (cx + i32::from(rx)).max(x0);
                        let hi = (cx + i32::from(rx) + i32::from(len)).min(x1);
                        if lo < hi {
                            let src = r * 16 + (lo - cx) as usize;
                            let n = (hi - lo) as usize;
                            row[(lo - x0) as usize..(hi - x0) as usize]
                                .copy_from_slice(&top[src..src + n]);
                        }
                    }
                }
            }
            fill_run(row, x0, x1, a, b, band.fill);
        }
    }
}

/// The rows of the clip of a frame: row `y` is the pixels from column `x0` to `x1`.
struct Rows<'f> {
    px: &'f mut [u8],
    w: usize,
    x0: usize,
    x1: usize,
}

impl Rows<'_> {
    fn row(&mut self, y: i32) -> &mut [u8] {
        let base = y as usize * self.w;
        &mut self.px[base + self.x0..base + self.x1]
    }
}

/// Fills the pixels of `row` (which starts at column `x0`) from column `a` to `b`, if they lie in the row.
fn fill_run(row: &mut [u8], x0: i32, x1: i32, a: i32, b: i32, c: u8) {
    let (a, b) = (a.max(x0), b.min(x1));
    if a < b {
        row[(a - x0) as usize..(b - x0) as usize].fill(c);
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

    /// The prepared stack and the direct path give the same picture, inside and outside the clip.
    fn same_picture(layers: &[Layer], w: usize, h: usize, cam: (i32, i32), tick: u32, clip: Clip) {
        let mut a = vec![0xEE; w * h + 8];
        let mut b = a.clone();
        let mut c = a.clone();
        Sky::new(w, h, cam.0, cam.1, layers, tick).draw(&mut a, w, h, clip);
        draw_layers_direct(&mut b, w, h, cam.0, cam.1, layers, tick, clip);
        draw_layers_clipped(&mut c, w, h, cam.0, cam.1, layers, tick, clip);
        assert!(
            a == b,
            "{w}x{h} at {cam:?}, tick {tick}, {clip:?}: {} pixels differ",
            a.iter().zip(&b).filter(|(x, y)| x != y).count()
        );
        assert!(a == c);
    }

    #[test]
    fn a_prepared_sky_is_the_picture_of_the_direct_path() {
        static HEIGHTS: [i8; 6] = [0, 2, 1, -1, 3, 1];
        let band = |fx, fy, base_y, heights: &'static [i8], surface_row, fill| Layer::Band {
            fx,
            fy,
            base_y,
            heights,
            top: &TOP,
            surface_row,
            fill,
        };
        let stars = Layer::Stars { fx: 10, fy: 10 };
        let clouds = Layer::Clouds {
            fx: 15,
            fy: 15,
            rows: 3,
        };
        let (b1, b2) = (
            band(31, 15, 100, &HEIGHTS, 4, 8),
            band(46, 15, 120, &[0], 0, 1),
        );
        let stacks: [&[Layer]; 8] = [
            &[stars, clouds, b1, b2],
            &[b2, b1, clouds, stars],
            &[stars, b1, clouds, b2],
            &[stars],
            &[clouds],
            &[b1],
            &[b1, band(5, 5, 60, &[], 0, 3), stars],
            &[stars, stars, b1],
        ];
        let mut seed = 99u32;
        let mut next = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 8
        };
        for layers in stacks {
            for (w, h) in [
                (320usize, 200usize),
                (96, 64),
                (1, 1),
                (7, 5),
                (700, 90),
                (200, 700),
            ] {
                for _ in 0..12 {
                    let cam = (next() as i32 % 2000 - 700, next() as i32 % 1500 - 400);
                    let tick = [0, 74, 75, 6000, u32::MAX][next() as usize % 5];
                    let (cx, cy) = (
                        next() as i32 % (w as i32 + 40) - 20,
                        next() as i32 % (h as i32 + 40) - 20,
                    );
                    let clip = Clip {
                        x0: cx,
                        y0: cy,
                        x1: cx + (next() % 200) as i32,
                        y1: cy + (next() % 40) as i32 + 1,
                    };
                    same_picture(layers, w, h, cam, tick, clip);
                    same_picture(
                        layers,
                        w,
                        h,
                        cam,
                        tick,
                        Clip {
                            x0: 0,
                            y0: 0,
                            x1: w as i32,
                            y1: h as i32,
                        },
                    );
                }
            }
        }
        for cam in [(i32::MIN, i32::MAX), (i32::MAX, i32::MIN), (0, 0)] {
            same_picture(
                &[stars, clouds, b1, b2],
                320,
                200,
                cam,
                1,
                Clip {
                    x0: 10,
                    y0: 10,
                    x1: 300,
                    y1: 190,
                },
            );
        }
    }

    #[test]
    fn a_stack_too_large_to_prepare_is_drawn_directly() {
        // Five layers, and a frame wider than the lists hold stars for, give the direct path's picture.
        let layers = [Layer::Stars { fx: 10, fy: 10 }; 5];
        same_picture(
            &layers,
            320,
            200,
            (5, 5),
            9,
            Clip {
                x0: 0,
                y0: 0,
                x1: 320,
                y1: 200,
            },
        );
        let wide = [Layer::Stars { fx: 10, fy: 10 }];
        let sky = Sky::new(1600, 600, 0, 0, &wide, 0);
        assert!(
            !sky.ok,
            "a frame of 1600 by 600 holds more stars than the list"
        );
        same_picture(
            &wide,
            1600,
            600,
            (40, 90),
            0,
            Clip {
                x0: 100,
                y0: 100,
                x1: 900,
                y1: 400,
            },
        );
    }
}
