// Tests of the slice drawn at night: pinned pictures, the markers through the light pass, sizes, and the light budget.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use fizz_ep2::{SLICE_SCRIPT, level, step};
use fizz_sim::{Buttons, InputFrame};
use std::vec;
use std::vec::Vec;

const NIGHT: DrawOptions = DrawOptions {
    captions: true,
    shake: true,
    full_sky: true,
    night: true,
};

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Plays the slice script, calling `each` with the world after every tick.
fn play(mut each: impl FnMut(&World)) {
    let mut w = level::world();
    let mut prev = Buttons::NONE;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            step(&mut w, InputFrame::from_transition(prev, now));
            prev = now;
            each(&w);
        }
    }
}

fn night_picture(world: &World, w: usize, h: usize, options: &DrawOptions) -> Vec<u8> {
    let mut f = vec![0xEEu8; w * h];
    draw_slice_with(&mut f, w, h, world, world.tick.0 as u32, options);
    f
}

#[test]
fn night_frames_are_pinned() {
    // Ben on the shaft's floor, by the lamp with the chicken, at the red door's lantern, in the dark between the shafts, in the
    // snack nook on the gallery (by day the worst frame on the floor), on the pogo stick with the beam on the sea, and at the exit;
    // with the Full detail (the light pass) and with Light (the DOS default: the palette's ambient and the glowing art only).
    const TICKS: [u64; 7] = [60, 300, 900, 1079, 1225, 1450, 1745];
    let light = DrawOptions {
        full_sky: false,
        ..NIGHT
    };
    let (mut full, mut lit) = (Vec::new(), Vec::new());
    play(|w| {
        if TICKS.contains(&w.tick.0) {
            full.push((w.tick.0, fnv(&night_picture(w, VIEW_W, VIEW_H, &NIGHT))));
            lit.push((w.tick.0, fnv(&night_picture(w, VIEW_W, VIEW_H, &light))));
        }
    });
    assert_eq!(full, PINNED_FULL, "full");
    assert_eq!(lit, PINNED_LIGHT, "light");
}

const PINNED_FULL: [(u64, u64); 7] = [
    (60, 0x3176_8ff7_4e02_5fb5),
    (300, 0x74c8_d534_a27a_d1b4),
    (900, 0x0a74_8c4c_22ae_0998),
    (1079, 0x2470_d45d_eb3d_6572),
    (1225, 0x5de1_dda1_3716_9a33),
    (1450, 0xd8c1_54e1_1566_ecf3),
    (1745, 0xc1f1_b22c_9729_7989),
];
const PINNED_LIGHT: [(u64, u64); 7] = [
    (60, 0x7f0_ce66_87cb_34d5),
    (300, 0x229d_8081_9dfe_9ee4),
    (900, 0x6d98_4c94_9454_2798),
    (1079, 0x4147_6295_cbc1_cf13),
    (1225, 0x8ebe_ddbe_141a_e6d5),
    (1450, 0xbec_52e7_1c45_b6cf),
    (1745, 0xa3bb_b9ab_034f_84e6),
];

#[test]
fn a_day_picture_is_the_sixteen_colours_and_night_changes_only_the_banks() {
    // By day no byte has a bank; at night every byte keeps the colour the day picture has there, except where the night
    // dressing (the lamps and lanterns) is drawn.
    let mut checked = 0;
    play(|w| {
        if w.tick.0 % 97 == 0 {
            let day = night_picture(w, VIEW_W, VIEW_H, &DrawOptions::default());
            let night = night_picture(w, VIEW_W, VIEW_H, &NIGHT);
            assert!(day.iter().all(|&p| p < 16), "tick {}", w.tick.0);
            let same = day
                .iter()
                .zip(&night)
                .filter(|&(d, n)| d & 15 == n & 15)
                .count();
            assert!(same * 100 >= day.len() * 98, "tick {}: {same}", w.tick.0);
            checked += 1;
        }
    });
    assert!(checked > 15);
}

