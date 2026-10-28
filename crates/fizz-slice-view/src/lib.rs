// Draws the Episode 2 slice into a palette-indexed frame; the Web and DOS presenters both call it so they never differ.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![forbid(unsafe_code)]

//! One routine, [`draw_slice`], turns a [`World`] into a 320 by 200 picture of palette indices (the sixteen EGA colours,
//! in the standard order, as `fizz-view`'s sprite data uses). The Web presenter maps the indices through the same palette
//! onto its canvas and DOS writes them into VGA mode 13h, so the two screens show the same pixels.
//!
//! The picture is a pure function of the world and a `tick` that only drives animation: the rules never see it, and
//! nothing here changes the world. Text (the HUD) is not drawn here; each presenter draws its own over the result.
//!
//! # What is drawn, back to front
//!
//! 1. The sky fills the frame.
//! 2. The tile map window, as flat colours per tile type: interior wall, solid fill, block, one-way platform, spikes, the
//!    red door, the exit. The tiles are told apart by colour only; the art is a later milestone.
//! 3. The red key, if it still lies in the world, bobbing one pixel.
//! 4. The red door's sprite on each closed door tile. An open door shows the interior wall behind it.
//! 5. The chicken (the Cluckzarg placeholder art).
//! 6. The Fizz Blaster shots (the `bubble` sprite).
//! 7. Ben (Fizz 1's sprites).
//! 8. The chicken's cluck caption, if [`DrawOptions::captions`] is on.
//!
//! # Choices that are ours
//!
//! - **Chicken.** The art faces right, so a chicken walking left is flipped. It walks with both legs stepping (`walk`,
//!   `idle`, `walk2`, `idle` as the distance it has travelled grows, four pixels to a step) and shows the `alert` frame while it touches Ben. A stunned chicken is
//!   shown as the `alert` frame sunk two pixels, so it reads as knocked flat, and it flashes (disappears every other four
//!   ticks) for its last second of stun to warn that it is about to wake.
//! - **Cluck beak.** While a chicken's `cluck_age` (ticks since its last cluck, kept by the rules and not hashed) is inside
//!   the open pattern for its variant, the `cluckzarg_beak` overlay is drawn over its idle, walk or walk2 body, so the beak
//!   moves mid-stride. The `alert` frame already has an open mouth and takes no overlay. See [`BEAK_OPEN`].
//! - **Cluck caption.** For [`CAPTION_TICKS`] ticks after a cluck, the variant's word floats above the chicken's head,
//!   centred on it, in whole-pixel steps up to [`CAPTION_RISE`] pixels, with a one-pixel black shadow. Its colour steps
//!   from bright to dark halfway through and blinks out over the last quarter. Fizz 1's in-world sound captions (`cap` in
//!   `episodes/episode-1/game/src/world.rs`, a table of text and colour; 950 ms, 24 pixels of rise, linear fade, a 1-pixel
//!   shadow) are the model; the slice is 320 pixels wide, so the rise is halved, and the 8 by 8 font has capital shapes
//!   only, so the lower-case words in [`CAPTIONS`] draw as capitals. A caption follows its chicken's
//!   position exactly and is cut off by the frame's edge as the chicken leaves it, never moved inside; it is dropped once the
//!   chicken is more than one tile ([`CAPTION_REACH`]) outside the frame. The captions option gates only the text; the beak always moves.
//! - **Ben.** Ben's pose follows the choice in Fizz 1's `design/melting/engine.js` (the `const sp` in the player draw): dead
//!   is `jump`; on the pogo stick it is the squashed `pogo2` during the squash after a bounce and `pogo` otherwise; then
//!   `shoot` while the rules' shooting pose runs (`aimUp` or `aimDown` when the shot went straight up or down, as Fizz 1's
//!   `render.rs` chooses), `jump` in the air, a run frame while his horizontal speed is above 0.5 tiles a second, `lookUp`
//!   or `lookDown` while a look timer runs (Fizz 1 shows them after the run frames), and `stand` at rest. Fizz 1 keeps a distance counter `anim`, adds `|vx| * dt` to it every step, and shows
//!   `run2` when `floor(anim * 1.4)` is odd and `run1` otherwise, so the frame changes every 1/1.4 tiles run and he never
//!   skates. The rules here keep no such counter and drawing may not add one, so the view uses Ben's x in its place: `run2`
//!   when `floor(|x| * 1.4)` is odd. The two agree in cadence, and differ only in phase after Ben turns round. The sprites face right, so facing left flips them. While Ben is
//!   invulnerable he blinks.
//! - **Dead Ben.** Fizz 1 has no dead pose. A dead Ben is drawn with the `jump` pose, arms up, flipped away from his
//!   facing, and he flickers, so the toss and fall after a death read as a hurt tumble. The world respawns him
//!   afterwards.
//!
//! # The camera
//!
//! `fizz_view::follow` centres the view on the middle of Ben's body on both axes and clamps it to the map, in whole
//! logical pixels, so the map scrolls without sub-pixel shimmer. While Ben looks up or down the rules' `look_offset` (up to
//! 3.6 tiles, ramped and returned by the rules) is added to that centre before the clamp, so the view never leaves the map. The map's y-axis is flipped with `fizz_view::screen_y`.

use fizz_ep2::World;
use fizz_ep2::tiles::{BLOCK, CHOC, DOOR_R, DOOR_RED_CHANNEL, EXIT, FILL, PLAT, SPIKE, WALLBG};
use fizz_view::sprites_data::sprite;
use fizz_view::{
    TILE_PX, blit_sprite, draw_text, draw_tilemap_on_sky, follow, screen_y, text_width, tile_px,
};

/// The width of the picture in pixels.
pub const VIEW_W: usize = 320;
/// The height of the picture in pixels.
pub const VIEW_H: usize = 200;

// EGA palette indices (the standard sixteen-colour order).
const BLUE: u8 = 1;
const CYAN: u8 = 3;
const RED: u8 = 4;
const BROWN: u8 = 6;
const LIGHT_GREY: u8 = 7;
const DARK_GREY: u8 = 8;
const LIGHT_BLUE: u8 = 9;
const LIGHT_GREEN: u8 = 10;
const LIGHT_CYAN: u8 = 11;
const LIGHT_RED: u8 = 12;
const YELLOW: u8 = 14;
const WHITE: u8 = 15;

/// Closed doors in view that are remembered from the tile pass before drawing falls back to a second search for them.
const DOORS: usize = 16;

/// The sky behind and above the level.
const SKY: u8 = LIGHT_BLUE;

/// The flat colour of a tile, or `None` for tiles that show the sky.
const fn tile_colour(tile: u8) -> Option<u8> {
    match tile {
        WALLBG => Some(BLUE),
        FILL => Some(DARK_GREY),
        BLOCK => Some(LIGHT_GREY),
        PLAT => Some(LIGHT_GREEN),
        SPIKE => Some(LIGHT_RED),
        CHOC => Some(BROWN),
        EXIT => Some(YELLOW),
        _ => None,
    }
}

/// Marks a tile type with no flat colour in [`TILE_COLOURS`] (the doors, which take their colour from the switch, and the tiles
/// that show the sky).
const NO_COLOUR: u8 = u8::MAX;

/// `tile_colour` for every tile type, so the tile pass reads a byte in place of working through a `match` for each tile.
static TILE_COLOURS: [u8; 256] = {
    let mut table = [NO_COLOUR; 256];
    let mut tile = 0;
    while tile < 256 {
        if let Some(colour) = tile_colour(tile as u8) {
            table[tile] = colour;
        }
        tile += 1;
    }
    table
};

