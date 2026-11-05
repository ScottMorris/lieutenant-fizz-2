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
//! 1. The ground, in oblique projection ([`oblique`], specified in `docs/OBLIQUE.md`): the back wall of the rooms or the open
//!    sky behind everything, then the portholes, the contact shadows and brackets under the wooden ledges, the railing on the
//!    gallery and the exit doorway, then the terrain from the bottom row up (planks you can jump up through, stone and brick
//!    walls, and the red and blue doors with their white badges, drawn closed and not at all once open), then the spikes. It
//!    writes every pixel of the frame and is baked data copied in runs (`tools/tiles/emit-oblique.ts`), so there is no separate
//!    sky or tile pass.
//! 2. The red key, if it still lies in the world, bobbing one pixel; the snacks and the blue key likewise.
//! 3. The chicken (the Cluckzarg placeholder art).
//! 4. The Fizz Blaster shots (the `bubble` sprite).
//! 5. The shot-end puffs a presenter's [`Puffs`] list holds (`puff0` to `puff3`).
//! 6. Ben (Fizz 1's sprites).
//! 7. At night ([`DrawOptions::night`]), the light pass: the cells the lights reach are shaded ([`light`]).
//! 8. The chicken's cluck caption, if [`DrawOptions::captions`] is on.
//!
//! # Night
//!
//! With [`DrawOptions::night`] the picture is drawn for the 256-entry palette of brightness banks ([`light::palette`] with the
//! theme's [`oblique::NIGHT_AMBIENT`], which the presenter shows instead of [`light::DAY`]): the ground as by day, which the
//! palette shows at the night's ambient; the night dressing ([`oblique::draw_night`]: the lamps, the lanterns, and the glow on
//! the door badges and the exit); the keys, snacks, shots and puffs in the glow bank and Ben and the chicken in the actor bank,
//! so they never go below level 9; then the lights ([`night_lights`]: Ben's lantern, pinned first, each shot's cyan light and
//! the slice's own) added up for each 8 by 8 cell, and the cells they reach shaded ([`light::shade`]). The captions are drawn
//! after, glowing. By day none of that runs and the picture is the one it always was.
//!
//! # Choices that are ours
//!
//! - **Chicken.** The art faces right, so a chicken walking left is flipped. It walks with both legs stepping (`walk`,
//!   `idle`, `walk2`, `idle` as the distance it has travelled grows, four pixels to a step) and shows the `alert` frame while it touches Ben. A stunned chicken is
//!   shown as the `alert` frame sunk two pixels, so it reads as knocked flat, and it flashes (disappears every other four
//!   ticks) for its last second of stun to warn that it is about to wake. Fizz 1's two-frame stun stars (`stars0`, `stars1`)
//!   also float 0.3 tiles above its head, swapping every [`STAR_FRAME_TICKS`] ticks (Fizz 1's 6 Hz), and flash away with the body.
//! - **Cluck beak.** While a chicken's `cluck_age` (ticks since its last cluck, kept by the rules and not hashed) is inside
//!   the open pattern for its variant, the `cluckzarg_beak` overlay is drawn over its idle, walk or walk2 body, so the beak
//!   moves mid-stride. The `alert` frame already has an open mouth and takes no overlay. See [`BEAK_OPEN`].
//! - **Cluck caption.** For [`CAPTION_TICKS`] ticks after a cluck, the variant's word floats above the chicken's head,
//!   centred on it, in whole-pixel steps up to [`CAPTION_RISE`] pixels, with a one-pixel black shadow. Its colour steps
//!   from bright to dark halfway through and blinks out over the last quarter. Fizz 1's in-world sound captions (`cap` in
//!   `episodes/episode-1/game/src/world.rs`, a table of text and colour; 950 ms, 24 pixels of rise, linear fade, a 1-pixel
//!   shadow) are the model; the slice is 320 pixels wide, so the rise is halved. The words are drawn as written, mixed case, in Fizz 1's own
//!   proportional pixel font (`fizz_view::pfont`; the HUD keeps the 8 by 8 capitals), centred by their measured width. A caption follows its chicken's
//!   position exactly and is cut off by the frame's edge as the chicken leaves it, never moved inside; it is dropped once the
//!   chicken is more than one tile ([`CAPTION_REACH`]) outside the frame. The captions option gates only the text; the beak always moves.
//! - **Shot-end puff.** Where a shot ends (the rules' `Event::ShotEnd`) Fizz 1 pushes a 16 by 16 puff that grows from 0.5x by
//!   1.5 a second and fades out over 0.3 s. DOS has no alpha or scaling, so the view plays [`PUFF_TICKS`] (18) ticks of
//!   four baked frames: `puff0` (8 pixels, 0.5x) for ticks 0 to 5, `puff1` (12 pixels, 0.75x) for 6 to 11, `puff2` (15
//!   pixels, 0.95x, Fizz 1's size at the end of its life) for 12 to 14, and `puff3`, `puff2` thinned to a sparse dither, for 15 to
//!   17. It is light magenta (Fizz 2's accent, `fizz_view::theme`), Ben's shot colour; the slice has no enemy shots, so there is no red one. A puff stays where the shot
//!   ended in the world, so the camera moves past it. The rules keep no puff state: [`Puffs`] is the view's own fixed list,
//!   fed by [`Puffs::observe`] after each step.
//! - **Ben.** Ben's pose follows the choice in Fizz 1's `design/melting/engine.js` (the `const sp` in the player draw): dead
//!   is `jump`; on the pogo stick it is the squashed `pogo2` during the squash after a bounce and `pogo` otherwise; then
//!   `shoot` while the rules' shooting pose runs (`aimUp` or `aimDown` when the shot went straight up or down, as Fizz 1's
//!   `render.rs` chooses), `jump` in the air, a run frame while his horizontal speed is above 0.5 tiles a second, `lookUp`
//!   or `lookDown` while a look timer runs (Fizz 1 shows them after the run frames), and `stand` at rest. Fizz 1 keeps a distance counter `anim`, adds `|vx| * dt` to it every step, and shows
//!   `run2` when `floor(anim * 1.4)` is odd and `run1` otherwise, so the frame changes every 1/1.4 tiles run and he never
//!   skates. The rules here keep no such counter and drawing may not add one, so the view uses Ben's x in its place: `run2`
//!   when `floor(|x| * 1.4)` is odd. The two agree in cadence, and differ only in phase after Ben turns round. The sprites face right, so facing left flips them. While Ben is
//!   invulnerable he blinks.
//! - **Dead Ben.** Fizz 1 draws a dead Ben with the `jump` pose, and its rules toss him up and let him fall (the same
//!   toss and fall as here), while its renderer turns his sprite: `rot += 8 * dt` every tick he is dead, about the sprite's
//!   centre, after the flip by his facing. With [`DrawOptions::shake`] on (Full Motion) the view does the same ([`death_spin`],
//!   `blit_sprite_turned`): the angle is 8 radians a second times the ticks since the death, read from `Ben::dead`, and
//!   the sprite is turned by inverse mapping, one source pixel for each destination pixel (nearest neighbour, no blur), with
//!   integers and a baked table of cosines and sines, so Web and DOS draw the same pixels and no float sine runs. He does not
//!   flicker while he spins. With it off (Reduced Motion) he is drawn as before: the `jump` pose, arms up, flipped away from
//!   his facing, and flickering. The world respawns him afterwards.
//!
//! # The camera
//!
//! After Ben dies the view shakes sideways for [`DEATH_SHAKE_TICKS`] ticks if [`DrawOptions::shake`] is on ([`death_shake_x`]),
//! and Ben's sprite spins as he falls ([`death_spin`]).
//!
//! `fizz_view::follow` centres the view on the middle of Ben's body on both axes and clamps it to the map, in whole
//! logical pixels, so the map scrolls without sub-pixel shimmer. While Ben looks up or down the rules' `look_offset` (up to
//! 3.6 tiles, ramped and returned by the rules) is added to that centre before the clamp, so the view never leaves the map. The map's y-axis is flipped with `fizz_view::screen_y`.

mod captions;
pub use captions::*;
mod fall;
pub use fall::FallWatch;
#[cfg(test)]
extern crate std;
pub mod light;
pub mod oblique;
#[rustfmt::skip]
mod oblique_data;
pub mod hud;
pub use hud::{HudAnim, HudModel, HudView, KeyKind, KeySlot};

use fizz_ep2::SnackKind;
use fizz_ep2::World;
use fizz_ep2::tiles::{DOOR_BLUE_CHANNEL, DOOR_RED_CHANNEL};
use fizz_view::sprites_data::sprite;
use fizz_view::{
    TILE_PX, blit_sprite_banked, follow, pfont::draw_text, pfont::text_width, screen_y, tile_px,
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

/// How a picture is drawn, beyond the world.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrawOptions {
    /// Draw the in-world captions that float above a chicken after it clucks. Off hides only the text; the beak still
    /// moves.
    pub captions: bool,
    /// The Motion setting: Full (on) shakes the view sideways for a moment after Ben dies and tumbles his sprite as he falls
    /// (Fizz 1's death shake and spin); Reduced (off) draws the camera still and Ben in a flat, flickering jump pose. A
    /// view effect only: the rules never see it. See [`death_shake_x`] and [`death_spin`].
    pub shake: bool,
    /// The Sky setting: Full (on) draws the open air's whole parallax night (stars, drifting clouds, the far headlands and the
    /// sea); Light (off) draws its stars and sea only, which costs a 486 about a third as much. A view effect only, and only
    /// open air differs. See `oblique::draw_ground_sky`.
    pub full_sky: bool,
    /// The Night setting: on draws the level at night, for the night palette ([`palette_for`]) and lit by the lights (see the
    /// module's "Night"); off, the default, draws it by day exactly as before the setting. A view effect only: the rules never
    /// see it.
    pub night: bool,
}

impl Default for DrawOptions {
    /// Captions on, motion (shake and spin) on, the full sky, by day.
    fn default() -> Self {
        DrawOptions {
            captions: true,
            shake: true,
            full_sky: true,
            night: false,
        }
    }
}

/// The palette a presenter shows a picture drawn with `options` in, as 6-bit DAC values: [`light::DAY`], or at night the one
/// whose bank 0 is the slice theme's night ambient. DOS writes it to the DAC; the Web expands it with [`light::rgba`].
pub fn palette_for(options: &DrawOptions) -> [[u8; 3]; 256] {
    if options.night {
        light::palette(oblique::NIGHT_AMBIENT)
    } else {
        light::DAY
    }
}