#[test]
fn actors_never_go_below_level_9_and_glowing_pixels_stay_full() {
    // The picture with its markers, before the light pass, against the finished one, for every 10th tick of the script at two view
    // sizes: an actor's pixel ends at level 9 or above, a glowing one is untouched at full brightness, and every pixel keeps its colour.
    let mut actors = 0;
    let mut glows = 0;
    play(|w| {
        if w.tick.0 % 10 != 0 {
            return;
        }
        for (vw, vh) in [(VIEW_W, VIEW_H), (480, 208)] {
            let tick = w.tick.0 as u32;
            let mut marked = vec![0u8; vw * vh];
            draw_picture(
                &mut marked,
                vw,
                vh,
                w,
                tick,
                &NIGHT,
                &Puffs::new(),
                &EventCaptions::new(),
                false,
            );
            let mut lit = vec![0u8; vw * vh];
            draw_picture(
                &mut lit,
                vw,
                vh,
                w,
                tick,
                &NIGHT,
                &Puffs::new(),
                &EventCaptions::new(),
                true,
            );
            for (&m, &l) in marked.iter().zip(&lit) {
                assert_eq!(m & 15, l & 15);
                match m & 0xF0 {
                    light::ACTOR => {
                        actors += 1;
                        assert!(light::level_of(l, oblique::NIGHT_AMBIENT) >= light::ACTOR_FLOOR);
                    }
                    light::GLOW => {
                        glows += 1;
                        assert_eq!(l, m);
                    }
                    light::LIT => assert!(
                        light::level_of(l, oblique::NIGHT_AMBIENT) >= oblique::NIGHT_AMBIENT
                    ),
                    other => panic!("an unshaded picture holds bank {other:#x}"),
                }
            }
        }
    });
    assert!(actors > 10_000 && glows > 10_000, "{actors} {glows}");
}

#[test]
fn any_view_size_is_drawn_at_night_without_a_panic() {
    let mut worlds = Vec::new();
    play(|w| {
        if [1, 400, 1079, 1225, 1745].contains(&w.tick.0) {
            worlds.push(w.clone());
        }
    });
    for w in &worlds {
        for (vw, vh) in [
            (96, 64),
            (1280, 416),
            (1280, 208),
            (97, 65),
            (321, 199),
            (1, 1),
            (7, 300),
            (1300, 420),
            (2000, 900),
        ] {
            let f = night_picture(w, vw, vh, &NIGHT);
            assert!(f.iter().all(|&p| p != 0xEE), "{vw} by {vh} is drawn whole");
        }
    }
}

#[test]
fn the_lights_in_reach_of_any_view_fit_the_budget() {
    // Fizz 1's check (`the_map_lights_fit_the_budget_wherever_the_camera_goes`): wherever the camera goes, the slice's own lights
    // that reach the widest view, with Ben's lantern and a shot's light for each of the five shots he can have out, leave a light
    // spare in the 16 the pass takes, so none ever pops in or out.
    let world = level::world();
    let map = &world.map;
    let g = oblique::Ground {
        data: &map.data,
        w: map.w,
        h: map.h,
        red_closed: true,
        blue_closed: true,
    };
    let (vw, vh) = (1280i32, 416i32);
    let mut worst = 0;
    for cam_y in (-vh..map.h * 16).step_by(16) {
        for cam_x in (-vw..map.w * 16).step_by(16) {
            for tick in [0u32, 128, 256, 384] {
                let mut lights = light::Lights::new();
                oblique::night_lights(
                    &mut lights,
                    &g,
                    (cam_x, cam_y),
                    (vw as usize, vh as usize),
                    tick,
                );
                let all = lights.select().to_vec();
                let reaching = all
                    .iter()
                    .filter(|l| {
                        let (x, y, r) = (l.x / light::SUB, l.y / light::SUB, l.r / light::SUB);
                        x + r > cam_x && x - r < cam_x + vw && y + r > cam_y && y - r < cam_y + vh
                    })
                    .count();
                worst = worst.max(reaching + 1 + fizz_ep2::SODA_AMMO as usize);
            }
        }
    }
    assert!(worst < light::MAX_ACTIVE, "{worst} lights reach one view");
}

#[test]
fn the_lanterns_flicker_round_fizz_1_s_1_6() {
    let mut seen = Vec::new();
    for tick in 0..512 {
        let f = oblique::flicker(tick, 236);
        assert!((410 - 51..=410 + 51).contains(&f), "{f}");
        seen.push(f);
    }
    // It moves, and lanterns at different places are out of step.
    assert!(seen.iter().max().unwrap() - seen.iter().min().unwrap() > 60);
    assert_ne!(
        (0..60)
            .map(|t| oblique::flicker(t, 236))
            .collect::<Vec<_>>(),
        (0..60)
            .map(|t| oblique::flicker(t, 348))
            .collect::<Vec<_>>()
    );
}