/// How a picture is drawn, beyond the world.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrawOptions {
    /// Draw the in-world captions that float above a chicken after it clucks. Off hides only the text; the beak still
    /// moves.
    pub captions: bool,
}

impl Default for DrawOptions {
    /// Captions on.
    fn default() -> Self {
        DrawOptions { captions: true }
    }
}

/// Ticks per second of the rules; the beak and caption times below are tick counts at this rate.
const TICKS_PER_SECOND: u32 = 60;

/// When the beak is open, per cluck variant: ranges of `cluck_age` in ticks, start included and end excluded.
///
/// A single cluck (variants 0 and 1) opens the beak for 12 ticks (0.2 s). A double cluck (variants 2 and 3, the two
/// `bwuk`s) opens it for 8 ticks, closes it for 4 and opens it again for 8, so 20 ticks (about a third of a second) from
/// first to last.
pub const BEAK_OPEN: [&[(u32, u32)]; 4] = [
    &[(0, 12)],
    &[(0, 12)],
    &[(0, 8), (12, 20)],
    &[(0, 8), (12, 20)],
];

/// How long a caption lives: 57 ticks, which is Fizz 1's 950 ms at 60 ticks a second.
pub const CAPTION_TICKS: u32 = TICKS_PER_SECOND * 950 / 1000;
/// How far a caption rises over its life, in pixels: half of Fizz 1's 24, because this picture is 320 pixels wide.
pub const CAPTION_RISE: i32 = 12;

/// The caption per cluck variant: its text, and its bright and dark colours (EGA indices). The text is lower-case, as in
/// Fizz 1, but the 8 by 8 font has one set of letter shapes, so it draws as capitals. The longest, `bwuk, bwuk`, is 80
/// pixels wide.
pub const CAPTIONS: [(&str, u8, u8); 4] = [
    ("buh-GAWK!", YELLOW, BROWN),
    ("bwuk", WHITE, LIGHT_GREY),
    ("bwuk-BWUK", LIGHT_RED, RED),
    ("bwuk, bwuk", LIGHT_CYAN, CYAN),
];

/// Whether the beak is open `age` ticks after a cluck of `variant`.
pub fn beak_open(variant: u8, age: u32) -> bool {
    BEAK_OPEN
        .get(usize::from(variant))
        .is_some_and(|ranges| ranges.iter().any(|&(from, to)| (from..to).contains(&age)))
}

/// Draws `world` into `frame`, a `w` by `h` buffer of palette indices (use [`VIEW_W`] by [`VIEW_H`]), with captions on.
///
/// `tick` drives animation only (the key's bob, blinking and flashing) and must not be fed back into the rules. Any frame
/// size is safe: the view is `w` by `h` and everything is clipped. This is [`draw_slice_with`] with the default options.
pub fn draw_slice(frame: &mut [u8], w: usize, h: usize, world: &World, tick: u32) {
    draw_slice_with(frame, w, h, world, tick, &DrawOptions::default());
}

/// Draws `world` as [`draw_slice`] does, with `options`.
pub fn draw_slice_with(
    frame: &mut [u8],
    w: usize,
    h: usize,
    world: &World,
    tick: u32,
    options: &DrawOptions,
) {
    let map = &world.map;
    let (map_w, map_h) = (map.w, map.h);

    // The camera centres on the middle of Ben's body, raised or lowered by the look offset the rules keep (zero unless Ben is
    // looking up or down, or the look is returning). `follow` clamps the result to the map exactly as it does for Ben alone.
    let (cam_x, cam_y) = camera(world, w, h);

    let door_closed = map.switch(DOOR_RED_CHANNEL);
    // The tile pass meets every visible tile, so it notes the closed doors for the sprites drawn after it; a view with more
    // than `DOORS` of them falls back to looking for them again.
    let mut doors = [(0, 0); DOORS];
    let mut door_count = 0;
    // The sky and the tiles are drawn as one layer, so no pixel of the view is written twice.
    draw_tilemap_on_sky(frame, w, h, cam_x, cam_y, map_w, map_h, SKY, |tx, ty| {
        // The pass only asks about tiles inside the map, so the index is valid for a map whose data holds `w * h` tiles.
        let t = map
            .data
            .get((ty * map_w + tx) as usize)
            .copied()
            .unwrap_or(0);
        let colour = TILE_COLOURS[usize::from(t)];
        if colour != NO_COLOUR {
            return Some(colour);
        }
        if t == DOOR_R {
            if door_closed && door_count < DOORS {
                doors[door_count] = (tx, ty);
            }
            door_count += usize::from(door_closed);
            return Some(if door_closed { BROWN } else { BLUE });
        }
        None
    });

    // Sprites are placed by their anchor, which is feet and centre. `at` turns a world position into frame pixels.
    let at = |x: f64, y: f64| (tile_px(x) - cam_x, screen_y(y, map_h) - cam_y);
    let phase = tick as usize;

    if door_closed && let Some(door) = sprite("doorRed") {
        if door_count <= DOORS {
            // The door art is one tile, so only the tiles the pass visited can show it.
            for &(tx, ty) in &doors[..door_count] {
                let (sx, sy) = at(f64::from(tx) + 0.5, f64::from(ty));
                blit_sprite(frame, w, h, door, sx, sy, false);
            }
        } else {
            let col0 = cam_x.div_euclid(TILE_PX).max(0);
            let col1 = (cam_x + w as i32).div_euclid(TILE_PX).min(map_w - 1);
            let row0 = cam_y.div_euclid(TILE_PX).max(0);
            let row1 = (cam_y + h as i32).div_euclid(TILE_PX).min(map_h - 1);
            for row in row0..=row1 {
                let ty = map_h - 1 - row;
                for tx in col0..=col1 {
                    if map.get(tx, ty) == DOOR_R {
                        let (sx, sy) = at(f64::from(tx) + 0.5, f64::from(ty));
                        blit_sprite(frame, w, h, door, sx, sy, false);
                    }
                }
            }
        }
    }

    if let (Some((kx, ky)), Some(key)) = (world.key_position(), sprite("keyRed")) {
        // The sprite's anchor is its feet; the key's centre is half a sprite above them.
        let (sx, sy) = at(kx, ky);
        let bob = i32::from(phase / 16 % 2 == 1);
        blit_sprite(frame, w, h, key, sx, sy + 8 - bob, false);
    }

    for c in &world.chickens {
        draw_chicken(frame, w, h, c, phase, &at);
    }

    if let Some(bubble) = sprite("bubble") {
        for s in &world.shots {
            let (sx, sy) = at(s.x, s.y);
            blit_sprite(frame, w, h, bubble, sx, sy + 4, false);
        }
    }

    draw_ben(frame, w, h, world, phase, &at);

    if options.captions {
        for c in &world.chickens {
            draw_caption(frame, w, h, c, &at);
        }
    }
}

/// The top left of the view in map pixels: Ben's centre plus the look offset, followed and clamped by `fizz_view::follow`.
fn camera(world: &World, w: usize, h: usize) -> (i32, i32) {
    let (map_w, map_h) = (world.map.w, world.map.h);
    let ben = &world.ben;
    let target_x = tile_px(ben.b.x + ben.b.w * 0.5);
    let target_y = screen_y(ben.b.y + ben.b.h * 0.5 + ben.look_offset, map_h);
    (
        follow(target_x, map_w.saturating_mul(TILE_PX), w as i32),
        follow(target_y, map_h.saturating_mul(TILE_PX), h as i32),
    )
}

type At<'a> = &'a dyn Fn(f64, f64) -> (i32, i32);