/// Ben's lantern at night, Fizz 1's for a theme with `lantern: true` (`update_lights` in `episodes/episode-1/game/src/render.rs`):
/// at the middle of his body and 1.1 tiles over his feet, 1.2 tiles high, a radius of 5.5 tiles and a warm `[0.95, 0.7, 0.45]`,
/// whose brightness (`0.3 r + 0.59 g + 0.11 b`) is 0.75. It is pinned first, so the light budget never drops it. Without it Ben
/// would walk through the dark between the lanterns, lit only by his floor of level 9, and the ledges he lands on would be
/// at the ambient; with it the floor around him reads, as in Fizz 1.
const BEN_LANTERN: (f64, i32, i32, i32) = (1.1, 19, 88, 765);
/// The light every shot carries: Fizz 1's Fizz Blaster bubble, cyan `[0.3, 1.1, 1.2]` (brightness 0.87), 0.6 tiles high, radius 3.
const SHOT_LIGHT: (i32, i32, i32) = (10, 48, 892);

/// The lights of a night picture of `world` at animation tick `tick` with the camera's top left at (`cam_x`, `cam_y`) in a `w`
/// by `h` view, nearest the camera first and at most [`light::MAX_ACTIVE`] (Fizz 1's budget): Ben's lantern (unless he is
/// dead), each shot's light and the slice's own ([`oblique::night_lights`]).
pub fn night_lights(
    world: &World,
    ground: &oblique::Ground,
    cam: (i32, i32),
    (w, h): (usize, usize),
    tick: u32,
) -> light::Lights {
    use light::{Light, SUB};
    let map_h = world.map.h;
    let centre = (
        (cam.0 + w as i32 / 2).saturating_mul(SUB),
        (cam.1 + h as i32 / 2).saturating_mul(SUB),
    );
    let mut lights = light::Lights::new();
    let ben = &world.ben;
    if ben.dead <= 0.0 {
        let (rise, z, r, s) = BEN_LANTERN;
        let at = Light {
            x: tile_px(ben.b.x + ben.b.w * 0.5).saturating_mul(SUB),
            y: screen_y(ben.b.y + rise, map_h).saturating_mul(SUB),
            z: z * SUB,
            r: r * SUB,
            strength: s,
        };
        if let Some(i) = lights.add(centre, at) {
            lights.pin_first(i);
        }
    }
    for shot in &world.shots {
        let (z, r, s) = SHOT_LIGHT;
        lights.add(
            centre,
            Light {
                x: tile_px(shot.x).saturating_mul(SUB),
                y: screen_y(shot.y, map_h).saturating_mul(SUB),
                z: z * SUB,
                r: r * SUB,
                strength: s,
            },
        );
    }
    oblique::night_lights(&mut lights, ground, cam, (w, h), tick);
    lights
}

/// The cells of the largest grid kept on the stack for a DOS-sized view (40 by 25 at 320 by 200), and of the largest for any view
/// the Web draws (1280 by 416 is 160 by 52).
const SMALL_GRID: usize = 1024;
const LARGE_GRID: usize = 160 * 52;

/// The light pass over a picture drawn at night: adds the lights to the cell grid and shades the frame by it.
fn light_pass(frame: &mut [u8], w: usize, h: usize, lights: &[light::Light], cam: (i32, i32)) {
    let (cols, rows) = light::grid_size(w, h);
    let cam = (
        cam.0.saturating_mul(light::SUB),
        cam.1.saturating_mul(light::SUB),
    );
    let mut pass = |add: &mut [u16]| {
        light::add_lights(add, cols, rows, lights, cam);
        light::shade(frame, w, h, add, oblique::NIGHT_AMBIENT);
    };
    if cols * rows <= SMALL_GRID {
        pass(&mut [0u16; SMALL_GRID]);
    } else if cols * rows <= LARGE_GRID {
        pass(&mut [0u16; LARGE_GRID]);
    }
}

/// How many ticks the death shake lasts: Fizz 1 sets `shake` to 0.6 and takes 2 off it every second, so it is gone in
/// 0.3 s, which is 18 ticks.
pub const DEATH_SHAKE_TICKS: usize = 18;

/// The camera's horizontal offset in whole pixels, per tick since Ben died (tick 0 is the tick of the death). Fizz 1 offsets
/// the camera by `sin(t * 70) * shake * 0.15` tiles with `shake = 0.6 - 2t` for `t` seconds since the death; at 16 pixels
/// to the tile that is `sin(n / 60 * 70) * (1.44 - 0.08 n)` pixels for tick `n`, rounded to the nearest pixel and baked
/// here so the Web and DOS builds cannot differ in a float sine. It never passes 1 pixel either way (Fizz 1's peak is 1.44).
const DEATH_SHAKE_X: [i32; DEATH_SHAKE_TICKS] =
    [0, 1, 1, 0, -1, 0, 1, 1, 0, -1, -1, 0, 0, 0, 0, 0, 0, 0];

/// The horizontal camera offset for the death shake, in pixels: zero unless Ben is dead and within [`DEATH_SHAKE_TICKS`]
/// of dying. Derived from `Ben::dead`, which the rules already keep and hash (0.001 at the death, then a tick longer each
/// tick), so it adds no state.
pub fn death_shake_x(world: &World) -> i32 {
    let dead = world.ben.dead;
    // `dead` is 0.001 plus whole ticks of 1/60 s; scaling to ticks and flooring recovers the count despite the float sum.
    if dead > 0.0 && dead < 1.0 {
        DEATH_SHAKE_X
            .get((dead * f64::from(TICKS_PER_SECOND)) as usize)
            .copied()
            .unwrap_or(0)
    } else {
        0
    }
}

/// How many ticks of spin the table below holds: Ben lies dead for 1.4 s (84 ticks, `fizz_ep2::world::RESPAWN_AFTER`), and
/// the world respawns him on the tick after, so tick 84 is the last he can be drawn at; one more is kept in hand.
pub const DEATH_SPIN_TICKS: usize = 85;

/// Bits of fraction in [`DEATH_SPIN`].
const SPIN_BITS: u32 = 12;

/// `(cos, sin)` of Fizz 1's death angle `rot = 8 * n / 60` radians (it adds `dt * 8.0` to `rot` every tick Ben is dead), for
/// the `n`th tick since the death, times 4096 and rounded. Baked so that no float sine is run in the view and the Web and DOS
/// builds, whose float units differ, draw the same pixels; a test checks every entry against `f64` trigonometry.
const DEATH_SPIN: [(i32, i32); DEATH_SPIN_TICKS + 1] = [
    (4096, 0),
    (4060, 545),
    (3951, 1079),
    (3773, 1595),
    (3527, 2082),
    (3219, 2533),
    (2854, 2938),
    (2438, 3292),
    (1979, 3586),
    (1484, 3818),
    (964, 3981),
    (426, 4074),
    (-120, 4094),
    (-663, 4042),
    (-1194, 3918),
    (-1705, 3724),
    (-2185, 3465),
    (-2626, 3144),
    (-3020, 2767),
    (-3361, 2341),
    (-3643, 1873),
    (-3859, 1372),
    (-4007, 847),
    (-4085, 307),
    (-4089, -239),
    (-4021, -781),
    (-3881, -1308),
    (-3673, -1813),
    (-3400, -2285),
    (-3066, -2716),
    (-2677, -3100),
    (-2241, -3428),
    (-1766, -3696),
    (-1259, -3898),
    (-729, -4031),
    (-187, -4092),
    (358, -4080),
    (898, -3996),
    (1421, -3842),
    (1919, -3619),
    (2383, -3331),
    (2805, -2985),
    (3177, -2586),
    (3492, -2140),
    (3746, -1657),
    (3933, -1144),
    (4050, -611),
    (4095, -68),
    (4068, 477),
    (3969, 1014),
    (3798, 1533),
    (3561, 2024),
    (3260, 2479),
    (2902, 2891),
    (2492, 3251),
    (2038, 3553),
    (1547, 3793),
    (1029, 3965),
    (493, 4066),
    (-52, 4096),
    (-596, 4052),
    (-1129, 3937),
    (-1643, 3752),
    (-2127, 3500),
    (-2573, 3187),
    (-2974, 2816),
    (-3322, 2396),
    (-3611, 1933),
    (-3836, 1436),
    (-3993, 913),
    (-4079, 374),
    (-4092, -172),
    (-4033, -714),
    (-3903, -1244),
    (-3703, -1752),
    (-3437, -2228),
    (-3110, -2665),
    (-2728, -3055),
    (-2298, -3391),
    (-1827, -3666),
    (-1323, -3876),
    (-796, -4018),
    (-255, -4088),
    (291, -4086),
    (832, -4011),
    (1357, -3865),
];

/// Ticks since Ben died, from `Ben::dead` (0.001 at the death, a tick longer each tick), or `None` while he is alive.
fn ticks_dead(world: &World) -> Option<usize> {
    let dead = world.ben.dead;
    // The scaling and flooring recover the whole tick count despite the float sum, as in [`death_shake_x`].
    (dead > 0.0).then(|| (dead * f64::from(TICKS_PER_SECOND)) as usize)
}

/// The `(cos, sin)` of the angle Ben's sprite is turned by, times 4096, when he is dead: 8 radians a second since the death,
/// counter-clockwise on screen as Fizz 1's matrix turns its sprite. `None` while he is alive. Derived from `Ben::dead` alone.
pub fn death_spin(world: &World) -> Option<(i32, i32)> {
    ticks_dead(world).map(|n| DEATH_SPIN[n.min(DEATH_SPIN_TICKS)])
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
/// Fizz 1, and `buh-GAWK!` keeps its capitals; it is drawn as written in `fizz_view::pfont`'s proportional face, so the width
/// of a word is `pfont::text_width`, not a fixed width per letter.
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

/// How long a shot-end puff lives: 0.3 s at 60 ticks a second, Fizz 1's `life: 0.3`.
pub const PUFF_TICKS: u32 = TICKS_PER_SECOND * 3 / 10;
/// The most puffs the list holds. Ben has five shots and a puff lasts 18 ticks, so a full list takes shots to end faster than
/// the slice's fire rate allows; if one ever does, the oldest puff is replaced.
pub const MAX_PUFFS: usize = 8;

/// The baked frame of a puff `age` ticks after its shot ended, or `None` once it is over: `puff0` for ticks 0 to 5, `puff1` for
/// 6 to 11, `puff2` for 12 to 14 and `puff3` (the dither) for 15 to 17.
pub const fn puff_frame(age: u32) -> Option<usize> {
    match age {
        0..=5 => Some(0),
        6..=11 => Some(1),
        12..=14 => Some(2),
        15..=17 => Some(3),
        _ => None,
    }
}

const PUFF_NAMES: [&str; 4] = ["puff0", "puff1", "puff2", "puff3"];

/// One live puff: where its shot ended (sixteenths of a tile, y up) and the tick the rules emitted the event on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Puff {
    x: i32,
    y: i32,
    born: u32,
}

