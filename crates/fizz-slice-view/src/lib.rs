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
//!
//! # Choices that are ours
//!
//! - **Chicken.** The art faces right, so a chicken walking left is flipped. It walks (the `walk` and `idle` frames
//!   alternating with the distance it has travelled) and shows the `alert` frame while it touches Ben. A stunned chicken is
//!   shown as the `alert` frame sunk two pixels, so it reads as knocked flat, and it flashes (disappears every other four
//!   ticks) for its last second of stun to warn that it is about to wake.
//! - **Ben.** Run frames alternate with the distance he has travelled, so he never skates. The `shoot` pose shows while the
//!   rules' shooting pose runs, the squashed `pogo2` frame during the squash after a bounce and `pogo` otherwise on the
//!   stick, `jump` in the air and `stand` at rest. The sprites face right, so facing left flips them. While Ben is
//!   invulnerable he blinks.
//! - **Dead Ben.** Fizz 1 has no dead pose. A dead Ben is drawn with the `jump` pose, arms up, flipped away from his
//!   facing, and he flickers, so the toss and fall after a death read as a hurt tumble. The world respawns him
//!   afterwards.
//!
//! # The camera
//!
//! `fizz_view::follow` centres the view on the middle of Ben's body on both axes and clamps it to the map, in whole
//! logical pixels, so the map scrolls without sub-pixel shimmer. The map's y-axis is flipped with `fizz_view::screen_y`.

use fizz_ep2::World;
use fizz_ep2::tiles::{BLOCK, CHOC, DOOR_R, DOOR_RED_CHANNEL, EXIT, FILL, PLAT, SPIKE, WALLBG};
use fizz_view::sprites_data::sprite;
use fizz_view::{TILE_PX, blit_sprite, draw_tilemap, follow, screen_y, tile_px};

/// The width of the picture in pixels.
pub const VIEW_W: usize = 320;
/// The height of the picture in pixels.
pub const VIEW_H: usize = 200;

// EGA palette indices (the standard sixteen-colour order).
const BLUE: u8 = 1;
const BROWN: u8 = 6;
const LIGHT_GREY: u8 = 7;
const DARK_GREY: u8 = 8;
const LIGHT_BLUE: u8 = 9;
const LIGHT_GREEN: u8 = 10;
const LIGHT_RED: u8 = 12;
const YELLOW: u8 = 14;

/// The sky behind and above the level.
const SKY: u8 = LIGHT_BLUE;

/// The flat colour of a tile, or `None` for tiles that show the sky.
fn tile_colour(tile: u8) -> Option<u8> {
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

/// Draws `world` into `frame`, a `w` by `h` buffer of palette indices (use [`VIEW_W`] by [`VIEW_H`]).
///
/// `tick` drives animation only (the key's bob, blinking and flashing) and must not be fed back into the rules. Any frame
/// size is safe: the view is `w` by `h` and everything is clipped.
pub fn draw_slice(frame: &mut [u8], w: usize, h: usize, world: &World, tick: u32) {
    let rows = (frame.len().checked_div(w).unwrap_or(0)).min(h);
    let used = rows * w;
    frame[..used].fill(SKY);

    let map = &world.map;
    let (map_w, map_h) = (map.w, map.h);

    // The camera centres on the middle of Ben's body.
    let ben = &world.ben;
    let target_x = tile_px(ben.b.x + ben.b.w * 0.5);
    let target_y = screen_y(ben.b.y + ben.b.h * 0.5, map_h);
    let cam_x = follow(target_x, map_w.saturating_mul(TILE_PX), w as i32);
    let cam_y = follow(target_y, map_h.saturating_mul(TILE_PX), h as i32);

    let door_closed = map.switch(DOOR_RED_CHANNEL);
    draw_tilemap(frame, w, h, cam_x, cam_y, map_w, map_h, |tx, ty| {
        let t = map.get(tx, ty);
        if t == DOOR_R {
            return Some(if door_closed { BROWN } else { BLUE });
        }
        tile_colour(t)
    });

    // Sprites are placed by their anchor, which is feet and centre. `at` turns a world position into frame pixels.
    let at = |x: f64, y: f64| (tile_px(x) - cam_x, screen_y(y, map_h) - cam_y);
    let phase = tick as usize;

    if door_closed && let Some(door) = sprite("doorRed") {
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
    } else if c.b.on_ground && (tile_px(c.b.x) / 4) % 2 == 0 {
        ("cluckzarg_walk", 0)
    } else {
        ("cluckzarg_idle", 0)
    };
    let Some(s) = sprite(name) else {
        return;
    };
    let (sx, sy) = at(c.b.x + c.b.w * 0.5, c.b.y);
    blit_sprite(frame, w, h, s, sx, sy + sink, c.dir < 0.0);
}

fn draw_ben(frame: &mut [u8], w: usize, h: usize, world: &World, phase: usize, at: At) {
    let ben = &world.ben;
    let dead = ben.dead > 0.0;
    // Invulnerable Ben blinks; dead Ben flickers.
    if (ben.inv > 0.0 || dead) && phase / 3 % 2 == 1 {
        return;
    }
    let name = if dead {
        "ben_jump"
    } else if ben.shoot_t > 0.0 {
        "ben_shoot"
    } else if ben.pogo {
        if ben.squash > 0.0 {
            "ben_pogo2"
        } else {
            "ben_pogo"
        }
    } else if !ben.b.on_ground {
        "ben_jump"
    } else if ben.b.vx.abs() > 0.1 {
        if (tile_px(ben.b.x) / 6) % 2 == 0 {
            "ben_run1"
        } else {
            "ben_run2"
        }
    } else {
        "ben_stand"
    };
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
    use fizz_ep2::world::{WorldFlags, step};
    use fizz_ep2::{SLICE_SCRIPT, SLICE_TICKS};
    use fizz_sim::{Buttons, InputFrame};

    extern crate std;
    use std::vec;
    use std::vec::Vec;

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
        let mut seen = Vec::new();
        play(|w| {
            if [0, 150, 400, 700, 1000, 1268].contains(&w.tick.0) {
                seen.push((w.tick.0, fnv(&picture(w, w.tick.0 as u32))));
            }
        });
        assert_eq!(
            seen,
            [
                (0, 0x5b84b8874be44ba4),
                (150, 0x7abd20c31ae39a54),
                (400, 0xe88d2a8a227b4cfb),
                (700, 0xdda9c10396315bd2),
                (1000, 0x1881c1623a3ec20c),
                (1268, 0x23afa9ef954ff4e0),
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