fn draw_chicken(frame: &mut [u8], w: usize, h: usize, c: &fizz_ep2::Chicken, phase: usize, at: At) {
    let stunned = c.stun > 0.0;
    // A stunned chicken flashes for its last second.
    if stunned && c.stun < 1.0 && phase / 4 % 2 == 1 {
        return;
    }
    let (name, sink) = if stunned {
        ("cluckzarg_alert", 2)
    } else if c.touch {
        ("cluckzarg_alert", 0)
    } else if c.b.on_ground {
        // Four steps of four pixels: one leg forward, legs together, the other leg forward, legs together.
        match (tile_px(c.b.x) / 4) % 4 {
            0 => ("cluckzarg_walk", 0),
            2 => ("cluckzarg_walk2", 0),
            _ => ("cluckzarg_idle", 0),
        }
    } else {
        ("cluckzarg_idle", 0)
    };
    let Some(s) = sprite(name) else {
        return;
    };
    let (sx, sy) = at(c.b.x + c.b.w * 0.5, c.b.y);
    let flip = c.dir < 0.0;
    blit_sprite(frame, w, h, s, sx, sy + sink, flip);
    // The overlay shares the body's anchor and flip, so it lands on the beak whichever way the chicken faces and whichever
    // body frame shows. The alert frame has its own open mouth.
    if name != "cluckzarg_alert"
        && beak_open(c.cluck_variant, c.cluck_age)
        && let Some(beak) = sprite("cluckzarg_beak")
    {
        blit_sprite(frame, w, h, beak, sx, sy + sink, flip);
    }
}

/// How far above the chicken's feet the top of a new caption's glyph cell sits: the body's 24 rows, the 8-row cell and two
/// pixels of air.
pub const CAPTION_ABOVE_FEET: i32 = 34;

/// How far outside the frame, in pixels, a chicken's feet may be and still have a caption: one tile. Fizz 1 drops a caption
/// whose source is more than 50 CSS pixels outside the viewport, about 5% of its roughly 1000 by 700 pixel viewport and so
/// under one tile; 16 pixels is the same proportion of this 320 by 200 frame.
pub const CAPTION_REACH: i32 = 16;

/// Where a chicken's live caption stands, before a presenter decides how to draw it. Both presenters take it from
/// [`caption_view`], so they agree on which captions exist, for how long and for which chicken.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptionView {
    /// The variant's word, lower-case as in Fizz 1.
    pub text: &'static str,
    /// The bright and dark colours of the word (EGA indices).
    pub bright: u8,
    pub dark: u8,
    /// Ticks since the cluck, from 0 up to but not including [`CAPTION_TICKS`].
    pub age: u32,
    /// The chicken's feet in frame pixels, which may lie outside the frame by up to [`CAPTION_REACH`]; presenters draw it there unclamped and let the edge clip it.
    pub x: i32,
    pub y: i32,
}

/// The caption of chicken `c` in a `w` by `h` frame drawn from `world`, or `None` when it has none: no cluck yet, the
/// caption's life is over, or the chicken's feet are more than [`CAPTION_REACH`] outside the frame on any side. The position is the chicken's
/// true one, never clamped, so a presenter lets the frame cut a word off as its chicken leaves, as Fizz 1 does. Pure: it
/// reads the chicken and the camera only.
pub fn caption_view(
    world: &World,
    w: usize,
    h: usize,
    c: &fizz_ep2::Chicken,
) -> Option<CaptionView> {
    let (cam_x, cam_y) = camera(world, w, h);
    let map_h = world.map.h;
    let at = |x: f64, y: f64| (tile_px(x) - cam_x, screen_y(y, map_h) - cam_y);
    caption_at(c, w, h, &at)
}

fn caption_at(c: &fizz_ep2::Chicken, w: usize, h: usize, at: At) -> Option<CaptionView> {
    let age = c.cluck_age;
    if age >= CAPTION_TICKS {
        return None;
    }
    let &(text, bright, dark) = CAPTIONS.get(usize::from(c.cluck_variant))?;
    let (x, y) = at(c.b.x + c.b.w * 0.5, c.b.y);
    if x < -CAPTION_REACH
        || x > w as i32 + CAPTION_REACH
        || y < -CAPTION_REACH
        || y > h as i32 + CAPTION_REACH
    {
        return None;
    }
    Some(CaptionView {
        text,
        bright,
        dark,
        age,
        x,
        y,
    })
}

/// Draws the chicken's caption as pixel text, the downscaled form for the 320 by 200 picture: the variant's word above its
/// head, rising and fading with `cluck_age`. The Web page draws the same captions as DOM text instead.
fn draw_caption(frame: &mut [u8], w: usize, h: usize, c: &fizz_ep2::Chicken, at: At) {
    let Some(cap) = caption_at(c, w, h, at) else {
        return;
    };
    let age = cap.age;
    // Bright for the first half, dark to three quarters, then dark and blinking (on for two ticks, off for two).
    let colour = if age < CAPTION_TICKS / 2 {
        cap.bright
    } else if age < CAPTION_TICKS * 3 / 4 || (age / 2).is_multiple_of(2) {
        cap.dark
    } else {
        return;
    };
    let rise = (age * CAPTION_RISE as u32 / CAPTION_TICKS) as i32;
    let text = cap.text;
    let width = text_width(text);
    // Centred on the chicken, a little above its head, and not moved: the frame clips it per pixel.
    let left = cap.x - width / 2;
    let top = cap.y - CAPTION_ABOVE_FEET - rise;
    draw_text(frame, w, h, left + 1, top + 1, text, 0);
    draw_text(frame, w, h, left, top, text, colour);
}

/// Ben shows a run frame while his horizontal speed is above this many tiles a second (Fizz 1's `Math.abs(p.vx) > 0.5`).
const RUN_SPEED: f64 = 0.5;
/// Run frames swap this many times per tile of distance (Fizz 1's `anim * 1.4`).
const RUN_STEPS_PER_TILE: f64 = 1.4;

/// The name of Ben's sprite, in the order Fizz 1's `ben_sprite` decides: dead, the pogo, a shot's aim pose, the air, a run
/// frame, a look, then standing.
fn ben_pose(ben: &fizz_ep2::Ben) -> &'static str {
    if ben.dead > 0.0 {
        "ben_jump"
    } else if ben.pogo {
        if ben.squash > 0.0 {
            "ben_pogo2"
        } else {
            "ben_pogo"
        }
    } else if ben.shoot_t > 0.0 {
        match ben.aim {
            1 => "ben_aimUp",
            -1 => "ben_aimDown",
            _ => "ben_shoot",
        }
    } else if !ben.b.on_ground {
        "ben_jump"
    } else if ben.b.vx.abs() > RUN_SPEED {
        // Fizz 1: `floor(anim * 1.4) & 1` picks `run2`, where `anim` is the distance Ben has run. Here `anim` is Ben's x.
        if (ben.b.x.abs() * RUN_STEPS_PER_TILE) as u64 % 2 == 1 {
            "ben_run2"
        } else {
            "ben_run1"
        }
    } else if ben.look_up > 0.0 {
        "ben_lookUp"
    } else if ben.look_down > 0.0 {
        "ben_lookDown"
    } else {
        "ben_stand"
    }
}

fn draw_ben(frame: &mut [u8], w: usize, h: usize, world: &World, phase: usize, at: At) {
    let ben = &world.ben;
    let dead = ben.dead > 0.0;
    // Invulnerable Ben blinks; dead Ben flickers.
    if (ben.inv > 0.0 || dead) && phase / 3 % 2 == 1 {
        return;
    }
    let name = ben_pose(ben);
    let Some(s) = sprite(name) else {
        return;
    };
    let (sx, sy) = at(ben.b.x + ben.b.w * 0.5, ben.b.y);
    // The sprites face right; dead Ben is turned away from the way he was facing.
    let flip = if dead { ben.face > 0.0 } else { ben.face < 0.0 };
    blit_sprite(frame, w, h, s, sx, sy, flip);
}

