// Tests that a living actor is drawn stopping at the front line of a wall on its right, and left alone beside a wall on its left.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use fizz_ep2::level;
use fizz_ep2::tiles::{BLOCK, DOOR_R, EMPTY, FILL, PLAT, WALLBG};
use fizz_sim::Buttons;
use std::vec;
use std::vec::Vec;

const COLS: i32 = 8;
const ROWS: i32 = 6;
const W: usize = 128;

/// Night without the light pass: an actor's pixels carry the actor bank there, so every one of them differs from the ground
/// under it, which a day picture cannot promise (a pixel of Ben's can be the colour of the wall behind him).
const DAY: DrawOptions = DrawOptions {
    captions: false,
    shake: true,
    full_sky: false,
    night: true,
};

/// A room 8 by 6 cells with a stone floor, back wall behind, and `tile` in column `col` for the two rows above the floor.
fn room(tile: u8, col: i32, red_closed: bool) -> World {
    let mut world = level::world();
    let map = &mut world.map;
    map.w = COLS;
    map.h = ROWS;
    map.data = vec![0; (COLS * ROWS) as usize];
    map.switches = 0;
    for y in 0..ROWS {
        for x in 0..COLS {
            map.set(x, y, WALLBG);
        }
    }
    for x in 0..COLS {
        map.set(x, 0, FILL);
    }
    for y in 1..3 {
        map.set(col, y, tile);
    }
    map.set_switch(fizz_ep2::tiles::DOOR_RED_CHANNEL, red_closed);
    world.chickens.clear();
    world.snacks.clear();
    world.ben.b.y = 1.0;
    world
}

/// A pose of Ben: facing, and which sprite it picks.
#[derive(Clone, Copy, Debug)]
struct Pose {
    face: f64,
    pogo: bool,
    shoot: i8,
    air: bool,
    look_up: bool,
    run: bool,
}

const STAND: Pose = Pose {
    face: 1.0,
    pogo: false,
    shoot: -9,
    air: false,
    look_up: false,
    run: false,
};

fn poses() -> Vec<Pose> {
    let mut v = Vec::new();
    for face in [1.0, -1.0] {
        v.push(Pose { face, ..STAND });
        v.push(Pose {
            face,
            run: true,
            ..STAND
        });
        v.push(Pose {
            face,
            air: true,
            ..STAND
        });
        v.push(Pose {
            face,
            pogo: true,
            ..STAND
        });
        v.push(Pose {
            face,
            shoot: 0,
            ..STAND
        });
        v.push(Pose {
            face,
            shoot: 1,
            ..STAND
        });
        v.push(Pose {
            face,
            look_up: true,
            ..STAND
        });
    }
    v
}

fn set_pose(world: &mut World, p: Pose) {
    let ben = &mut world.ben;
    ben.face = p.face;
    ben.pogo = p.pogo;
    ben.shoot_t = if p.shoot == -9 { 0.0 } else { 0.5 };
    ben.aim = if p.shoot == -9 { 0 } else { p.shoot };
    ben.b.on_ground = !p.air;
    ben.look_up = if p.look_up { 0.5 } else { 0.0 };
    ben.b.vx = if p.run { 4.0 } else { 0.0 };
    // Invulnerable, so that the picture at tick 3 has no Ben in it and the one at tick 0 has: the difference is his pixels.
    ben.inv = 1.0;
}

/// The frame pixels Ben (or the actor that `hide` removes) draws: those that differ between tick 0 and tick 3, when an
/// invulnerable Ben blinks off. Returns the mask, `W` by 96.
fn ben_mask(world: &World) -> Vec<bool> {
    let draw = |tick| {
        let mut f = vec![0xEEu8; W * 96];
        draw_slice_with(&mut f, W, 96, world, tick, &DAY);
        f
    };
    let (a, b) = (draw(0), draw(3));
    a.iter().zip(&b).map(|(x, y)| x != y).collect()
}

/// The first column and one past the last column of a mask.
fn extent(mask: &[bool]) -> (i32, i32) {
    let mut cols = (0..W).filter(|&x| (0..96).any(|y| mask[y * W + x]));
    let first = cols.clone().next().expect("something was drawn") as i32;
    (first, cols.next_back().unwrap() as i32 + 1)
}

