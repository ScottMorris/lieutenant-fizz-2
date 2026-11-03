// Tests of the night's palette, light model and shade pass.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::oblique_data::SINE;
use std::vec;
use std::vec::Vec;

#[test]
fn every_entry_follows_the_6_bit_dac_rule() {
    for base in 1..=LEVELS {
        let p = palette(base);
        for (i, e) in p.iter().enumerate() {
            let level = level_of(i as u8, base);
            let ega = EGA6[i & 15];
            for k in 0..3 {
                // `(L - 1) / 13` of the 6-bit value, to the nearest step; a thirteenth is never a half, so there are no ties.
                let want = (f64::from(ega[k]) * f64::from(level - 1) / 13.0).round() as u8;
                assert_eq!(e[k], want, "base {base} entry {i}");
                assert!(e[k] <= 63);
            }
        }
    }
}

#[test]
fn the_day_palette_keeps_the_sixteen_colours_and_the_dim_bank() {
    assert_eq!(&DAY[..16], &EGA6[..]);
    for c in 0..16 {
        // Entries 16 to 31 are fizz-present-dos's dimmed copies: 4/13 of the colour, rounded.
        for k in 0..3 {
            assert_eq!(DAY[16 + c][k], ((u16::from(EGA6[c][k]) * 4 + 6) / 13) as u8);
        }
        // Bank 2 is full too, so a glowing pixel shows the colour itself by day and by night.
        assert_eq!(DAY[usize::from(GLOW) + c], EGA6[c]);
        assert_eq!(palette(6)[usize::from(GLOW) + c], EGA6[c]);
        // Bank 15 is level 1, black.
        assert_eq!(DAY[0xF0 + c], [0, 0, 0]);
    }
    // The night palette differs from the day's in bank 0 only.
    let night = palette(6);
    assert_eq!(&night[16..], &DAY[16..]);
    assert_eq!(night[15], [at_level(63, 6); 3]);
}

#[test]
fn banks_and_levels_round_trip() {
    for l in 1..=LEVELS {
        let b = bank(l);
        assert_eq!(b & 15, 0);
        assert!(b >= 0x20, "never bank 0 or 1");
        assert_eq!(level_of(b | 7, 3), l);
    }
    assert_eq!((bank(0), bank(99)), (bank(1), bank(LEVELS)));
    assert_eq!((GLOW, ACTOR), (0x20, 0x70));
    assert_eq!((level_of(5, 6), level_of(0x15, 6)), (6, DIM_LEVEL));
}

#[test]
fn the_web_table_widens_the_dac_values_to_the_ega_bytes() {
    let t = rgba(&DAY);
    let ega8: [[u8; 3]; 16] = [
        [0x00, 0x00, 0x00],
        [0x00, 0x00, 0xAA],
        [0x00, 0xAA, 0x00],
        [0x00, 0xAA, 0xAA],
        [0xAA, 0x00, 0x00],
        [0xAA, 0x00, 0xAA],
        [0xAA, 0x55, 0x00],
        [0xAA, 0xAA, 0xAA],
        [0x55, 0x55, 0x55],
        [0x55, 0x55, 0xFF],
        [0x55, 0xFF, 0x55],
        [0x55, 0xFF, 0xFF],
        [0xFF, 0x55, 0x55],
        [0xFF, 0x55, 0xFF],
        [0xFF, 0xFF, 0x55],
        [0xFF, 0xFF, 0xFF],
    ];
    for c in 0..16 {
        assert_eq!(t[c], [ega8[c][0], ega8[c][1], ega8[c][2], 0xFF]);
    }
    // Every entry is its DAC value scaled to 8 bits, so the canvas shows what the VGA shows.
    for (e, d) in t.iter().zip(DAY.iter()) {
        for k in 0..3 {
            assert_eq!(u32::from(e[k]), (u32::from(d[k]) * 255 + 31) / 63);
        }
    }
}

#[test]
fn the_falloff_table_matches_its_formula() {
    assert_eq!(FALLOFF.len(), 1024);
    for (i, &f) in FALLOFF.iter().enumerate() {
        let u = ((i as f64 + 0.5) / 1024.0).sqrt();
        let want = ((1.0 - u) * (1.0 - u) / u * 256.0 + 0.5).floor().min(65535.0);
        assert_eq!(f64::from(f), want, "step {i}");
    }
    // Falling all the way: the far end of the radius adds nothing.
    assert!(FALLOFF.windows(2).all(|w| w[0] >= w[1]));
    assert_eq!(FALLOFF[1023], 0);
}