/// The shot-end puffs a presenter is showing: a fixed list of [`MAX_PUFFS`], no allocation. It is view state, not rules
/// state; the Web and DOS presenters each keep one, call [`Puffs::observe`] after every `fizz_ep2::step` (the events last
/// for one step only) and hand it to [`draw_slice_puffs`], so both draw the same puffs on the same ticks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Puffs {
    list: [Puff; MAX_PUFFS],
    /// How many entries of `list` have ever been written, up to `MAX_PUFFS`.
    len: usize,
    /// Where the next puff goes: the oldest, once the list is full.
    next: usize,
}

impl Default for Puffs {
    fn default() -> Self {
        Self::new()
    }
}

impl Puffs {
    /// An empty list.
    pub const fn new() -> Self {
        Puffs {
            list: [Puff {
                x: 0,
                y: 0,
                born: 0,
            }; MAX_PUFFS],
            len: 0,
            next: 0,
        }
    }

    /// Takes the `ShotEnd` events of the step that just ran (at `world.tick`).
    pub fn observe(&mut self, world: &World) {
        for e in world.events() {
            if let fizz_ep2::Event::ShotEnd { x, y } = *e {
                self.list[self.next] = Puff {
                    x,
                    y,
                    born: world.tick.0 as u32,
                };
                self.next = (self.next + 1) % MAX_PUFFS;
                self.len = (self.len + 1).min(MAX_PUFFS);
            }
        }
    }

    /// Forgets every puff (a presenter restarting the level).
    pub fn clear(&mut self) {
        *self = Puffs::new();
    }

    /// The puffs still showing at `tick`, as `(x, y, frame)`: sixteenths of a tile, y up, and the index into the four baked frames.
    pub fn live(&self, tick: u32) -> impl Iterator<Item = (i32, i32, usize)> + '_ {
        self.list[..self.len].iter().filter_map(move |p| {
            let frame = puff_frame(tick.wrapping_sub(p.born))?;
            Some((p.x, p.y, frame))
        })
    }
}

/// Draws `world` as [`draw_slice`] does, with `options` and no puffs.
pub fn draw_slice_with(
    frame: &mut [u8],
    w: usize,
    h: usize,
    world: &World,
    tick: u32,
    options: &DrawOptions,
) {
    draw_slice_puffs(frame, w, h, world, tick, options, &Puffs::new());
}

/// Draws `world` as [`draw_slice_with`] does, with the shot-end puffs `puffs` holds, aged by `world.tick`.
pub fn draw_slice_puffs(
    frame: &mut [u8],
    w: usize,
    h: usize,
    world: &World,
    tick: u32,
    options: &DrawOptions,
    puffs: &Puffs,
) {
    draw_slice_captions(
        frame,
        w,
        h,
        world,
        tick,
        options,
        puffs,
        &EventCaptions::new(),
    );
}

/// Draws `world` as [`draw_slice_puffs`] does, and the event captions `captions` holds after the chickens' own, when
/// `options.captions` is on.
#[allow(clippy::too_many_arguments)]
pub fn draw_slice_captions(
    frame: &mut [u8],
    w: usize,
    h: usize,
    world: &World,
    tick: u32,
    options: &DrawOptions,
    puffs: &Puffs,
    captions: &EventCaptions,
) {
    draw_picture(frame, w, h, world, tick, options, puffs, captions, true);
}

/// [`draw_slice_captions`], with the light pass left out when `shade` is false (for the tests, which check the markers it reads).
#[allow(clippy::too_many_arguments)]
fn draw_picture(
    frame: &mut [u8],
    w: usize,
    h: usize,
    world: &World,
    tick: u32,
    options: &DrawOptions,
    puffs: &Puffs,
    captions: &EventCaptions,
    shade: bool,
) {
    let map = &world.map;
    let (map_w, map_h) = (map.w, map.h);

    // The camera centres on the middle of Ben's body, raised or lowered by the look offset the rules keep (zero unless Ben is
    // looking up or down, or the look is returning). `follow` clamps the result to the map exactly as it does for Ben alone.
    let (cam_x, cam_y) = camera_shaken(world, w, h, options);

    // The ground: backgrounds, terrain and doors in oblique projection, written over the whole frame.
    let ground = oblique::Ground {
        data: &map.data,
        w: map_w,
        h: map_h,
        red_closed: map.switch(DOOR_RED_CHANNEL),
        blue_closed: map.switch(DOOR_BLUE_CHANNEL),
    };
    oblique::draw_ground_sky(frame, w, h, &ground, cam_x, cam_y, tick, options.full_sky);
    let night = options.night;
    if night {
        oblique::draw_night(frame, w, h, &ground, cam_x, cam_y);
    }
    // At night the art carries its marker in its bank (see `light`): pickups, shots and puffs glow, Ben and the chicken are actors.
    let (glow, actor) = if night {
        (light::GLOW, light::ACTOR)
    } else {
        (0, 0)
    };

    // Sprites are placed by their anchor, which is feet and centre. `at` turns a world position into frame pixels.
    let at = |x: f64, y: f64| (tile_px(x) - cam_x, screen_y(y, map_h) - cam_y);
    let phase = tick as usize;

    // The items bob a pixel every 16 ticks, as the key does. Their sprites are anchored at the feet; an item's centre is half a
    // sprite above them.
    let bob = i32::from(phase / 16 % 2 == 1);
    if !world.snacks.is_empty()
        && let (Some(cheezie), Some(choc), Some(cookie), Some(soda), Some(elixir)) = (
            sprite("cheezie"),
            sprite("choc"),
            sprite("cookie"),
            sprite("soda"),
            sprite("elixir"),
        )
    {
        for sn in world.snacks.iter().filter(|sn| !sn.taken) {
            let art = match sn.kind {
                SnackKind::Cheezie => cheezie,
                SnackKind::Choc => choc,
                SnackKind::Cookie => cookie,
                SnackKind::Soda => soda,
                SnackKind::Elixir => elixir,
            };
            let (sx, sy) = at(sn.x, sn.y);
            blit_sprite_banked(frame, w, h, art, sx, sy + 8 - bob, false, glow);
        }
    }

    if let (Some((kx, ky)), Some(key)) = (world.key_blue_position(), sprite("keyBlue")) {
        let (sx, sy) = at(kx, ky);
        blit_sprite_banked(frame, w, h, key, sx, sy + 8 - bob, false, glow);
    }

    if let (Some((kx, ky)), Some(key)) = (world.key_position(), sprite("keyRed")) {
        // The sprite's anchor is its feet; the key's centre is half a sprite above them.
        let (sx, sy) = at(kx, ky);
        blit_sprite_banked(frame, w, h, key, sx, sy + 8 - bob, false, glow);
    }

    let walls = Walls {
        ground: &ground,
        map_h,
    };
    for c in &world.chickens {
        draw_chicken(frame, w, h, c, phase, &at, actor);
    }

    if let Some(bubble) = sprite("bubble") {
        for s in &world.shots {
            let (sx, sy) = at(s.x, s.y);
            // A bubble pops at the wall, so it is drawn where it is and is not stopped at the wall's edge.
            blit_sprite_banked(frame, w, h, bubble, sx, sy + 4, false, glow);
        }
    }

    // Fizz 1 draws its effects after the shots and before Ben.
    for (px, py, which) in puffs.live(world.tick.0 as u32) {
        if let Some(puff) = sprite(PUFF_NAMES[which]) {
            let (sx, sy) = (px - cam_x, map_h.saturating_mul(TILE_PX) - py - cam_y);
            // The puff is anchored at its centre, as the shot's position is the centre of the bubble.
            blit_sprite_banked(frame, w, h, puff, sx, sy, false, glow);
        }
    }

    draw_ben(frame, w, h, world, phase, &at, options.shake, actor, &walls);

    // The light pass is the Full detail's (the Sky setting): with Light, the detail the 486DX2-66 floor draws, the night is the
    // palette's ambient, the glowing art and the actors' floor, and no cell is shaded.
    if night && shade && options.full_sky {
        let mut lights = night_lights(world, &ground, (cam_x, cam_y), (w, h), tick);
        light_pass(frame, w, h, lights.select(), (cam_x, cam_y));
    }

    if options.captions {
        for c in &world.chickens {
            draw_caption(frame, w, h, c, &at, glow);
        }
        for cap in captions.views(world, w, h, options) {
            draw_caption_view(frame, w, h, &cap, glow);
        }
    }
}