fn at(world: &mut World, x: f64, pose: Pose) -> Vec<bool> {
    set_pose(world, pose);
    world.ben.b.x = x;
    ben_mask(world)
}

/// Ben's body is `w` tiles wide, so a wall in column `c` on his right meets him with his left edge at `c - w`.
fn against_right(world: &World, c: i32) -> f64 {
    f64::from(c) - world.ben.b.w
}

#[test]
fn ben_stops_on_the_front_line_of_a_wall_on_his_right() {
    for tile in [FILL, BLOCK] {
        for pose in poses() {
            let mut world = room(tile, 5, false);
            let x = against_right(&world, 5);
            let mask = at(&mut world, x, pose);
            // The wall's front face starts at 16 * 5 - 4 = 76: Ben's opaque edge ends exactly there.
            assert_eq!(extent(&mask).1, 76, "{tile} {pose:?}");
        }
    }
}

#[test]
fn a_wall_on_his_left_changes_nothing() {
    // The wall's slanted side face is visible there, so Ben is drawn at his collision edge, as with no wall at all, in every
    // pose and at every distance from it.
    for pose in poses() {
        for px in 32..=48 {
            let x = f64::from(px) / 16.0;
            let (mut wall, mut free) = (room(BLOCK, 1, false), room(EMPTY, 1, false));
            assert!(
                at(&mut wall, x, pose) == at(&mut free, x, pose),
                "{pose:?} {px}"
            );
        }
    }
}

#[test]
fn a_shut_door_is_a_wall_and_an_open_one_is_not() {
    let mut shut = room(DOOR_R, 5, true);
    let x = against_right(&shut, 5);
    assert_eq!(extent(&at(&mut shut, x, STAND)).1, 76);
    // An open door is not drawn and Ben walks through it, so he is drawn where his body is.
    let mut open = room(DOOR_R, 5, false);
    let mut free = room(EMPTY, 5, false);
    assert!(at(&mut open, x, STAND) == at(&mut free, x, STAND));
}

#[test]
fn planks_and_walls_out_of_his_rows_move_nothing() {
    let mut plank = room(PLAT, 5, false);
    let mut free = room(EMPTY, 5, false);
    let x = against_right(&plank, 5);
    assert!(at(&mut plank, x, STAND) == at(&mut free, x, STAND));
    // A wall one row above his head is not beside him.
    let mut high = room(EMPTY, 5, false);
    for y in 4..5 {
        high.map.set(5, y, BLOCK);
    }
    assert!(at(&mut high, x, STAND) == at(&mut free, x, STAND));
}

/// The right edge of Ben's drawing as he walks up to a wall in column 5, a pixel at a time from `x0` tiles.
fn walk_up(x0: i32, wall: bool) -> Vec<(i32, Vec<bool>)> {
    let mut world = room(if wall { BLOCK } else { EMPTY }, 5, false);
    let end = (against_right(&world, 5) * 16.0).round() as i32;
    (x0..=end)
        .map(|px| {
            let m = at(&mut world, f64::from(px) / 16.0, STAND);
            (px, m)
        })
        .collect()
}

#[test]
fn walking_up_to_a_wall_is_smooth_and_far_from_it_nothing_moves() {
    let with = walk_up(24, true);
    let without = walk_up(24, false);
    let mut last = None;
    for ((px, a), (_, b)) in with.iter().zip(&without) {
        let edge = extent(a).1;
        let natural = extent(b).1;
        // Twelve pixels or more from contact the picture is the one with no wall at all.
        if *px <= with.last().unwrap().0 - 12 {
            assert!(a == b, "far from the wall at {px}");
        }
        // The edge never moves back and never jumps more than a pixel a pixel, and never passes the front line.
        if let Some(prev) = last {
            assert!((0..=1).contains(&(edge - prev)), "{px}: {prev} to {edge}");
        }
        assert!(edge <= natural && edge <= 76);
        last = Some(edge);
    }
}