#[test]
fn the_sine_table_matches_its_formula() {
    for (i, &s) in SINE.iter().enumerate() {
        let want = ((2.0 * core::f64::consts::PI * i as f64 / 256.0).sin() * 256.0).round();
        assert_eq!(f64::from(s), want, "step {i}");
    }
}

#[test]
fn the_shade_table_lights_lit_bytes_floors_actors_and_keeps_the_rest() {
    for l in 1..=LEVELS {
        for p in 0..=255u8 {
            let s = shade_byte(p, l);
            assert_eq!(s & 15, p & 15, "the colour stays");
            match p & 0xF0 {
                LIT => assert_eq!(level_of(s, 0), l),
                ACTOR => assert_eq!(level_of(s, 0), l.max(ACTOR_FLOOR)),
                _ => assert_eq!(s, p),
            }
            assert!(level_of(s, 0) >= 1);
        }
    }
}

/// The frame shaded one byte at a time, the plain statement of what [`shade`] does.
fn shade_slow(frame: &mut [u8], w: usize, h: usize, add: &[u16], ambient: u8) {
    let (cols, _) = grid_size(w, h);
    for y in 0..h {
        for x in 0..w {
            let a = add[(y / CELL) * cols + x / CELL];
            let q = cell_q(a, ambient);
            if a == 0 || q == (ambient - 1) * 4 {
                continue;
            }
            let p = &mut frame[y * w + x];
            *p = shade_byte(*p, dithered(q, y, x));
        }
    }
}

/// A small deterministic generator for the tests.
fn lcg(seed: &mut u64) -> u32 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*seed >> 33) as u32
}

#[test]
fn the_word_path_shades_as_byte_by_byte_does_at_any_size() {
    let mut seed = 7u64;
    for &(w, h) in &[(320, 200), (96, 64), (1280, 416), (101, 67), (8, 8), (7, 3), (1, 1)] {
        let (cols, rows) = grid_size(w, h);
        let mut add = vec![0u16; cols * rows];
        for a in add.iter_mut() {
            *a = match lcg(&mut seed) % 4 {
                0 => 0,
                1 => (lcg(&mut seed) % 60) as u16,
                _ => (lcg(&mut seed) % 1500) as u16,
            };
        }
        // Lit bytes mostly, with markers, dimmed and shaded bytes among them.
        let frame: Vec<u8> = (0..w * h)
            .map(|_| match lcg(&mut seed) % 8 {
                0 => ACTOR | (lcg(&mut seed) % 16) as u8,
                1 => GLOW | (lcg(&mut seed) % 16) as u8,
                2 => (lcg(&mut seed) % 256) as u8,
                _ => (lcg(&mut seed) % 16) as u8,
            })
            .collect();
        for ambient in [1, 6, 9] {
            let (mut a, mut b) = (frame.clone(), frame.clone());
            shade(&mut a, w, h, &add, ambient);
            shade_slow(&mut b, w, h, &add, ambient);
            assert_eq!(a, b, "{w} by {h}, ambient {ambient}");
        }
    }
}

#[test]
fn a_cell_at_the_ambient_is_not_written_and_markers_hold() {
    let (w, h) = (32, 16);
    let (cols, rows) = grid_size(w, h);
    let mut add = vec![0u16; cols * rows];
    add[1] = 5; // rounds to no quarter level
    add[2] = 1024; // full brightness over the ambient
    let mut frame: Vec<u8> = (0..w * h).map(|i| (i % 16) as u8).collect();
    for x in 16..24 {
        frame[x] = ACTOR | 3;
        frame[w + x] = GLOW | 4;
    }
    let before = frame.clone();
    shade(&mut frame, w, h, &add, 6);
    for y in 0..h {
        for x in 0..w {
            let (p, q) = (frame[y * w + x], before[y * w + x]);
            if !(16..24).contains(&x) || y >= 8 {
                assert_eq!(p, q, "({x}, {y}) is at the ambient");
            } else if q & 0xF0 == GLOW {
                assert_eq!(p, q, "glowing stays");
            } else {
                assert_eq!(level_of(p, 6), LEVELS, "the cell is full");
            }
        }
    }
    // An actor never shows below level 9: in a dim cell it is raised to the floor.
    let mut actor = vec![ACTOR | 9; 64];
    shade(&mut actor, 8, 8, &[30], 2);
    assert!(actor.iter().all(|&p| level_of(p, 2) == ACTOR_FLOOR));
}

#[test]
fn the_dither_gives_four_steps_between_two_levels() {
    for q in 0..=QMAX {
        let lv: Vec<u8> = (0..4).map(|i| dithered(q, i / 2, i % 2)).collect();
        let lo = 1 + q / 4;
        let up = lv.iter().filter(|&&l| l > lo).count();
        assert_eq!(usize::from(q % 4), up, "q {q}");
        assert!(lv.iter().all(|&l| l == lo || l == lo + 1) && lo + 1 <= LEVELS + 1);
        assert!(lv.iter().all(|&l| l <= LEVELS));
    }
}