/// [`camera`] with the death shake added to the target when `options.shake` is on; `follow` then clamps it to the map as
/// ever, so the shake never shows beyond the map's edge.
fn camera_shaken(world: &World, w: usize, h: usize, options: &DrawOptions) -> (i32, i32) {
    let shake = if options.shake {
        death_shake_x(world)
    } else {
        0
    };
    let (map_w, map_h) = (world.map.w, world.map.h);
    let ben = &world.ben;
    let target_x = tile_px(ben.b.x + ben.b.w * 0.5).saturating_add(shake);
    let target_y = screen_y(ben.b.y + ben.b.h * 0.5 + ben.look_offset, map_h);
    (
        follow(target_x, map_w.saturating_mul(TILE_PX), w as i32),
        follow(target_y, map_h.saturating_mul(TILE_PX), h as i32),
    )
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

/// Where a chicken's cluck sounds in the view: the chicken's body centre in map pixels, placed in the `w` by `h` view the camera
/// draws (the shaken camera when `options.shake` is on, as [`draw_slice_with`] uses). Both presenters call this, so they agree on
/// the source and the camera; the result is for sound only. Pure: it reads the chicken and the camera, writes nothing, and the rules
/// never see it. An unknown chicken gives the centre.
pub fn cluck_placement(
    world: &World,
    chicken: u8,
    w: usize,
    h: usize,
    options: &DrawOptions,
) -> fizz_sfx::place::Placement {
    let Some(c) = world.chickens.get(usize::from(chicken)) else {
        return fizz_sfx::place::Placement::CENTRE;
    };
    let (cam_x, cam_y) = camera_shaken(world, w, h, options);
    let x = tile_px(c.b.x + c.b.w * 0.5);
    let y = screen_y(c.b.y + c.b.h * 0.5, world.map.h);
    fizz_sfx::place::place(
        x,
        y,
        fizz_sfx::place::View {
            x: cam_x,
            y: cam_y,
            w: w as i32,
            h: h as i32,
        },
    )
}

/// Where Ben's 16 by 16 sprite lies in the `w` by `h` view the camera draws (the shaken camera when `options.shake` is on):
/// `(left, top, width, height)` in view pixels, from his feet and centre as `draw_ben` places the sprite. For a presenter that
/// lays something over the picture and must keep it off Ben, such as the Web page's HUD. It includes the draw offset that stops him
/// at a wall on his right (at most 6 px). Pure: nothing in the rules reads it.
pub fn ben_view_box(
    world: &World,
    w: usize,
    h: usize,
    options: &DrawOptions,
) -> (i32, i32, i32, i32) {
    let (cam_x, cam_y) = camera_shaken(world, w, h, options);
    let ben = &world.ben;
    // The sprite is drawn offset sideways when a wall is on his right, as `draw_ben` does, and the box follows it.
    let mut x = tile_px(ben.b.x + ben.b.w * 0.5) - cam_x;
    if ben.dead <= 0.0 {
        let map = &world.map;
        let ground = oblique::Ground {
            data: &map.data,
            w: map.w,
            h: map.h,
            red_closed: map.switch(DOOR_RED_CHANNEL),
            blue_closed: map.switch(DOOR_BLUE_CHANNEL),
        };
        if let Some(s) = sprite(ben_pose(ben)) {
            let walls = Walls {
                ground: &ground,
                map_h: map.h,
            };
            let b = &ben.b;
            x += walls.offset(s, ben.face < 0.0, (b.x, b.y, b.w, b.h));
        }
    }
    let y = screen_y(ben.b.y, world.map.h) - cam_y;
    (x - 8, y - 16, 16, 16)
}

/// How many ticks each stun-star frame shows. Fizz 1 swaps frames when `floor(t * 6)` changes: six swaps a second, a
/// 1/6 s frame, so 60 Hz / 6 Hz = 10 ticks a frame and a full two-frame cycle of 20 ticks (3 Hz). Dividing by the two
/// states as well would give 5 ticks, which is twelve swaps a second, twice Fizz 1's rate.
pub const STAR_FRAME_TICKS: usize = 10;

/// Which stun-star frame (0 or 1) shows at animation tick `phase`. Fizz 1 keys it to the enemy's own clock; the view has
/// only the tick, so every stunned chicken shows the same frame.
pub const fn star_frame(phase: usize) -> usize {
    phase / STAR_FRAME_TICKS % 2
}

type At<'a> = &'a dyn Fn(f64, f64) -> (i32, i32);

#[allow(clippy::too_many_arguments)]
fn draw_chicken(
    frame: &mut [u8],
    w: usize,
    h: usize,
    c: &fizz_ep2::Chicken,
    phase: usize,
    at: At,
    bank: u8,
) {
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
    // Enemies are drawn where their bodies are: an enemy's collision body contains its sprite and the 4 px near-plane margin on
    // its right, so it needs no correction at a wall (docs/OBLIQUE.md).
    blit_sprite_banked(frame, w, h, s, sx, sy + sink, flip, bank);
    if stunned
        && let Some(stars) = sprite(if star_frame(phase) == 0 {
            "stars0"
        } else {
            "stars1"
        })
    {
        // Fizz 1 centres the stars 0.3 tiles above the enemy's head; the 8-row sprite then ends 0.3 * 16 - 4 = 0.8, about one
        // pixel, above it. The head is the top of the sprite drawn, so the stars follow the sink. They are not flipped.
        blit_sprite_banked(
            frame,
            w,
            h,
            stars,
            sx,
            sy + sink - i32::from(s.h) - 1,
            false,
            bank,
        );
    }
    // The overlay shares the body's anchor and flip, so it lands on the beak whichever way the chicken faces and whichever
    // body frame shows. The alert frame has its own open mouth.
    if name != "cluckzarg_alert"
        && beak_open(c.cluck_variant, c.cluck_age)
        && let Some(beak) = sprite("cluckzarg_beak")
    {
        blit_sprite_banked(frame, w, h, beak, sx, sy + sink, flip, bank);
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
    /// Whether this is an event caption ([`EventCaptions`]) rather than a chicken's: its `x` and `y` are then the centre of the
    /// word's rest position, not a chicken's feet, so a presenter lifts it by nothing instead of [`CAPTION_ABOVE_FEET`].
    pub event: bool,
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
        event: false,
    })
}

/// Draws the chicken's caption as pixel text, the downscaled form for the 320 by 200 picture: the variant's word above its
/// head, rising and fading with `cluck_age`. The Web page draws the same captions as DOM text instead.
fn draw_caption(frame: &mut [u8], w: usize, h: usize, c: &fizz_ep2::Chicken, at: At, bank: u8) {
    if let Some(cap) = caption_at(c, w, h, at) {
        draw_caption_view(frame, w, h, &cap, bank);
    }
}

/// Draws one caption view: bright, then dark, then blinking out, rising [`CAPTION_RISE`] pixels, with a black shadow. The
/// word's colour takes `bank` (the glow bank at night, so it stays at full brightness over a shaded picture).
fn draw_caption_view(frame: &mut [u8], w: usize, h: usize, cap: &CaptionView, bank: u8) {
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
    // A chicken's caption sits above its head; an event's is centred on its point (the glyph cell is 8 rows).
    let top = cap.y - if cap.event { 4 } else { CAPTION_ABOVE_FEET } - rise;
    draw_text(frame, w, h, left + 1, top + 1, text, 0);
    draw_text(frame, w, h, left, top, text, colour | bank);
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

#[allow(clippy::too_many_arguments)]
fn draw_ben(
    frame: &mut [u8],
    w: usize,
    h: usize,
    world: &World,
    phase: usize,
    at: At,
    spin: bool,
    bank: u8,
    walls: &Walls,
) {
    let ben = &world.ben;
    let dead = ben.dead > 0.0;
    let turn = if spin { death_spin(world) } else { None };
    // Invulnerable Ben blinks; dead Ben flickers, unless he is spinning, which Fizz 1 does without a flicker.
    if (ben.inv > 0.0 || (dead && turn.is_none())) && phase / 3 % 2 == 1 {
        return;
    }
    let name = ben_pose(ben);
    let Some(s) = sprite(name) else {
        return;
    };
    let (sx, sy) = at(ben.b.x + ben.b.w * 0.5, ben.b.y);
    if let Some((cos, sin)) = turn {
        // Fizz 1 flips by `face` alone, dead or alive, and turns the flipped sprite about its centre.
        blit_sprite_turned(frame, w, h, s, sx, sy, ben.face < 0.0, cos, sin, bank);
        return;
    }
    // The sprites face right; dead Ben is turned away from the way he was facing.
    let flip = if dead { ben.face > 0.0 } else { ben.face < 0.0 };
    // A living Ben is drawn stopping at the front edge of a wall on his right. Dead Ben does not collide (he is knocked up
    // and falls through the level), so he is drawn where his body is, as the turn above is.
    let sx = if dead {
        sx
    } else {
        sx + walls.offset(s, flip, (ben.b.x, ben.b.y, ben.b.w, ben.b.h))
    };
    blit_sprite_banked(frame, w, h, s, sx, sy, flip, bank);
}

/// What Ben needs to be drawn stopping at a wall on his right: the map as the renderer reads it and its height.
struct Walls<'a> {
    ground: &'a oblique::Ground<'a>,
    map_h: i32,
}

impl Walls<'_> {
    /// The horizontal draw offset of `sprite`, anchored at frame column `x` (mirrored if `flip`), for Ben's body. See
    /// [`oblique::wall_offset`]: the sprite's opaque right edge lands on the front line of a wall on the right. A wall on the
    /// left adds nothing. Zero, and no scan of the sprite, unless a wall is within [`oblique::WALL_RAMP`] pixels of the body's
    /// right.
    fn offset(
        &self,
        sprite: &fizz_view::sprites_data::Sprite,
        flip: bool,
        (bx, by, bw, bh): (f64, f64, f64, f64),
    ) -> i32 {
        let right = tile_px(bx + bw);
        let (top, bottom) = (screen_y(by + bh, self.map_h), screen_y(by, self.map_h));
        let Some((_, edge)) = oblique::wall_gap(self.ground, right, top, bottom) else {
            return 0;
        };
        let cols = drawn_columns(sprite, flip);
        // Where the anchor is when the body touches the wall: its rounded position then. The gap that sets the ramp is the
        // anchor's own distance from that, not the body edge's, so the sprite (anchor plus offset) moves with the anchor's
        // whole pixel steps and never back, however the two roundings fall.
        let at_contact = tile_px(f64::from(edge) / 16.0 - bw * 0.5);
        let gap = at_contact - tile_px(bx + bw * 0.5);
        oblique::wall_offset(Some((gap, edge - at_contact)), cols)
    }
}

/// The sprite's opaque columns as drawn, relative to its anchor: `(first, past the last)`, mirrored when `flip`.
fn drawn_columns(sprite: &fizz_view::sprites_data::Sprite, flip: bool) -> (i32, i32) {
    let (first, last) = opaque_columns(sprite);
    let ax = i32::from(sprite.ax);
    if flip {
        (ax - last - 1, ax - first)
    } else {
        (first - ax, last + 1 - ax)
    }
}

/// The first and last columns of `sprite` that hold an opaque pixel (0 and 0 for an empty one).
fn opaque_columns(sprite: &fizz_view::sprites_data::Sprite) -> (i32, i32) {
    let (sw, sh) = (usize::from(sprite.w), usize::from(sprite.h));
    let (mut first, mut last) = (i32::MAX, -1);
    if sprite.pixels.len() >= sw * sh {
        for row in sprite.pixels.chunks_exact(sw.max(1)).take(sh) {
            for (c, &p) in row.iter().enumerate() {
                if p != fizz_view::TRANSPARENT {
                    first = first.min(c as i32);
                    last = last.max(c as i32);
                }
            }
        }
    }
    if last < 0 { (0, 0) } else { (first, last) }
}