#[cfg(test)]
mod tests {
    use super::*;
    use fizz_ep2::level;
    use fizz_ep2::tiles::FILL;
    use fizz_ep2::world::{WorldFlags, step};
    use fizz_ep2::{SLICE_SCRIPT, SLICE_TICKS};
    use fizz_sim::{Buttons, InputFrame};

    extern crate std;
    use std::vec;
    use std::vec::Vec;

    const PINNED_CLUCKS: [(u64, u64, u64); 5] = [
        (197, 0x2d57854b7aee9077, 0x72d540e334aa0693),
        (217, 0x03843befb7eff66f, 0x1b12c767752c9322),
        (853, 0x8e928974e44f33b3, 0x8e928974e44f33b3),
        (873, 0x0b38a92100a6d5cc, 0x0b38a92100a6d5cc),
        (1094, 0xfdb0c73c8b23cb88, 0xfdb0c73c8b23cb88),
    ];
    const PINNED_SYNTHETIC: [u64; 6] = [
        0x83d3e3edf0e83f97,
        0x89b053cb0ae96559,
        0x53c463dce9c1f92a,
        0x4673d86f1eba4ab5,
        0x9877aaedcd8dd3d5,
        0xf2717d4dbee9a625,
    ];

    fn fnv(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    fn picture(world: &World, tick: u32) -> Vec<u8> {
        let mut f = vec![0xEE; VIEW_W * VIEW_H];
        draw_slice(&mut f, VIEW_W, VIEW_H, world, tick);
        f
    }

    /// Plays the slice script, calling `each` with the world before the first tick and after every tick.
    fn play(mut each: impl FnMut(&World)) {
        let mut w = level::world();
        let mut prev = Buttons::NONE;
        each(&w);
        for &(frames, now) in SLICE_SCRIPT {
            for _ in 0..frames {
                step(&mut w, InputFrame::from_transition(prev, now));
                prev = now;
                each(&w);
            }
        }
    }

    #[test]
    fn every_pixel_is_an_ega_colour_for_the_whole_replay() {
        let mut f = vec![0xEE; VIEW_W * VIEW_H];
        let mut ticks = 0u64;
        play(|w| {
            f.fill(0xEE);
            draw_slice(&mut f, VIEW_W, VIEW_H, w, w.tick.0 as u32);
            assert!(f.iter().all(|&p| p <= 15), "tick {}", w.tick.0);
            ticks += 1;
        });
        assert_eq!(ticks, SLICE_TICKS + 1);
    }

    #[test]
    fn frames_at_chosen_ticks_are_pinned() {
        // These digests lock the picture, so any change to the art, the colours or the camera is a deliberate edit here.
        //
        // Tick 865 is 12 ticks after the cluck at 853. Its chicken is off screen below the frame, so no caption is drawn and the
        // digest is the old one again (it was pinned to a clamped caption once). `without_clucks` draws the world as it was drawn before, and its
        // digests are the old ones at every tick, which shows the change is the cluck's pixels and nothing else.
        let mut seen = Vec::new();
        let mut old = Vec::new();
        play(|w| {
            if [0, 150, 270, 400, 602, 700, 865, 1000, 1268].contains(&w.tick.0) {
                seen.push((w.tick.0, fnv(&picture(w, w.tick.0 as u32))));
                old.push((w.tick.0, fnv(&picture(&without_clucks(w), w.tick.0 as u32))));
            }
        });
        assert_eq!(
            old,
            [
                (0, 0x5b84b8874be44ba4),
                (150, 0x7abd20c31ae39a54),
                (270, 0x9bf387153127ec74),
                (400, 0xe88d2a8a227b4cfb),
                (602, 0xfea72a408c6a4ab1),
                (700, 0xdda9c10396315bd2),
                (865, 0x1e0d93372eb50e10),
                (1000, 0x1881c1623a3ec20c),
                (1268, 0x23afa9ef954ff4e0),
            ]
        );
        assert_eq!(
            seen,
            [
                (0, 0x5b84b8874be44ba4),
                (150, 0x7abd20c31ae39a54),
                // Ben is running on the ground at 270, 602 and 865, so these lock the run-frame rule.
                (270, 0x9bf387153127ec74),
                (400, 0xe88d2a8a227b4cfb),
                (602, 0xfea72a408c6a4ab1),
                (700, 0xdda9c10396315bd2),
                (865, 0x1e0d93372eb50e10),
                (1000, 0x1881c1623a3ec20c),
                (1268, 0x23afa9ef954ff4e0),
            ]
        );
    }

    /// The world as it looked before chickens had a cluck to show: every chicken has never clucked.
    fn without_clucks(world: &World) -> World {
        let mut w = world.clone();
        for c in &mut w.chickens {
            c.cluck_age = fizz_ep2::chicken::CLUCK_NEVER;
            c.cluck_variant = 0;
        }
        w
    }

    fn picture_with(world: &World, tick: u32, captions: bool) -> Vec<u8> {
        let mut f = vec![0xEE; VIEW_W * VIEW_H];
        draw_slice_with(
            &mut f,
            VIEW_W,
            VIEW_H,
            world,
            tick,
            &DrawOptions { captions },
        );
        f
    }

    /// The bounding box `(x0, y0, x1, y1)`, ends included, of the pixels where `a` and `b` differ.
    fn changed_box(a: &[u8], b: &[u8]) -> Option<(usize, usize, usize, usize)> {
        let mut bb: Option<(usize, usize, usize, usize)> = None;
        for (i, (p, q)) in a.iter().zip(b).enumerate() {
            if p != q {
                let (x, y) = (i % VIEW_W, i / VIEW_W);
                bb = Some(match bb {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
        bb
    }

    /// The slice's first screen with the chicken in view, `age` ticks after a cluck of `variant`, Ben out of its way.
    fn cluck_world(variant: u8, age: u32) -> World {
        let mut w = level::world();
        w.ben.b.x = 12.0;
        w.ben.b.y = 11.0;
        w.chickens[0].cluck_variant = variant;
        w.chickens[0].cluck_age = age;
        w
    }

    #[test]
    fn caption_view_is_the_shared_caption_state() {
        let view = |w: &World| caption_view(w, VIEW_W, VIEW_H, &w.chickens[0]);
        let w = cluck_world(2, 5);
        let cap = view(&w).expect("a caption");
        assert_eq!((cap.text, cap.age), ("bwuk-BWUK", 5));
        assert_eq!((cap.bright, cap.dark), (LIGHT_RED, RED));
        // Over after its life, and never for a chicken that has not clucked.
        assert!(view(&cluck_world(2, CAPTION_TICKS)).is_none());
        assert!(view(&cluck_world(2, fizz_ep2::chicken::CLUCK_NEVER)).is_none());
        // Dropped when the chicken is far off screen, as the pixel version does.
        let mut far = cluck_world(0, 3);
        far.ben.b.x = 2.0;
        far.chickens[0].b.x += 40.0;
        assert!(view(&far).is_none());
    }

    #[test]
    fn a_caption_follows_its_chicken_off_the_edge_unclamped() {
        let view = |w: &World| caption_view(w, VIEW_W, VIEW_H, &w.chickens[0]);
        let on_screen = view(&cluck_world(0, 3)).expect("a caption");
        // Move the chicken so its feet stand `x` frame pixels from the left edge (the camera follows Ben, not the chicken).
        let with_feet_at = |x: i32| {
            let mut w = cluck_world(0, 3);
            w.chickens[0].b.x += f64::from(x - on_screen.x) / 16.0;
            w
        };
        // Just off the left edge: the caption stands at the chicken's true x, so the frame cuts it off.
        let cap = view(&with_feet_at(-6)).expect("a clipped caption");
        assert_eq!((cap.x, cap.y), (-6, on_screen.y));
        // Exactly one tile out still has one; 20 pixels out has none; the same on the right.
        assert_eq!(view(&with_feet_at(-CAPTION_REACH)).map(|c| c.x), Some(-16));
        assert!(view(&with_feet_at(-20)).is_none());
        assert_eq!(
            view(&with_feet_at(VIEW_W as i32 + 16)).map(|c| c.x),
            Some(336)
        );
        assert!(view(&with_feet_at(VIEW_W as i32 + 20)).is_none());
        // The pixel word is centred on the chicken and cut by the frame: with the feet at -6 it spans -42 to 30, so its
        // right part is drawn and no pixel is moved in to keep the whole word.
        let w = with_feet_at(-6);
        let mut with = vec![0u8; VIEW_W * VIEW_H];
        draw_slice(&mut with, VIEW_W, VIEW_H, &w, 0);
        let mut bare = vec![0u8; VIEW_W * VIEW_H];
        let off = DrawOptions { captions: false };
        draw_slice_with(&mut bare, VIEW_W, VIEW_H, &w, 0, &off);
        let changed: Vec<usize> = (0..with.len())
            .filter(|&i| with[i] != bare[i])
            .map(|i| i % VIEW_W)
            .collect();
        assert!(changed.iter().any(|&x| x < 8), "{changed:?}");
        assert!(changed.iter().all(|&x| x <= 31), "{changed:?}");
    }

    /// Every world in the replay whose chicken clucked within the caption's life, with the cluck's age.
    fn cluck_moments() -> Vec<(World, u32)> {
        let mut out = Vec::new();
        play(|w| {
            let age = w.chickens[0].cluck_age;
            if age <= CAPTION_TICKS + 2 {
                out.push((w.clone(), age));
            }
        });
        out
    }

    #[test]
    fn a_cluck_changes_only_the_beak_and_the_caption() {
        let moments = cluck_moments();
        // Three clucks, each seen from its first tick to 59 ticks later, less any cut short by the death in the script.
        assert!(
            moments.len() >= 3 * (CAPTION_TICKS as usize),
            "{}",
            moments.len()
        );
        let mut beaks = 0;
        let mut captions = 0;
        for (w, age) in &moments {
            let t = w.tick.0 as u32;
            let quiet = picture_with(&without_clucks(w), t, true);
            let beak_only = picture_with(w, t, false);
            let both = picture_with(w, t, true);
            let v = w.chickens[0].cluck_variant;
            // A stunned chicken, or one touching Ben, shows the alert frame, whose mouth is already open: no overlay.
            let alert = w.chickens[0].stun > 0.0 || w.chickens[0].touch;
            let open = beak_open(v, *age) && !alert;
            // The beak is a 4 by 4 patch, and only while the pattern has it open.
            match changed_box(&quiet, &beak_only) {
                // No change means the chicken is off screen, or the beak is closed: never an open beak on screen.
                None => {}
                Some((x0, y0, x1, y1)) => {
                    assert!(open, "tick {t}: beak drawn closed");
                    assert!(
                        x1 - x0 < 4 && y1 - y0 < 4,
                        "tick {t}: beak box {x0},{y0}..{x1},{y1}"
                    );
                    beaks += 1;
                }
            }
            // The caption is a box no bigger than its text and shadow, and only for its life.
            match changed_box(&beak_only, &both) {
                None => {}
                Some((x0, y0, x1, y1)) => {
                    assert!(*age < CAPTION_TICKS, "tick {t}: caption after its life");
                    let max_w = fizz_view::text_width(CAPTIONS[usize::from(v)].0) as usize + 1;
                    assert!(
                        x1 - x0 < max_w && y1 - y0 < 9,
                        "tick {t}: caption box {x0},{y0}..{x1},{y1}"
                    );
                    captions += 1;
                }
            }
            // After the beak and caption are done the picture is the old one.
            if *age >= CAPTION_TICKS {
                assert_eq!(quiet, both, "tick {t}");
            }
        }
        assert!(
            beaks >= 10 && captions >= 40,
            "{beaks} beak frames, {captions} caption frames"
        );
    }

    /// A tall arena with a floor two tiles thick under Ben, so the camera has room above and below him.
    fn tall_world() -> World {
        let mut map = level::world().map;
        for y in 0..map.h {
            for x in 0..map.w {
                map.set(x, y, if y == 23 || y == 24 { FILL } else { 0 });
            }
        }
        World::new(map, (16.0, 25.0))
    }

    /// Steps `world` with `held` for `ticks` ticks, continuing from `prev`.
    fn hold(world: &mut World, prev: &mut Buttons, held: Buttons, ticks: u32) {
        for _ in 0..ticks {
            step(world, InputFrame::from_transition(*prev, held));
            *prev = held;
        }
    }

    #[test]
    fn the_pose_follows_fizz_1s_precedence() {
        let mut ben = fizz_ep2::Ben::new(0.0, 0.0);
        ben.b.on_ground = true;
        assert_eq!(ben_pose(&ben), "ben_stand");
        ben.look_down = 0.1;
        assert_eq!(ben_pose(&ben), "ben_lookDown");
        ben.look_up = 0.1;
        assert_eq!(ben_pose(&ben), "ben_lookUp", "Up wins if both somehow run");
        ben.b.vx = 1.0;
        assert_eq!(ben_pose(&ben), "ben_run1", "running beats looking");
        ben.b.on_ground = false;
        assert_eq!(ben_pose(&ben), "ben_jump", "the air beats running");
        ben.shoot_t = 0.1;
        assert_eq!(ben_pose(&ben), "ben_shoot");
        ben.aim = 1;
        assert_eq!(ben_pose(&ben), "ben_aimUp", "a shot beats the air");
        ben.aim = -1;
        assert_eq!(ben_pose(&ben), "ben_aimDown");
        ben.pogo = true;
        assert_eq!(ben_pose(&ben), "ben_pogo", "the pogo beats a shot");
        ben.pogo = false;
        ben.dead = 0.5;
        assert_eq!(ben_pose(&ben), "ben_jump", "dead beats everything");
        ben.dead = 0.0;
        ben.shoot_t = 0.0;
        ben.b.on_ground = true;
        ben.b.vx = 0.0;
        assert_eq!(
            ben_pose(&ben),
            "ben_lookUp",
            "an expired shot leaves the look"
        );
    }

    #[test]
    fn an_open_beak_and_a_caption_show_on_screen() {
        for v in 0..4u8 {
            let open = cluck_world(v, 1);
            let t = 7;
            let quiet = picture_with(&without_clucks(&open), t, false);
            assert!(
                changed_box(&quiet, &picture_with(&open, t, false)).is_some(),
                "variant {v}: beak"
            );
            assert!(
                changed_box(
                    &picture_with(&open, t, false),
                    &picture_with(&open, t, true)
                )
                .is_some(),
                "variant {v}: caption"
            );
            // Once the beak shuts and the caption is gone the picture is the old one.
            let done = cluck_world(v, CAPTION_TICKS);
            assert_eq!(picture_with(&done, t, true), quiet, "variant {v}");
        }
        // The double cluck's beak closes between its two halves, and the single cluck's stays shut after 12 ticks.
        let quiet = picture_with(&without_clucks(&cluck_world(2, 0)), 0, false);
        assert_ne!(picture_with(&cluck_world(2, 3), 0, false), quiet);
        assert_eq!(picture_with(&cluck_world(2, 9), 0, false), quiet);
        assert_ne!(picture_with(&cluck_world(2, 13), 0, false), quiet);
        assert_eq!(picture_with(&cluck_world(1, 13), 0, false), quiet);
        // The beak shows mid-stride on all three body frames, and not on the alert frame.
        let mut w = cluck_world(0, 0);
        for x in [8.0, 8.0 + 4.0 / 16.0, 8.0 + 8.0 / 16.0, 8.0 + 12.0 / 16.0] {
            w.chickens[0].b.x = x;
            let open = picture_with(&w, 0, false);
            w.chickens[0].cluck_age = fizz_ep2::chicken::CLUCK_NEVER;
            assert_ne!(open, picture_with(&w, 0, false), "x = {x}");
            w.chickens[0].cluck_age = 0;
        }
        w.chickens[0].touch = true;
        let alert = picture_with(&w, 0, false);
        w.chickens[0].cluck_age = fizz_ep2::chicken::CLUCK_NEVER;
        assert_eq!(alert, picture_with(&w, 0, false));
    }

    #[test]
    fn the_beak_opens_once_for_a_single_cluck_and_twice_for_a_double() {
        for v in 0..4u8 {
            let opens: Vec<bool> = (0..24).map(|a| beak_open(v, a)).collect();
            let runs = opens.windows(2).filter(|p| !p[0] && p[1]).count() + usize::from(opens[0]);
            assert_eq!(runs, if v < 2 { 1 } else { 2 }, "variant {v}: {opens:?}");
            assert!(opens[0], "the beak opens on the tick of the cluck");
            assert!(!beak_open(v, 20) && !beak_open(v, 21) && !beak_open(v, CAPTION_TICKS));
            assert!(!beak_open(v, fizz_ep2::chicken::CLUCK_NEVER));
        }
        assert!(!beak_open(4, 0));
    }

    #[test]
    fn the_caption_rises_in_whole_pixels_and_stays_in_the_frame() {
        // Rises 12 pixels over its 57 ticks.
        let w = cluck_world(0, 0);
        let top = |age: u32| {
            let mut w = w.clone();
            w.chickens[0].cluck_age = age;
            let t = w.tick.0 as u32;
            let both = picture_with(&w, t, true);
            let beak_only = picture_with(&w, t, false);
            changed_box(&beak_only, &both).map(|b| b.1)
        };
        let (first, last) = (top(0).unwrap(), top(CAPTION_TICKS - 1).unwrap());
        assert_eq!(first - last, CAPTION_RISE as usize - 1, "{first} to {last}");
        assert!((0..CAPTION_TICKS).all(|a| top(a).is_none_or(|y| y <= first && y >= last)));
        // At the left edge, the right edge and the top of the frame the caption is cut by the frame, not moved inside.
        for (x, y) in [(2.0, 11.0), (21.0, 11.0), (8.0, 16.0)] {
            let mut w = w.clone();
            w.chickens[0].b.x = x;
            w.chickens[0].b.y = y;
            w.chickens[0].cluck_age = 0;
            let t = w.tick.0 as u32;
            let on = picture_with(&w, t, true);
            let off = picture_with(&w, t, false);
            let (x0, y0, x1, y1) =
                changed_box(&off, &on).unwrap_or_else(|| panic!("caption at {x},{y}"));
            assert!(x1 < VIEW_W && y1 < VIEW_H, "{x0},{y0}..{x1},{y1}");
        }
        // A frame too small for the text is safe.
        let mut f = vec![0u8; 20 * 12];
        draw_slice(&mut f, 20, 12, &w, 0);
    }

    #[test]
    fn captions_off_hides_only_the_text() {
        let w = cluck_world(0, 0);
        let t = w.tick.0 as u32;
        let off = picture_with(&w, t, false);
        let on = picture_with(&w, t, true);
        assert_ne!(off, on);
        assert_ne!(
            off,
            picture_with(&without_clucks(&w), t, false),
            "the beak still moves"
        );
        let mut by_default = vec![0xEE; VIEW_W * VIEW_H];
        draw_slice(&mut by_default, VIEW_W, VIEW_H, &w, t);
        assert_eq!(by_default, on, "draw_slice means captions on");
        assert!(DrawOptions::default().captions);
    }

    #[test]
    fn a_flipped_chicken_opens_its_beak_on_the_other_side() {
        let mut w = cluck_world(0, 0);
        let t = w.tick.0 as u32;
        w.chickens[0].dir = 1.0;
        let right = changed_box(
            &picture_with(&without_clucks(&w), t, false),
            &picture_with(&w, t, false),
        )
        .unwrap();
        w.chickens[0].dir = -1.0;
        let left = changed_box(
            &picture_with(&without_clucks(&w), t, false),
            &picture_with(&w, t, false),
        )
        .unwrap();
        // The chicken's centre is the same column; the beak is right of it facing right and left of it facing left.
        assert!(right.0 >= left.2, "{right:?} {left:?}");
    }

    #[test]
    fn synthetic_cluck_pictures_are_pinned() {
        // (variant, age, captions): the beak open with the caption bright, the beak shut between the two halves of a double
        // cluck, the beak shut with the caption dark and part-way up, the caption blinking, and captions off.
        let cases = [
            (0, 0, true),
            (2, 3, true),
            (2, 9, true),
            (1, 30, true),
            (3, 50, true),
            (0, 5, false),
        ];
        let seen: Vec<u64> = cases
            .iter()
            .map(|&(v, a, c)| fnv(&picture_with(&cluck_world(v, a), 7, c)))
            .collect();
        assert_eq!(seen, PINNED_SYNTHETIC);
    }

    #[test]
    fn the_cluck_pictures_are_pinned() {
        // The first tick of each cluck: variant 2 (a double bwuk) at 197, variant 1 at 853, variant 0 at 1094. The tick
        // 20 ticks later has the beak closed and the caption part-way up.
        let mut seen = Vec::new();
        play(|w| {
            let t = w.tick.0;
            if [197, 217, 853, 873, 1094].contains(&t) {
                let t = t as u32;
                seen.push((
                    w.tick.0,
                    fnv(&picture_with(w, t, true)),
                    fnv(&picture_with(w, t, false)),
                ));
            }
        });
        assert_eq!(seen, PINNED_CLUCKS);
    }

    #[test]
    fn the_look_camera_ramps_clamps_and_returns() {
        let mut w = tall_world();
        let mut prev = Buttons::NONE;
        hold(&mut w, &mut prev, Buttons::NONE, 10);
        let rest = camera(&w, VIEW_W, VIEW_H);
        let mut last = rest.1;
        for _ in 0..60 {
            hold(&mut w, &mut prev, Buttons::UP, 1);
            let y = camera(&w, VIEW_W, VIEW_H).1;
            assert!(y <= last, "the view only rises while Up is held");
            last = y;
        }
        // 3.6 tiles of 16 pixels is 57.6 pixels: the floor in `tile_px` gives 57 going up and 58 going down.
        assert_eq!(rest.1 - last, 57);
        let mut down = last;
        loop {
            hold(&mut w, &mut prev, Buttons::NONE, 1);
            let y = camera(&w, VIEW_W, VIEW_H).1;
            assert!(y >= down, "the view only comes back");
            down = y;
            if w.ben.look_offset == 0.0 {
                break;
            }
        }
        assert_eq!(camera(&w, VIEW_W, VIEW_H), rest);
        for _ in 0..60 {
            hold(&mut w, &mut prev, Buttons::DOWN, 1);
        }
        assert_eq!(camera(&w, VIEW_W, VIEW_H).1 - rest.1, 58);
    }

    #[test]
    fn the_look_never_loses_ben_or_leaves_the_map() {
        let mut w = tall_world();
        let (map_px_h, map_px_w) = (w.map.h * TILE_PX, w.map.w * TILE_PX);
        for y in [0.0, 5.0, 25.0, 40.0, 46.0] {
            for off in [-3.6, -1.0, 0.0, 1.0, 3.6] {
                w.ben.b.y = y;
                w.ben.look_offset = off;
                let (cx, cy) = camera(&w, VIEW_W, VIEW_H);
                assert!((0..=map_px_w - VIEW_W as i32).contains(&cx));
                assert!(
                    (0..=map_px_h - VIEW_H as i32).contains(&cy),
                    "y {y} off {off}"
                );
                // Ben's body, in map pixels measured from the top, is inside the view.
                let top = screen_y(w.ben.b.y + w.ben.b.h, w.map.h);
                let bottom = screen_y(w.ben.b.y, w.map.h);
                assert!(top >= cy && bottom <= cy + VIEW_H as i32, "y {y} off {off}");
            }
        }
    }

    #[test]
    fn a_narrow_and_a_wide_view_keep_ben_in_view_and_inside_the_map() {
        // The Web view is 208 tall and as wide as the window's shape asks: 96 is a portrait phone, 640 an ultra-wide screen
        // that is wider than the 32 tile (512 pixel) level, so the map is centred there and the sky shows at the sides.
        let mut w = fizz_ep2::level::world();
        let (map_px_w, map_px_h) = (w.map.w * TILE_PX, w.map.h * TILE_PX);
        for (vw, vh) in [(96usize, 208usize), (99, 208), (320, 208), (640, 208)] {
            for x in [0.0, 1.0, 8.0, 16.0, 24.0, 30.0] {
                for y in [0.0, 1.0, 12.0, 24.0, 36.0, 44.0] {
                    for off in [-3.6, 0.0, 3.6] {
                        w.ben.b.x = x;
                        w.ben.b.y = y;
                        w.ben.look_offset = off;
                        let (cx, cy) = camera(&w, vw, vh);
                        if map_px_w > vw as i32 {
                            assert!((0..=map_px_w - vw as i32).contains(&cx), "{vw}x{vh} ben {x},{y} look {off}");
                        } else {
                            assert_eq!(cx, (map_px_w - vw as i32).div_euclid(2), "{vw}x{vh} ben {x},{y} look {off}");
                        }
                        assert!((0..=map_px_h - vh as i32).contains(&cy), "{vw}x{vh} ben {x},{y} look {off}");
                        // Ben's body, in map pixels from the top and left, lies inside the view (the look offset only
                        // moves the camera as far as the map allows, and 3.6 tiles of look leaves him in a 208 view).
                        let left = tile_px(w.ben.b.x);
                        let right = tile_px(w.ben.b.x + w.ben.b.w);
                        let top = screen_y(w.ben.b.y + w.ben.b.h, w.map.h);
                        let bottom = screen_y(w.ben.b.y, w.map.h);
                        assert!(left >= cx && right <= cx + vw as i32, "{vw}x{vh} ben {x},{y} look {off}");
                        assert!(top >= cy && bottom <= cy + vh as i32, "{vw}x{vh} ben {x},{y} look {off}");
                        // Drawing at this size neither panics nor leaves the buffer unwritten.
                        let mut f = vec![0xEE; vw * vh];
                        draw_slice(&mut f, vw, vh, &w, 0);
                        assert!(f.iter().all(|&p| p != 0xEE), "{vw}x{vh} ben {x},{y} look {off}");
                    }
                }
            }
        }
    }

    #[test]
    fn scripted_looks_and_aims_are_pinned() {
        let mut w = tall_world();
        let mut prev = Buttons::NONE;
        let mut seen = Vec::new();
        hold(&mut w, &mut prev, Buttons::NONE, 10);
        seen.push(("stand", fnv(&picture(&w, 0))));
        hold(&mut w, &mut prev, Buttons::UP, 60);
        assert_eq!(ben_pose(&w.ben), "ben_lookUp");
        seen.push(("look up", fnv(&picture(&w, 0))));
        hold(&mut w, &mut prev, Buttons::NONE, 30);
        hold(&mut w, &mut prev, Buttons::DOWN, 60);
        assert_eq!(ben_pose(&w.ben), "ben_lookDown");
        seen.push(("look down", fnv(&picture(&w, 0))));
        hold(&mut w, &mut prev, Buttons::NONE, 30);
        hold(&mut w, &mut prev, Buttons::FIRE, 3);
        assert_eq!(ben_pose(&w.ben), "ben_shoot");
        seen.push(("aim level", fnv(&picture(&w, 0))));
        hold(&mut w, &mut prev, Buttons::NONE, 30);
        hold(&mut w, &mut prev, Buttons::UP.union(Buttons::FIRE), 3);
        assert_eq!(w.ben.aim, 1);
        assert_eq!(ben_pose(&w.ben), "ben_aimUp");
        seen.push(("aim up", fnv(&picture(&w, 0))));
        hold(&mut w, &mut prev, Buttons::NONE, 30);
        hold(&mut w, &mut prev, Buttons::JUMP, 1);
        hold(&mut w, &mut prev, Buttons::NONE, 6);
        hold(&mut w, &mut prev, Buttons::DOWN.union(Buttons::FIRE), 3);
        assert_eq!(w.ben.aim, -1);
        assert_eq!(ben_pose(&w.ben), "ben_aimDown");
        seen.push(("aim down", fnv(&picture(&w, 0))));
        assert_eq!(
            seen,
            [
                ("stand", 0x3685_3ba7_d340_af03),
                ("look up", 0x0814_4c64_8495_d0e4),
                ("look down", 0x6efd_b16a_7546_e7cd),
                ("aim level", 0xb78e_4f97_b6ae_b26e),
                ("aim up", 0xac57_d4e8_fdef_dbe6),
                ("aim down", 0x6c31_2cb2_b5dd_365e),
            ]
        );
    }

    #[test]
    fn drawing_is_repeatable() {
        let w = level::world();
        assert_eq!(picture(&w, 5), picture(&w, 5));
    }

    #[test]
    fn odd_frame_sizes_are_safe() {
        let w = level::world();
        for (fw, fh, len) in [
            (0, 0, 0),
            (1, 1, 1),
            (7, 5, 35),
            (320, 200, 100),
            (320, 200, 64_000),
            (640, 400, 256_000),
            (320, 0, 64_000),
            (0, 200, 64_000),
        ] {
            let mut f = vec![0u8; len];
            draw_slice(&mut f, fw, fh, &w, 3);
        }
    }

    #[test]
    fn the_key_is_drawn_until_it_is_picked_up() {
        let mut w = level::world();
        w.ben.b.x = 3.0;
        w.ben.b.y = 11.0;
        let with = picture(&w, 0);
        w.flags.set(WorldFlags::KEY_RED);
        assert_ne!(with, picture(&w, 0));
    }

    #[test]
    fn the_door_is_drawn_closed_and_then_open() {
        let mut w = level::world();
        w.ben.b.x = 13.0;
        w.ben.b.y = 20.0;
        let closed = picture(&w, 0);
        w.map.set_switch(fizz_ep2::tiles::DOOR_RED_CHANNEL, false);
        w.flags.set(WorldFlags::DOOR_RED_OPEN);
        let open = picture(&w, 0);
        let count = |f: &[u8], c: u8| f.iter().filter(|&&p| p == c).count();
        assert!(count(&closed, BROWN) > 0);
        assert_eq!(count(&open, BROWN), 0);
        assert_ne!(closed, open);
    }

    #[test]
    fn more_doors_than_the_tile_pass_remembers_are_all_drawn() {
        // 30 door tiles in view is more than `DOORS`, so the second search draws them; each shows the door art on brown.
        let mut w = level::world();
        w.ben.b.x = 13.0;
        w.ben.b.y = 20.0;
        let (bx, by) = (w.ben.b.x as i32, w.ben.b.y as i32);
        let tiles: Vec<(i32, i32)> = (4..10)
            .flat_map(|dx| (-2..3).map(move |dy| (bx + dx, by + dy)))
            .collect();
        assert!(tiles.len() > DOORS);
        for &(tx, ty) in &tiles {
            w.map.set(tx, ty, DOOR_R);
        }
        let f = picture(&w, 0);
        let map_h = w.map.h;
        let target_x = tile_px(w.ben.b.x + w.ben.b.w * 0.5);
        let target_y = screen_y(w.ben.b.y + w.ben.b.h * 0.5, map_h);
        let cam_x = follow(target_x, w.map.w * TILE_PX, VIEW_W as i32);
        let cam_y = follow(target_y, map_h * TILE_PX, VIEW_H as i32);
        let door = sprite("doorRed").unwrap();
        for &(tx, ty) in &tiles {
            let (left, top) = (tx * TILE_PX - cam_x, (map_h - 1 - ty) * TILE_PX - cam_y);
            assert!(
                left >= 0
                    && left + TILE_PX <= VIEW_W as i32
                    && top >= 0
                    && top + TILE_PX <= VIEW_H as i32
            );
            for sy in 0..16usize {
                for sx in 0..16usize {
                    let p = door.pixels[sy * 16 + sx];
                    let want = if p == fizz_view::TRANSPARENT {
                        BROWN
                    } else {
                        p
                    };
                    let got = f[(top as usize + sy) * VIEW_W + left as usize + sx];
                    assert_eq!(got, want, "door ({tx}, {ty}) pixel ({sx}, {sy})");
                }
            }
        }
    }

    #[test]
    fn the_tile_types_have_distinct_colours() {
        let colours: Vec<u8> = [WALLBG, FILL, BLOCK, PLAT, SPIKE, EXIT]
            .iter()
            .map(|&t| tile_colour(t).unwrap())
            .collect();
        for (i, a) in colours.iter().enumerate() {
            assert!(colours[i + 1..].iter().all(|b| a != b));
            assert_ne!(*a, SKY);
        }
        assert_eq!(tile_colour(0), None);
    }

    #[test]
    fn a_walking_chicken_steps_with_both_legs() {
        let rows = |name: &str| {
            let s = sprite(name).unwrap();
            assert_eq!((s.w, s.h, s.ax, s.ay), (24, 24, 12, 24));
            s.pixels
                .chunks(usize::from(s.w))
                .map(<[u8]>::to_vec)
                .collect::<Vec<_>>()
        };
        let (idle, walk, walk2) = (
            rows("cluckzarg_idle"),
            rows("cluckzarg_walk"),
            rows("cluckzarg_walk2"),
        );
        // Above the legs the walk frames are the same chicken.
        assert_eq!(walk[..19], walk2[..19]);
        // Each walk frame moves a different leg: the left foot is out in `walk2` only, the right foot in `walk` only.
        let col = |r: &[u8]| r.iter().position(|&p| p != 255).unwrap();
        let right = |r: &[u8]| r.iter().rposition(|&p| p != 255).unwrap();
        assert_eq!(col(&walk[22]), col(&idle[22]));
        assert!(right(&walk[22]) > right(&idle[22]));
        assert!(col(&walk2[22]) < col(&idle[22]));
        assert_eq!(right(&walk2[22]), right(&idle[22]));
    }

    #[test]
    fn a_stunned_chicken_looks_different_from_a_walking_one() {
        let mut w = level::world();
        w.ben.b.x = 8.0;
        w.ben.b.y = 11.0;
        let walking = picture(&w, 0);
        w.chickens[0].stun = 3.0;
        assert_ne!(walking, picture(&w, 0));
        // In its last second the chicken flashes: some ticks hide it altogether.
        w.chickens[0].stun = 0.5;
        assert_ne!(picture(&w, 0), picture(&w, 4));
    }

    #[test]
    fn ben_runs_with_fizz_1s_frame_rule() {
        // Fizz 1: a run frame above 0.5 tiles a second, `run2` when `floor(distance * 1.4)` is odd, pogo before shoot.
        let mut w = level::world();
        w.ben.b.on_ground = true;
        w.ben.b.vx = 3.0;
        w.ben.b.x = 1.0; // floor(1.4) = 1: run2
        let odd = picture(&w, 0);
        w.ben.b.x = 1.0 + 0.72; // floor(2.408) = 2: run1
        let even = picture(&w, 0);
        w.ben.b.x = 1.0 + 0.72 * 2.0; // floor(3.416) = 3: run2 again, at another screen position
        assert_ne!(odd, even);
        // Below the speed threshold Ben stands, wherever he is.
        w.ben.b.vx = 0.4;
        w.ben.b.x = 1.0;
        let slow = picture(&w, 0);
        w.ben.b.vx = 0.0;
        assert_eq!(slow, picture(&w, 0));
        // On the pogo stick the pogo frame wins over the shooting pose.
        w.ben.pogo = true;
        let pogo = picture(&w, 0);
        w.ben.shoot_t = 0.2;
        assert_eq!(pogo, picture(&w, 0));
    }

    #[test]
    fn ben_changes_with_pose_and_facing() {
        let mut w = level::world();
        w.ben.b.on_ground = true;
        let stand = picture(&w, 0);
        w.ben.face = -1.0;
        let flipped = picture(&w, 0);
        w.ben.face = 1.0;
        w.ben.pogo = true;
        let pogo = picture(&w, 0);
        w.ben.squash = 0.1;
        let squash = picture(&w, 0);
        w.ben.pogo = false;
        w.ben.squash = 0.0;
        w.ben.shoot_t = 0.2;
        let shoot = picture(&w, 0);
        w.ben.shoot_t = 0.0;
        w.ben.dead = 0.5;
        let dead = picture(&w, 0);
        let all = [&stand, &flipped, &pogo, &squash, &shoot, &dead];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate().skip(i + 1) {
                assert!(a != b, "poses {i} and {j} look the same");
            }
        }
        // Invulnerable Ben blinks.
        w.ben.dead = 0.0;
        w.ben.inv = 1.0;
        assert_ne!(picture(&w, 0), picture(&w, 3));
    }

    #[test]
    fn a_shot_is_drawn() {
        let mut w = level::world();
        let none = picture(&w, 0);
        w.shots.push(fizz_ep2::Shot {
            x: 5.0,
            y: 3.0,
            vx: 16.0,
            vy: 0.0,
            life: 1.0,
        });
        assert_ne!(none, picture(&w, 0));
    }

    #[test]
    fn drawing_leaves_the_rules_alone() {
        let mut a = level::world();
        let mut b = level::world();
        let mut prev = Buttons::NONE;
        let mut f = vec![0u8; VIEW_W * VIEW_H];
        for &(frames, now) in SLICE_SCRIPT.iter().take(10) {
            for _ in 0..frames {
                let input = InputFrame::from_transition(prev, now);
                prev = now;
                step(&mut a, input);
                step(&mut b, input);
                draw_slice(&mut f, VIEW_W, VIEW_H, &b, b.tick.0 as u32);
            }
        }
        assert_eq!(a.hash().finish(), b.hash().finish());
    }
}