#[test]
fn between_two_walls_only_the_right_one_counts() {
    // A shaft one cell wide, columns 1 and 3 solid: Ben is drawn as he is with only the wall on his right.
    for px in 32..=36 {
        let x = f64::from(px) / 16.0;
        let mut shaft = room(BLOCK, 1, false);
        let mut right_only = room(BLOCK, 3, false);
        for y in 1..3 {
            shaft.map.set(3, y, BLOCK);
        }
        assert!(
            at(&mut shaft, x, STAND) == at(&mut right_only, x, STAND),
            "{px}"
        );
    }
}

#[test]
fn dead_ben_is_drawn_where_his_body_is() {
    let mut world = room(BLOCK, 5, false);
    let mut free = room(EMPTY, 5, false);
    let x = against_right(&world, 5);
    for w in [&mut world, &mut free] {
        w.ben.dead = 0.4;
    }
    assert!(at(&mut world, x, STAND) == at(&mut free, x, STAND));
}

#[test]
fn it_holds_at_the_map_edge_in_wider_views_and_at_night() {
    // The wall is the last column, so the edge cell repeats beyond it; the contact line is the same at any view width.
    for w in [128usize, 160, 200] {
        let mut world = room(BLOCK, 7, false);
        let x = against_right(&world, 7);
        set_pose(&mut world, STAND);
        world.ben.b.x = x;
        let draw = |tick, options: &DrawOptions| {
            let mut f = vec![0xEEu8; w * 96];
            draw_slice_with(&mut f, w, 96, &world, tick, options);
            f
        };
        let night = DrawOptions {
            night: true,
            full_sky: false,
            ..DAY
        };
        for options in [DAY, night] {
            let (a, b) = (draw(0, &options), draw(3, &options));
            let cols: Vec<usize> = (0..w)
                .filter(|&c| (0..96).any(|r| a[r * w + c] != b[r * w + c]))
                .collect();
            let (_, cam) = (0, camera_shaken(&world, w, 96, &options));
            // The front line is 7 * 16 - 4 = 108 in the map, wherever the camera puts it.
            assert_eq!(*cols.last().unwrap() as i32 + 1 + cam.0, 108, "width {w}");
        }
    }
}

#[test]
fn enemies_are_drawn_where_their_bodies_are_at_any_wall() {
    // A chicken at a wall on either side, across contact and pixel boundaries, is drawn exactly as with no wall there: the draw
    // correction is Ben's alone.
    let mut keep = level::world();
    keep.chickens.truncate(1);
    let mask = |world: &mut World, x: f64, dir: f64| -> Vec<bool> {
        let c = &mut world.chickens[0];
        c.b.x = x;
        c.b.y = 1.0;
        c.dir = dir;
        c.stun = 0.0;
        world.ben.b.x = 0.5;
        world.ben.inv = 1.0;
        let mut f = vec![0xEEu8; W * 96];
        let mut g = f.clone();
        draw_slice_with(&mut f, W, 96, world, 0, &DAY);
        let saved = core::mem::take(&mut world.chickens);
        draw_slice_with(&mut g, W, 96, world, 0, &DAY);
        world.chickens = saved;
        f.iter().zip(&g).map(|(a, b)| a != b).collect()
    };
    for col in [1, 5] {
        let (mut wall, mut free) = (room(BLOCK, col, false), room(EMPTY, col, false));
        for w in [&mut wall, &mut free] {
            w.chickens = keep.chickens.clone();
        }
        for dir in [1.0, -1.0] {
            for step in 0..=120 {
                let x = 2.0 + f64::from(step) / 120.0 * 4.5;
                if (f64::from(col) - 0.9..f64::from(col) + 1.0).contains(&x) {
                    continue;
                }
                assert!(
                    mask(&mut wall, x, dir) == mask(&mut free, x, dir),
                    "{col} {x} {dir}"
                );
            }
            // Pressed into the wall on its right, with its x wobbling by a fraction of a pixel, it is drawn as its body is.
            if col == 5 {
                let contact = 5.0 - wall.chickens[0].b.w;
                for tick in 0..30 {
                    let x = contact - f64::from((tick * 7) % 13) / 13.0 * 0.8 / 16.0;
                    assert!(mask(&mut wall, x, dir) == mask(&mut free, x, dir));
                }
            }
        }
    }
}