/// Draws `sprite` as [`blit_sprite`] does, with its anchor at (`x`, `y`), mirrored if `flip_x`, then turned about the middle of
/// its box by the angle whose cosine and sine (times 4096) are `cos` and `sin`, counter-clockwise on screen. Nearest
/// neighbour, no blur: each destination pixel's centre is mapped back through the turn to one source pixel, in integers only
/// (offsets doubled and the table's 12-bit values, so nothing nears overflow; a row is walked by additions); a source pixel outside the
/// box, or the transparent value, draws nothing. A turn of zero is exactly [`blit_sprite`]. Clipped to the frame; any
/// position is safe. Cost: a square of twice half the diagonal, plus one, pixels a side (31 by 31 for Ben's 16 by 24), a few multiplications each.
#[allow(clippy::too_many_arguments)]
fn blit_sprite_turned(
    frame: &mut [u8],
    w: usize,
    h: usize,
    sprite: &fizz_view::sprites_data::Sprite,
    x: i32,
    y: i32,
    flip_x: bool,
    cos: i32,
    sin: i32,
    bank: u8,
) {
    let (sw, sh) = (i32::from(sprite.w), i32::from(sprite.h));
    if w == 0 || h == 0 || sw == 0 || sh == 0 || sprite.pixels.len() < (sw * sh) as usize {
        return;
    }
    let rows = (frame.len() / w).min(h) as i32;
    // The sprite's box, where `blit_sprite` would draw it.
    let left = if flip_x {
        x + i32::from(sprite.ax) - sw
    } else {
        x - i32::from(sprite.ax)
    };
    let top = y - i32::from(sprite.ay);
    // Twice the box's middle, so an odd size still has an exact centre.
    let (mid_x2, mid_y2) = (2 * left + sw, 2 * top + sh);
    // Half the box's diagonal, rounded up: no pixel of the turned box is further than this from the middle, whatever the turn.
    let mut reach = 1;
    while 4 * reach * reach < sw * sw + sh * sh {
        reach += 1;
    }
    let (cx, cy) = (mid_x2.div_euclid(2), mid_y2.div_euclid(2));
    let (x0, x1) = ((cx - reach).max(0), (cx + reach + 1).min(w as i32));
    for py in (cy - reach).max(0)..(cy + reach + 1).min(rows) {
        let dy = 2 * py + 1 - mid_y2;
        // The turn undone: the offset from the middle in the sprite's own pixels, doubled and times 4096, with the sprite's
        // half size added so that it counts from the box's top left. A step of one pixel is a step of `2 * cos` and `2 * sin`,
        // so the row is walked by additions, not the two multiplications a 486 would pay a pixel for.
        let dx = 2 * x0 + 1 - mid_x2;
        let mut u = dx * cos - dy * sin + (sw << SPIN_BITS);
        let mut v = dx * sin + dy * cos + (sh << SPIN_BITS);
        for px in x0..x1 {
            let (cu, cv) = (u, v);
            u += 2 * cos;
            v += 2 * sin;
            let mut col = cu >> (SPIN_BITS + 1);
            let row = cv >> (SPIN_BITS + 1);
            if flip_x {
                col = sw - 1 - col;
            }
            if col < 0 || col >= sw || row < 0 || row >= sh {
                continue;
            }
            let p = sprite.pixels[(row * sw + col) as usize];
            if p != fizz_view::TRANSPARENT {
                frame[py as usize * w + px as usize] = p | bank;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fizz_view::theme::FIZZ_INDEX;

    const MAGENTA: u8 = 5;
    use fizz_ep2::level;
    use fizz_ep2::tiles::{BLOCK, DOOR_B, DOOR_R, EXIT, FILL, PLAT, SPIKE, WALLBG};
    use fizz_ep2::world::{WorldFlags, step};
    use fizz_ep2::{SLICE_SCRIPT, SLICE_TICKS};
    use fizz_sim::{Buttons, InputFrame};

    extern crate std;
    use std::vec;
    use std::vec::Vec;

    const PINNED_CLUCKS: [(u64, u64, u64); 5] = [
        (197, 0x4c650f748ae497e1, 0x27bd3d1c68c0a099),
        (217, 0x7cde865b828ea386, 0xc59ae3e97b3c9f10),
        (853, 0xd975ed46e10b710f, 0xd975ed46e10b710f),
        (873, 0x992f23fd749b0115, 0x992f23fd749b0115),
        (1094, 0x58f794bd4e37e929, 0x58f794bd4e37e929),
    ];
    const PINNED_SYNTHETIC: [u64; 6] = [
        0x864162f2053ab4d8,
        0xf264e4cf27aa00fd,
        0xba85d6c850c5961d,
        0x397967520854dc48,
        0x4a137742b898fdce,
        0xd1cb0706ffbc05da,
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

    /// Death frames, from the death at tick 394 to just before the respawn, in both Motion settings.
    #[test]
    fn death_frames_are_pinned_full_and_reduced() {
        const TICKS: [u64; 10] = [394, 395, 396, 400, 410, 420, 440, 460, 470, 477];
        let (mut full, mut reduced) = (Vec::new(), Vec::new());
        play(|w| {
            if TICKS.contains(&w.tick.0) {
                let draw = |shake| {
                    let mut f = vec![0xEEu8; VIEW_W * VIEW_H];
                    let o = DrawOptions {
                        captions: true,
                        shake,
                        ..DrawOptions::default()
                    };
                    draw_slice_with(&mut f, VIEW_W, VIEW_H, w, w.tick.0 as u32, &o);
                    fnv(&f)
                };
                full.push((w.tick.0, draw(true)));
                reduced.push((w.tick.0, draw(false)));
            }
        });
        // Reduced is the picture as it was drawn before the spin, with the camera still: these are the digests the same
        // ticks had on the code before the spin (the flat flipped pose, flickering), unchanged by the spin. At 470 and 477 Ben has
        // fallen out of the frame, so Full is the same picture.
        assert_eq!(
            reduced,
            [
                (394, 0xa9bcb7102c2c7335),
                (395, 0x061e6b9af90e4c02),
                (396, 0xfdfafc30f6493bd8),
                (400, 0xe7ea08e8ca0c528d),
                (410, 0x6d061c44a9f69667),
                (420, 0xc06ee0f26f3a2614),
                (440, 0x3b95be129f246b31),
                (460, 0x26ff964c63f141a4),
                (470, 0x7d056bb0585ac30e),
                (477, 0x5413a16685440f58),
            ]
        );
        assert_eq!(
            full,
            [
                (394, 0x82961607f46b3a24),
                (395, 0xb07fcca732a873e1),
                (396, 0xbc03c3300fc95fb4),
                (400, 0x96988a931f7790d3),
                (410, 0xfcaa96fe131e4a96),
                (420, 0x99a011ff573a5855),
                (440, 0x9cb4fde8f7505da9),
                (460, 0x1038c1385d56a40a),
                (470, 0x7d056bb0585ac30e),
                (477, 0x5413a16685440f58),
            ]
        );
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
        // Tick 400 is 6 ticks into a death shake (Ben has just died), so the camera is a pixel to the right. The digest was
        // 0xe7ea08e8ca0c528d before the shake and 0x2f5e4ba6e0a6569b with it; the spin (Ben turned 0.8 rad) made it
        // 0x96988a931f7790d3. The other ticks are not death frames and kept theirs.
        //
        // Tick 700 has a stunned chicken in view, so its digest includes the stun stars (it was 0xdda9c10396315bd2 before them).
        //
        // Tick 865 is 12 ticks after the cluck at 853. Its chicken is off screen below the frame, so no caption is drawn and the
        // digest is the old one again (it was pinned to a clamped caption once). `without_clucks` draws the world as it was drawn before, and its
        // digests are the old ones at every tick, which shows the change is the cluck's pixels and nothing else.
        let mut seen = Vec::new();
        let mut old = Vec::new();
        play(|w| {
            if [0, 150, 270, 400, 602, 700, 865, 1000, 1268, 1450, 1681].contains(&w.tick.0) {
                seen.push((w.tick.0, fnv(&picture(w, w.tick.0 as u32))));
                old.push((w.tick.0, fnv(&picture(&without_clucks(w), w.tick.0 as u32))));
            }
        });
        assert_eq!(
            old,
            [
                (0, 0x8f5d1d703632de3e),
                (150, 0xc1960f3dc04ee373),
                (270, 0xe8f1fafe4a5f62b2),
                (400, 0x96988a931f7790d3),
                (602, 0x340d5091addc7678),
                (700, 0xabc05a7a4a86e0ae),
                (865, 0x0364355a21d6a202),
                (1000, 0x7ef3a65746089bbc),
                (1268, 0xf41efb06e7f6764b),
                (1450, 0xf49aaad3a6e0110b),
                (1681, 0xda9c59c9720ad45d),
            ]
        );
        assert_eq!(
            seen,
            [
                (0, 0x8f5d1d703632de3e),
                (150, 0xc1960f3dc04ee373),
                // Ben is running on the ground at 270, 602 and 865, so these lock the run-frame rule.
                (270, 0xe8f1fafe4a5f62b2),
                (400, 0x96988a931f7790d3),
                (602, 0x340d5091addc7678),
                (700, 0xabc05a7a4a86e0ae),
                (865, 0x0364355a21d6a202),
                (1000, 0x7ef3a65746089bbc),
                (1268, 0xf41efb06e7f6764b),
                (1450, 0xf49aaad3a6e0110b),
                (1681, 0xda9c59c9720ad45d),
            ]
        );
    }

    /// The world as it looked before chickens had a cluck to show: every chicken has never clucked.
    /// Plays the slice script as a presenter does, with a puff list fed after every step, and calls `each` with the world and the list.
    fn play_puffs(mut each: impl FnMut(&World, &Puffs)) {
        let mut w = level::world();
        let mut puffs = Puffs::new();
        let mut prev = Buttons::NONE;
        for &(frames, now) in SLICE_SCRIPT {
            for _ in 0..frames {
                step(&mut w, InputFrame::from_transition(prev, now));
                prev = now;
                puffs.observe(&w);
                each(&w, &puffs);
            }
        }
    }

    fn puff_picture(world: &World, puffs: &Puffs) -> Vec<u8> {
        let mut f = vec![0xEE; VIEW_W * VIEW_H];
        draw_slice_puffs(
            &mut f,
            VIEW_W,
            VIEW_H,
            world,
            world.tick.0 as u32,
            &DrawOptions::default(),
            puffs,
        );
        f
    }

    #[test]
    fn puff_frames_cover_eighteen_ticks_in_four_steps() {
        assert_eq!(PUFF_TICKS, 18);
        let frames: Vec<Option<usize>> = (0..20).map(puff_frame).collect();
        assert_eq!(&frames[..6], &[Some(0); 6]);
        assert_eq!(&frames[6..12], &[Some(1); 6]);
        assert_eq!(&frames[12..15], &[Some(2); 3]);
        assert_eq!(&frames[15..18], &[Some(3); 3]);
        assert_eq!(&frames[18..], &[None, None]);
        for name in PUFF_NAMES {
            assert!(sprite(name).is_some(), "{name}");
        }
    }

    #[test]
    fn a_puff_lives_from_the_event_for_eighteen_ticks_and_only_changes_its_own_pixels() {
        // Each of the script's thirteen shot ends gives a puff for ticks e..e+17 and no other tick.
        let ends = [
            96u32, 106, 281, 1132, 1142, 1152, 1162, 1172, 1203, 1213, 1223, 1233, 1243,
        ];
        let mut seen = [0u32; 13];
        let mut blank = Vec::new();
        play_puffs(|w, puffs| {
            let t = w.tick.0 as u32;
            let n = puffs.live(t).count();
            let expect = ends
                .iter()
                .filter(|&&e| (e..e + PUFF_TICKS).contains(&t))
                .count();
            assert_eq!(n, expect, "tick {t}");
            for (i, &e) in ends.iter().enumerate() {
                if (e..e + PUFF_TICKS).contains(&t) {
                    seen[i] += 1;
                }
            }
            if n > 0 {
                let with = puff_picture(w, puffs);
                let without = picture(w, t);
                let diff = with.iter().zip(&without).filter(|(a, b)| a != b).count();
                // A puff is at most 15 by 15 and only the magenta and light magenta of the puff may differ.
                assert!(diff <= 15 * 15 * n, "tick {t}: {diff} pixels differ");
                assert!(
                    with.iter()
                        .zip(&without)
                        .all(|(a, b)| a == b || *a == MAGENTA || *a == FIZZ_INDEX),
                    "tick {t}"
                );
                if diff == 0 {
                    blank.push(t);
                }
            }
        });
        assert_eq!(seen, [PUFF_TICKS; 13]);
        // The puffs that draw nothing: the first (a shot fired straight up, ending above the frame) is off screen, and the third
        // ends where its first three ticks leave no puff pixel in the frame (the picture is identical with and without the puff).
        let off_screen = 96..106;
        let hidden = 285..288;
        let expect: Vec<u32> = off_screen.chain(hidden).collect();
        assert_eq!(blank, expect);
    }

    #[test]
    fn the_puff_list_is_fixed_and_replaces_the_oldest() {
        let mut w = level::world();
        let mut puffs = Puffs::new();
        for i in 0..(MAX_PUFFS as i32 + 3) {
            w.tick = fizz_sim::Tick(100 + i as u64);
            w.shots.clear();
            w.shots.push(fizz_ep2::Shot {
                x: 10.0 + f64::from(i),
                y: 11.0,
                vx: 0.0,
                vy: 0.0,
                life: 0.0,
            });
            step(
                &mut w,
                InputFrame::from_transition(Buttons::NONE, Buttons::NONE),
            );
            puffs.observe(&w);
        }
        let now = w.tick.0 as u32;
        let xs: Vec<i32> = puffs.live(now).map(|(x, _, _)| x).collect();
        assert_eq!(xs.len(), MAX_PUFFS);
        assert!(!xs.contains(&160) && xs.contains(&(160 + 16 * (MAX_PUFFS as i32 + 2))));
        puffs.clear();
        assert_eq!(puffs.live(now).count(), 0);
    }

    #[test]
    fn frames_with_puffs_are_pinned() {
        // Re-pinned for the shot-end puff: the ticks the DOS bench samples with a puff on screen, one for each baked frame. The
        // second shot ends at 106, so 110 is its `puff0` (age 4), 118 its `puff2` (age 12) and 122 its dither `puff3` (age 16); the
        // third ends at 281, so 290 is its `puff1` (age 9). The first shot ends above the frame, so nothing of it shows. The
        // pictures at the other pinned ticks (and tick 100) are unchanged, because no visible puff is alive there.
        let mut seen = Vec::new();
        play_puffs(|w, puffs| {
            if [100, 110, 118, 122, 290].contains(&w.tick.0) {
                seen.push((w.tick.0, fnv(&puff_picture(w, puffs))));
            }
        });
        assert_eq!(
            seen,
            [
                (100, 0xf455e3ec0429f12e),
                (110, 0x4ec8aed63a9af639),
                (118, 0x6b8f9003ea2782a7),
                (122, 0xfcff42654786ba63),
                (290, 0xf39396a2c98efadb)
            ]
        );
    }

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
            &DrawOptions {
                captions,
                ..DrawOptions::default()
            },
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
        let off = DrawOptions {
            captions: false,
            ..DrawOptions::default()
        };
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
                    let max_w =
                        fizz_view::pfont::text_width(CAPTIONS[usize::from(v)].0) as usize + 1;
                    assert!(
                        x1 - x0 < max_w && y1 - y0 < fizz_view::pfont::ROWS + 1,
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
                            assert!(
                                (0..=map_px_w - vw as i32).contains(&cx),
                                "{vw}x{vh} ben {x},{y} look {off}"
                            );
                        } else {
                            assert_eq!(
                                cx,
                                (map_px_w - vw as i32).div_euclid(2),
                                "{vw}x{vh} ben {x},{y} look {off}"
                            );
                        }
                        assert!(
                            (0..=map_px_h - vh as i32).contains(&cy),
                            "{vw}x{vh} ben {x},{y} look {off}"
                        );
                        // Ben's body, in map pixels from the top and left, lies inside the view (the look offset only
                        // moves the camera as far as the map allows, and 3.6 tiles of look leaves him in a 208 view).
                        let left = tile_px(w.ben.b.x);
                        let right = tile_px(w.ben.b.x + w.ben.b.w);
                        let top = screen_y(w.ben.b.y + w.ben.b.h, w.map.h);
                        let bottom = screen_y(w.ben.b.y, w.map.h);
                        assert!(
                            left >= cx && right <= cx + vw as i32,
                            "{vw}x{vh} ben {x},{y} look {off}"
                        );
                        assert!(
                            top >= cy && bottom <= cy + vh as i32,
                            "{vw}x{vh} ben {x},{y} look {off}"
                        );
                        // Drawing at this size neither panics nor leaves the buffer unwritten.
                        let mut f = vec![0xEE; vw * vh];
                        draw_slice(&mut f, vw, vh, &w, 0);
                        assert!(
                            f.iter().all(|&p| p != 0xEE),
                            "{vw}x{vh} ben {x},{y} look {off}"
                        );
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
                ("stand", 0x1deb726ac325be49),
                ("look up", 0xab88c020fd4aef5c),
                ("look down", 0xc91cd6e208d07cd4),
                ("aim level", 0x6b2556fe3f543057),
                ("aim up", 0x5b77750264e3bb31),
                ("aim down", 0xdbd6e404040063b5),
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

    /// The pixels of a door's 16 by 16 front face for the tile (`tx`, `ty`) as the picture `f` of `w` shows them.
    fn door_front(w: &World, f: &[u8], tx: i32, ty: i32) -> Vec<u8> {
        let (cam_x, cam_y) = camera(w, VIEW_W, VIEW_H);
        let left = tx * TILE_PX - oblique::FEET_DX - cam_x;
        let top = (w.map.h - 1 - ty) * TILE_PX + oblique::STRIP - oblique::BACK - cam_y;
        assert!(
            left >= 0
                && left + TILE_PX <= VIEW_W as i32
                && top >= 0
                && top + TILE_PX <= VIEW_H as i32,
            "door ({tx}, {ty}) is in view"
        );
        let mut out = Vec::new();
        for y in 0..16usize {
            let row = (top as usize + y) * VIEW_W + left as usize;
            out.extend_from_slice(&f[row..row + 16]);
        }
        out
    }

    #[test]
    fn the_door_is_drawn_closed_and_then_open() {
        let mut w = level::world();
        w.ben.b.x = 13.0;
        w.ben.b.y = 20.0;
        let closed = picture(&w, 0);
        // The red door is three tiles, (15, 20) to (15, 22): bottom, middle and top, with the badge on the top.
        for (ty, piece) in [
            (22, &oblique_data::DOOR_RED_TOP),
            (21, &oblique_data::DOOR_RED_MID),
            (20, &oblique_data::DOOR_RED_BOT),
        ] {
            assert_eq!(door_front(&w, &closed, 15, ty), piece.pixels, "row {ty}");
        }
        w.map.set_switch(fizz_ep2::tiles::DOOR_RED_CHANNEL, false);
        w.flags.set(WorldFlags::KEY_RED_USED);
        let open = picture(&w, 0);
        for (ty, piece) in [
            (22, &oblique_data::DOOR_RED_TOP),
            (21, &oblique_data::DOOR_RED_MID),
            (20, &oblique_data::DOOR_RED_BOT),
        ] {
            let front = door_front(&w, &open, 15, ty);
            // An open door is not drawn: the back wall shows through (with the side face of the brick above it), so
            // little of the door's art remains.
            let same = front
                .iter()
                .zip(piece.pixels)
                .filter(|(a, b)| a == b)
                .count();
            assert!(
                same < 100,
                "row {ty}: {same} of 256 pixels are still the door's"
            );
        }
        assert_ne!(closed, open);
    }

    #[test]
    fn the_nook_draws_its_snacks_key_and_door_and_hides_what_is_taken() {
        let mut w = level::world();
        w.ben.b.x = 26.0;
        w.ben.b.y = 38.0;
        let full = picture(&w, 0);
        // The blue door is drawn closed, and a taken snack or key leaves the picture.
        for (ty, piece) in [
            (40, &oblique_data::DOOR_BLUE_TOP),
            (39, &oblique_data::DOOR_BLUE_MID),
            (38, &oblique_data::DOOR_BLUE_BOT),
        ] {
            assert_eq!(door_front(&w, &full, 22, ty), piece.pixels, "row {ty}");
        }
        // The elixir lies on the second ledge of the left shaft, out of the nook's view; its own test draws it.
        for i in (0..w.snacks.len()).filter(|&i| w.snacks[i].kind != SnackKind::Elixir) {
            let mut one = w.clone();
            one.snacks[i].taken = true;
            assert_ne!(picture(&one, 0), full, "snack {i} is drawn");
        }
        let mut no_key = w.clone();
        no_key.flags.set(WorldFlags::KEY_BLUE);
        assert_ne!(picture(&no_key, 0), full, "the blue key is drawn");
        let mut open = w.clone();
        open.map
            .set_switch(fizz_ep2::tiles::DOOR_BLUE_CHANNEL, false);
        open.flags.set(WorldFlags::KEY_BLUE_USED);
        let opened = picture(&open, 0);
        for ty in 38..=40 {
            let front = door_front(&open, &opened, 22, ty);
            assert!(
                front.iter().all(|&p| p != LIGHT_BLUE),
                "the open door has no light blue, row {ty}"
            );
        }
        assert_ne!(opened, full);
    }

    #[test]
    fn the_elixir_is_drawn_where_it_lies_and_leaves_the_picture_when_taken() {
        // The elixir (provisional) lies on the second ledge of the left shaft, in view from the start.
        let mut w = level::world();
        let elixir = w
            .snacks
            .iter()
            .position(|s| s.kind == SnackKind::Elixir)
            .unwrap();
        let shown = picture(&w, 0);
        w.snacks[elixir].taken = true;
        let taken = picture(&w, 0);
        assert_ne!(shown, taken, "the elixir is drawn");
        // Its light magenta glass is on screen.
        let bottle = sprite("elixir").unwrap();
        assert!(bottle.pixels.contains(&13), "light magenta fizz");
    }

    #[test]
    fn any_number_of_doors_in_view_are_all_drawn() {
        // Thirty door tiles in view, in columns of five: each column is a top, three middles and a bottom.
        let mut w = level::world();
        w.ben.b.x = 13.0;
        w.ben.b.y = 20.0;
        let (bx, by) = (w.ben.b.x as i32, w.ben.b.y as i32);
        let tiles: Vec<(i32, i32)> = (4..10)
            .flat_map(|dx| (-2..3).map(move |dy| (bx + dx, by + dy)))
            .collect();
        assert_eq!(tiles.len(), 30);
        for &(tx, ty) in &tiles {
            w.map.set(tx, ty, DOOR_R);
        }
        let f = picture(&w, 0);
        for &(tx, ty) in &tiles {
            let piece = if ty == by + 2 {
                &oblique_data::DOOR_RED_TOP
            } else if ty == by - 2 {
                &oblique_data::DOOR_RED_BOT
            } else {
                &oblique_data::DOOR_RED_MID
            };
            assert_eq!(
                door_front(&w, &f, tx, ty),
                piece.pixels,
                "door ({tx}, {ty})"
            );
        }
    }

    #[test]
    fn the_tile_types_look_different() {
        // A small room of one tile type, drawn the same way each time: no two types give the same picture.
        let types = [
            (WALLBG, "wall"),
            (FILL, "fill"),
            (BLOCK, "block"),
            (PLAT, "plank"),
            (SPIKE, "spikes"),
            (EXIT, "exit"),
            (DOOR_R, "red door"),
            (DOOR_B, "blue door"),
        ];
        let pictures: Vec<Vec<u8>> = types
            .iter()
            .map(|&(t, _)| {
                let mut data = vec![WALLBG; 25];
                for y in 1..4 {
                    for x in 1..4 {
                        data[y * 5 + x] = t;
                    }
                }
                let g = oblique::Ground {
                    data: &data,
                    w: 5,
                    h: 5,
                    red_closed: true,
                    blue_closed: true,
                };
                let mut f = vec![0xEEu8; 80 * 80];
                oblique::draw_ground(&mut f, 80, 80, &g, 0, 0);
                assert!(f.iter().all(|&p| p <= 15));
                f
            })
            .collect();
        for (i, a) in pictures.iter().enumerate() {
            for (j, b) in pictures.iter().enumerate().skip(i + 1) {
                assert_ne!(a, b, "{} and {} look alike", types[i].1, types[j].1);
            }
        }
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

    /// Frame with the first chicken in view and stunned for `stun` seconds (0 means not stunned), at animation tick `tick`.
    fn stun_picture(stun: f64, tick: u32) -> Vec<u8> {
        let mut w = level::world();
        w.ben.b.x = 8.0;
        w.ben.b.y = 11.0;
        w.chickens[0].stun = stun;
        picture_with(&w, tick, false)
    }

    #[test]
    fn stun_stars_are_one_sprite_above_the_head_and_swap_every_ten_ticks() {
        assert_eq!(STAR_FRAME_TICKS, 10);
        // The stars are the only difference between a stunned chicken and the same chicken with the stars sprite blanked.
        let stars = |f| sprite(f).unwrap();
        let (a, b) = (stars("stars0"), stars("stars1"));
        assert_eq!((a.w, a.h, b.w, b.h), (16, 8, 16, 8));
        assert_ne!(a.pixels, b.pixels);
        // Compare only the stars' box: the key's bob changes the rest of the picture at tick 16.
        let f0 = stun_picture(3.0, 0);
        let f1 = stun_picture(3.0, STAR_FRAME_TICKS as u32);
        let (x0, y0, x1, y1) = changed_box(&f0, &f1).expect("the frame changes after ten ticks");
        assert!(
            x1 - x0 < 16 && y1 - y0 < 8,
            "differ only inside a 16 by 8 box"
        );
        let crop = |f: &[u8]| -> Vec<u8> {
            (y0..=y1)
                .flat_map(|y| f[y * VIEW_W + x0..=y * VIEW_W + x1].to_vec())
                .collect()
        };
        let (c0, c1) = (crop(&f0), crop(&f1));
        // Ticks 0 to 9 show frame 0, 10 to 19 frame 1, 20 to 29 frame 0 again; stun 3.0 never flashes.
        for t in 0..10 {
            assert_eq!(crop(&stun_picture(3.0, t)), c0, "tick {t}");
        }
        for t in 10..20 {
            assert_eq!(crop(&stun_picture(3.0, t)), c1, "tick {t}");
        }
        assert_eq!(crop(&stun_picture(3.0, 20)), c0);
        // Not stunned: no stars, so the frame does not depend on the tick at the swap.
        assert_eq!(stun_picture(0.0, 0), stun_picture(0.0, 10));
    }

    #[test]
    fn stun_stars_sit_above_the_sunk_head_and_follow_the_chicken() {
        let mut w = level::world();
        w.ben.b.x = 8.0;
        w.ben.b.y = 11.0;
        w.chickens[0].stun = 3.0;
        let stunned = picture_with(&w, 0, false);
        // Frames 0 and 10 differ in the stars alone, so their changed box is the stars.
        let other = picture_with(&w, STAR_FRAME_TICKS as u32, false);
        let (x0, y0, x1, y1) = changed_box(&stunned, &other).unwrap();
        assert!(y1 < y0 + 8 && x1 < x0 + 16);
        // The stars end one pixel above the sprite's top, which is 24 rows above the feet less the 2 pixel sink.
        let c = &w.chickens[0];
        let cam = camera(&w, VIEW_W, VIEW_H);
        let feet_y = screen_y(c.b.y, w.map.h) - cam.1;
        let head = feet_y + 2 - 24;
        assert!(
            (y1 as i32) < head,
            "stars end above the head ({y1} vs {head})"
        );
        assert!((y1 as i32) >= head - 8 - 2, "and not far above it");
        // Moving the chicken a tile right moves the stars a tile right.
        let mut w2 = w.clone();
        w2.chickens[0].b.x += 1.0;
        let a = changed_box(
            &picture_with(&w2, 0, false),
            &picture_with(&w2, STAR_FRAME_TICKS as u32, false),
        )
        .unwrap();
        assert_eq!((a.0 as i32 - x0 as i32, a.1, a.3), (16, y0, y1));
    }

    #[test]
    fn stun_stars_flash_away_with_the_body_in_the_last_second() {
        // stun 0.5: hidden on the ticks where the body is hidden (phase / 4 odd), stars and all.
        let shown = stun_picture(0.5, 0);
        let hidden = stun_picture(0.5, 4);
        let mut w = level::world();
        w.ben.b.x = 8.0;
        w.ben.b.y = 11.0;
        w.chickens.clear();
        assert_eq!(
            hidden,
            picture_with(&w, 4, false),
            "no chicken, no stars while flashed away"
        );
        assert_ne!(shown, hidden);
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

    /// A world with Ben dead for `n` ticks, by the rules' own bookkeeping: `kill`, then `n` ticks of the dead branch.
    fn dead_for(world: &World, n: usize) -> World {
        let mut w = world.clone();
        w.ben.kill();
        let (mut ammo, mut shots) = (0, Vec::new());
        for _ in 0..n {
            w.ben
                .tick(&w.map, &[], InputFrame::default(), &mut ammo, &mut shots);
        }
        w
    }

    fn shaken(w: &World, shake: bool) -> Vec<u8> {
        let mut f = vec![0xEE; VIEW_W * VIEW_H];
        let o = DrawOptions {
            captions: true,
            shake,
            ..DrawOptions::default()
        };
        draw_slice_with(&mut f, VIEW_W, VIEW_H, w, 0, &o);
        f
    }

    #[test]
    fn the_death_shake_is_zero_when_alive_and_dies_away_in_18_ticks() {
        let w = level::world();
        assert_eq!(death_shake_x(&w), 0);
        let mut moved = 0;
        for n in 0..40 {
            let x = death_shake_x(&dead_for(&w, n));
            assert!(x.abs() <= 2, "tick {n}: {x}");
            if n >= DEATH_SHAKE_TICKS {
                assert_eq!(x, 0, "tick {n}");
            }
            moved += i32::from(x != 0);
        }
        assert!(moved >= 5, "it does shake: {moved} ticks");
        // After the respawn Ben is alive again and the view is still.
        let mut back = dead_for(&w, 10);
        back.ben.dead = 0.0;
        assert_eq!(death_shake_x(&back), 0);
    }

    #[test]
    fn motion_off_is_the_still_flat_picture_and_alive_frames_never_differ() {
        let mut w = level::world();
        w.ben.b.x = 14.0;
        w.ben.b.y = 3.0;
        assert!(DrawOptions::default().shake);
        assert_eq!(shaken(&w, true), shaken(&w, false), "alive");
        // Dead, Full draws the spin as well as the shake, so every death frame differs from Reduced's, even where the
        // shake is zero (tick 0 alone is the same pose, as the spin has not begun, but the shake is not zero there either).
        for n in 1..30 {
            let d = dead_for(&w, n);
            assert_ne!(shaken(&d, true), shaken(&d, false), "tick {n}");
        }
        // A tick with an offset does move the picture.
        let d = dead_for(&w, 1);
        assert_ne!(death_shake_x(&d), 0);
    }

    /// The `(cos, sin)` table is Fizz 1's `rot = 8 * n / 60`, rounded to 12 bits: every entry checked against `f64`.
    #[test]
    fn the_spin_table_is_eight_radians_a_second() {
        let w = level::world();
        assert_eq!(death_spin(&w), None, "alive Ben does not spin");
        assert_eq!(DEATH_SPIN.len(), DEATH_SPIN_TICKS + 1);
        let one = f64::from(1 << SPIN_BITS);
        for (n, &(c, s)) in DEATH_SPIN.iter().enumerate() {
            let a = 8.0 * n as f64 / f64::from(TICKS_PER_SECOND);
            assert!((f64::from(c) - a.cos() * one).abs() <= 0.5, "cos {n}");
            assert!((f64::from(s) - a.sin() * one).abs() <= 0.5, "sin {n}");
        }
        // Through the rules' own bookkeeping: n ticks dead is the nth entry, from the death to the last tick before the respawn.
        for (n, &want) in DEATH_SPIN.iter().enumerate().take(85) {
            let d = dead_for(&w, n);
            assert_eq!(death_spin(&d), Some(want), "tick {n}");
        }
        assert_eq!(DEATH_SPIN[0], (4096, 0));
        // A whole turn takes 2 pi / 8 = 0.785 s, about 47 ticks: the table is back near its start there.
        let (c, s) = DEATH_SPIN[47];
        assert!(c > 4000 && s.abs() < 300, "{c} {s}");
        // Past the table (not reachable in play) the last entry holds.
        let mut far = w.clone();
        far.ben.dead = 3.0;
        assert_eq!(death_spin(&far), Some(DEATH_SPIN[DEATH_SPIN_TICKS]));
    }

    fn jump() -> &'static fizz_view::sprites_data::Sprite {
        sprite("ben_jump").unwrap()
    }

    #[test]
    fn a_turn_of_zero_is_the_plain_blit_and_a_quarter_turn_is_exact() {
        let s = jump();
        for flip in [false, true] {
            let (mut a, mut b) = (vec![0xEEu8; 40 * 40], vec![0xEEu8; 40 * 40]);
            fizz_view::blit_sprite(&mut a, 40, 40, s, 20, 30, flip);
            blit_sprite_turned(&mut b, 40, 40, s, 20, 30, flip, 4096, 0, 0);
            assert_eq!(a, b, "flip {flip}");
        }
        // A quarter turn counter-clockwise turns the 16 by 24 box into a 24 by 16 one about the same middle: the top of the
        // picture becomes the left and the left edge the bottom. Box top left (12, 6), middle (20, 18), turned box x 8..32, y 10..26.
        let mut q = vec![0xEEu8; 40 * 40];
        blit_sprite_turned(&mut q, 40, 40, s, 20, 30, false, 0, 4096, 0);
        let sw = 16usize;
        let mut ink = 0;
        for y in 0..40usize {
            for x in 0..40usize {
                let want = if (8..32).contains(&x) && (10..26).contains(&y) {
                    // Turned row r, column c comes from source row c and column sw - 1 - r.
                    let (r, c) = (y - 10, x - 8);
                    match s.pixels[c * sw + (sw - 1 - r)] {
                        fizz_view::TRANSPARENT => 0xEE,
                        p => p,
                    }
                } else {
                    0xEE
                };
                ink += usize::from(want != 0xEE);
                assert_eq!(q[y * 40 + x], want, "({x}, {y})");
            }
        }
        assert!(ink > 50, "Ben is drawn: {ink}");
    }

    #[test]
    fn a_turned_sprite_has_no_new_colours_and_stays_inside_the_frame() {
        let s = jump();
        let mut own: Vec<u8> = s
            .pixels
            .iter()
            .copied()
            .filter(|&p| p != fizz_view::TRANSPARENT)
            .collect();
        own.sort_unstable();
        own.dedup();
        // Ben at every third turn and at and past the edges of a 40 by 30 frame, which is the front of a larger buffer.
        let (fw, fh) = (40usize, 30usize);
        for &(cos, sin) in DEATH_SPIN.iter().step_by(3) {
            for (x, y) in [
                (20, 20),
                (0, 0),
                (-30, 15),
                (70, 15),
                (20, -40),
                (20, 70),
                (39, 29),
                (3, 27),
            ] {
                for flip in [false, true] {
                    let mut buf = vec![0xEEu8; 60 * 60];
                    blit_sprite_turned(&mut buf[..fw * fh], fw, fh, s, x, y, flip, cos, sin, 0);
                    assert!(buf[fw * fh..].iter().all(|&p| p == 0xEE), "outside");
                    assert!(
                        buf.iter().all(|&p| p == 0xEE || own.contains(&p)),
                        "{cos} {sin} {x} {y}"
                    );
                }
            }
        }
        // It really is drawn when on screen, with about as many pixels as the sprite has.
        let mut frame = vec![0xEEu8; 40 * 40];
        let (c, sn) = DEATH_SPIN[20];
        blit_sprite_turned(&mut frame, 40, 40, s, 20, 30, false, c, sn, 0);
        let ink = frame.iter().filter(|&&p| p != 0xEE).count();
        let src = s
            .pixels
            .iter()
            .filter(|&&p| p != fizz_view::TRANSPARENT)
            .count();
        assert!(ink.abs_diff(src) * 4 < src, "{ink} against {src}");
    }

    #[test]
    fn full_motion_spins_ben_and_reduced_keeps_the_flat_flickering_pose() {
        let mut w = level::world();
        w.ben.b.x = 14.0;
        w.ben.b.y = 3.0;
        // Tick 30 is past the shake. Reduced is the flat, flipped pose and flickers; Full is turned and does not.
        let d = dead_for(&w, 30);
        let draw = |o: &DrawOptions, tick| {
            let mut f = vec![0xEEu8; VIEW_W * VIEW_H];
            draw_slice_with(&mut f, VIEW_W, VIEW_H, &d, tick, o);
            f
        };
        let reduced = DrawOptions {
            captions: true,
            shake: false,
            ..DrawOptions::default()
        };
        let full = DrawOptions::default();
        assert_ne!(draw(&reduced, 0), draw(&reduced, 3), "reduced flickers");
        assert_ne!(draw(&full, 0), draw(&reduced, 0), "full is turned");
        assert_eq!(draw(&full, 0), draw(&full, 3), "the spin does not flicker");
        // Ben moving a tile moves the turned picture: it is placed from the camera like any sprite.
        let mut e = d.clone();
        e.ben.b.x += 1.0;
        let mut g = vec![0xEEu8; VIEW_W * VIEW_H];
        draw_slice_with(&mut g, VIEW_W, VIEW_H, &e, 0, &full);
        assert_ne!(draw(&full, 0), g);
        // An invulnerable, living Ben still blinks and does not turn.
        let mut alive = w.clone();
        alive.ben.inv = 1.0;
        assert_ne!(picture(&alive, 0), picture(&alive, 3));
    }

    #[test]
    fn the_shaken_camera_still_clamps_to_the_map() {
        let w = level::world();
        let map_px = w.map.w * TILE_PX;
        for (x, y) in [(0.0, 3.0), (w.map.w as f64 - 2.0, 3.0), (10.0, 3.0)] {
            for n in 0..DEATH_SHAKE_TICKS {
                let mut d = dead_for(&w, n);
                d.ben.b.x = x;
                d.ben.b.y = y;
                let (cx, _) = camera_shaken(&d, VIEW_W, VIEW_H, &DrawOptions::default());
                assert!(
                    (0..=map_px - VIEW_W as i32).contains(&cx),
                    "{x} tick {n}: {cx}"
                );
                assert!((cx - camera(&d, VIEW_W, VIEW_H).0).abs() <= 2);
            }
        }
    }

    #[test]
    fn bens_view_box_is_his_sprite_at_his_feet_in_the_view_and_follows_the_view_size() {
        let mut w = level::world();
        w.ben.b.x = 20.0;
        w.ben.b.y = 30.0;
        let o = DrawOptions::default();
        let (x, y, bw, bh) = ben_view_box(&w, VIEW_W, VIEW_H, &o);
        assert_eq!((bw, bh), (16, 16));
        // Mid-map the camera centres on his body, so the box is on the middle of the view.
        let (cam_x, cam_y) = camera(&w, VIEW_W, VIEW_H);
        let feet_x = tile_px(w.ben.b.x + w.ben.b.w * 0.5) - cam_x;
        assert_eq!(x, feet_x - 8);
        assert_eq!(y, screen_y(w.ben.b.y, w.map.h) - cam_y - 16);
        assert!((0..VIEW_W as i32).contains(&x) && (0..VIEW_H as i32).contains(&y));
        // A wider view moves the camera, not the sprite's size.
        let wide = ben_view_box(&w, VIEW_W * 2, VIEW_H, &o);
        assert_eq!((wide.2, wide.3), (16, 16));
    }

    #[test]
    fn a_cluck_pans_toward_the_side_its_chicken_is_on_in_the_view_it_is_drawn_in() {
        let mut w = level::world();
        assert!(!w.chickens.is_empty());
        w.ben.b.x = 20.0;
        w.ben.b.y = 3.0;
        let (cam_x, _) = camera(&w, VIEW_W, VIEW_H);
        let tiles = |px: i32| f64::from(cam_x + px) / TILE_PX as f64;
        let o = DrawOptions::default();
        for (px, side) in [(20, -1), (160, 0), (300, 1)] {
            w.chickens[0].b.x = tiles(px) - w.chickens[0].b.w * 0.5;
            let p = cluck_placement(&w, 0, VIEW_W, VIEW_H, &o);
            assert_eq!(p.pan.signum(), side, "{px}: {p:?}");
            assert_eq!(side == 0, p.left == p.right, "{px}: {p:?}");
        }
        // A view twice as wide, centred on the same Ben, pans the same chicken less.
        let narrow = cluck_placement(&w, 0, VIEW_W, VIEW_H, &o);
        let wide = cluck_placement(&w, 0, VIEW_W * 2, VIEW_H, &o);
        assert!(wide.pan.abs() < narrow.pan.abs());
        // No such chicken: the centre.
        assert_eq!(
            cluck_placement(&w, 200, VIEW_W, VIEW_H, &o),
            fizz_sfx::place::Placement::CENTRE
        );
    }
}
#[cfg(test)]
mod night_tests;
#[cfg(test)]
mod wall_tests;