#[test]
fn lights_select_the_nearest_sixteen_with_the_pinned_first() {
    let mut l = Lights::new();
    for i in 0..20 {
        let light = Light {
            x: (20 - i) * 100,
            y: 0,
            z: 10,
            r: 100,
            strength: i,
        };
        l.add((0, 0), light).unwrap();
    }
    let far = l
        .add(
            (0, 0),
            Light {
                x: 50_000,
                strength: 99,
                ..Light::default()
            },
        )
        .unwrap();
    l.pin_first(far);
    let sel = l.select();
    assert_eq!(sel.len(), MAX_ACTIVE);
    assert_eq!(sel[0].strength, 99);
    assert_eq!(sel[1].x, 100);
    assert_eq!(sel[2].x, 200);
    assert!(sel.windows(2).skip(1).all(|w| w[0].x <= w[1].x));
    let mut full = Lights::new();
    for _ in 0..POOL {
        assert!(full.add((0, 0), Light::default()).is_some());
    }
    assert!(full.add((0, 0), Light::default()).is_none());
}

#[test]
fn a_light_adds_its_shader_term_and_nothing_past_its_radius() {
    // One light over the middle of a 320 by 200 view: 40 by 25 cells.
    let (cols, rows) = grid_size(320, 200);
    let mut add = vec![0u16; cols * rows];
    let light = Light {
        x: 164 * SUB,
        y: 100 * SUB,
        z: 16 * SUB,
        r: 80 * SUB,
        strength: LIGHT_ONE,
    };
    add_lights(&mut add, cols, rows, &[light], (0, 0));
    for cy in 0..rows {
        for cx in 0..cols {
            let (dx, dy) = (
                (cx * CELL + 4) as f64 - 164.0,
                (cy * CELL + 4) as f64 - 100.0,
            );
            let d = (dx * dx + dy * dy + 16.0 * 16.0).sqrt();
            let got = f64::from(add[cy * cols + cx]);
            if d >= 80.0 {
                assert_eq!(got, 0.0, "({cx}, {cy})");
                continue;
            }
            // Fizz 1's shader with a normal facing the screen: z / |d| * (1 - |d| / r)^2, within the table's step.
            let want = 1024.0 * 16.0 / d * (1.0 - d / 80.0).powi(2);
            assert!(
                (got - want).abs() <= want * 0.06 + 3.0,
                "({cx}, {cy}) {got} {want}"
            );
        }
    }
}

#[test]
fn no_light_or_size_writes_outside_the_grid_or_the_frame() {
    let mut seed = 99u64;
    for _ in 0..300 {
        let w = 1 + (lcg(&mut seed) % 1300) as usize;
        let h = 1 + (lcg(&mut seed) % 420) as usize;
        let (cols, rows) = grid_size(w, h);
        let mut add = vec![0u16; cols * rows];
        let mut lights = Vec::new();
        for _ in 0..(lcg(&mut seed) % 20) {
            let big = |s: &mut u64| (lcg(s) as i32) >> (lcg(s) % 31);
            lights.push(Light {
                x: big(&mut seed),
                y: big(&mut seed),
                z: big(&mut seed),
                r: big(&mut seed),
                strength: big(&mut seed),
            });
        }
        let cam = (lcg(&mut seed) as i32, lcg(&mut seed) as i32 >> 3);
        add_lights(&mut add, cols, rows, &lights, cam);
        let mut frame = vec![3u8; w * h];
        shade(&mut frame, w, h, &add, 6);
        // A grid or frame too small is refused, not overrun.
        add_lights(&mut add[..cols * rows - 1], cols, rows, &lights, cam);
        shade(&mut frame[..w * h - 1], w, h, &add, 6);
        shade(&mut frame, w, h, &add[..cols * rows - 1], 6);
    }
}

#[test]
fn lowering_a_span_darkens_by_levels_and_keeps_colours() {
    let mut span = [0u8, 7, 15, GLOW | 14, bank(3) | 2, DIM | 9];
    lower_span(&mut span, 2, LEVELS);
    assert_eq!(
        span,
        [bank(12), bank(12) | 7, bank(12) | 15, bank(12) | 14, bank(1) | 2, bank(3) | 9]
    );
    let mut night = [4u8];
    lower_span(&mut night, 1, 6);
    assert_eq!(night, [bank(5) | 4]);
    let mut floor = [bank(1) | 5];
    lower_span(&mut floor, 9, 6);
    assert_eq!(floor, [bank(1) | 5]);
}