#[test]
fn ben_does_not_shake_at_a_wall() {
    // His drawing is a function of his position alone: the same position gives the same picture, and moving into the wall a
    // sub-pixel step at a time his opaque right edge never moves back and never jumps more than a pixel.
    let mut world = room(BLOCK, 5, false);
    let contact = against_right(&world, 5);
    let mut last: Option<i32> = None;
    for step in 0..=64 {
        let x = contact - 1.0 + f64::from(step) / 64.0;
        let (a, b) = (at(&mut world, x, STAND), at(&mut world, x, STAND));
        assert!(a == b);
        let edge = extent(&a).1;
        if let Some(prev) = last {
            assert!((0..=1).contains(&(edge - prev)), "{x}: {prev} to {edge}");
        }
        last = Some(edge);
    }
    // Pressed into the wall (physics stops him at contact, so only contact and a hair short are real): the edge stays on the line.
    assert_eq!(last, Some(76));
}

#[test]
fn the_view_box_follows_the_offset() {
    let (mut wall, mut free) = (room(BLOCK, 5, false), room(EMPTY, 5, false));
    let x = against_right(&wall, 5);
    for w in [&mut wall, &mut free] {
        w.ben.b.x = x;
    }
    let (a, b) = (
        ben_view_box(&wall, W, 96, &DAY),
        ben_view_box(&free, W, 96, &DAY),
    );
    // Moved left by the offset, the same size and height; free of walls it is where the sprite is drawn.
    assert!((1..=6).contains(&(b.0 - a.0)));
    assert_eq!((a.1, a.2, a.3), (b.1, b.2, b.3));
    // And the box's right edge is the front line less what the sprite does not cover of its 16 px.
    assert!(a.0 + a.2 <= 76 + 3);
}

/// Plays `script` (buttons for each tick) in a room with a wall in column 5 and returns, for each tick, Ben's x and the right
/// edge of what he draws.
fn play_room(script: &[Buttons]) -> Vec<(f64, i32)> {
    let mut world = room(BLOCK, 5, false);
    for y in 3..ROWS {
        world.map.set(5, y, BLOCK);
    }
    world.ben.b.x = 2.0;
    let mut prev = Buttons::NONE;
    let mut out = Vec::new();
    for &now in script {
        fizz_ep2::step(&mut world, fizz_sim::InputFrame::from_transition(prev, now));
        prev = now;
        world.ben.inv = 1.0;
        let x = world.ben.b.x;
        let m = ben_mask_at(&world, world.tick.0 as u32);
        out.push((
            x,
            if m.iter().any(|&b| b) {
                extent(&m).1
            } else {
                0
            },
        ));
    }
    out
}

fn ben_mask_at(world: &World, tick: u32) -> Vec<bool> {
    let draw = |t| {
        let mut f = vec![0xEEu8; W * 96];
        draw_slice_with(&mut f, W, 96, world, t, &DAY);
        f
    };
    // The blink hides an invulnerable Ben on ticks whose third is odd: pick a tick that shows him and one that does not.
    let show = tick - tick % 6;
    let (a, b) = (draw(show), draw(show + 3));
    a.iter().zip(&b).map(|(x, y)| x != y).collect()
}

#[test]
fn ben_running_into_a_wall_and_tapping_back_does_not_shake() {
    // Held into the wall, the rules leave his x exactly where it was, so the same picture every tick. Tapping Left and Right
    // against it moves his body by under a pixel, and the edge stays on the front line throughout.
    let run: Vec<Buttons> = (0..120).map(|_| Buttons::RIGHT).collect();
    let r = play_room(&run);
    assert!(r[60..].iter().all(|&(x, e)| x == r[60].0 && e == 76));
    let tap: Vec<Buttons> = (0..160)
        .map(|i| match i % 7 {
            0..=3 => Buttons::RIGHT,
            4 => Buttons::LEFT,
            _ => Buttons::NONE,
        })
        .collect();
    let r = play_room(&tap);
    assert!(r[70..].iter().all(|&(_, e)| e == 76), "{:?}", &r[70..90]);
}
