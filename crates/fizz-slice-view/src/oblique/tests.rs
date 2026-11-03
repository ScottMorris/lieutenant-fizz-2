// Tests of the oblique ground: its geometry, its clipping, its overdraw and its baked data.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::{VIEW_H, VIEW_W, ben_view_box, camera};
use fizz_ep2::tiles::{DOOR_BLUE_CHANNEL, DOOR_RED_CHANNEL};
use fizz_ep2::{SLICE_SCRIPT, World, level, step};
use fizz_sim::{Buttons, InputFrame};
use std::vec;
use std::vec::Vec;

fn ground(world: &World) -> Ground<'_> {
    Ground {
        data: &world.map.data,
        w: world.map.w,
        h: world.map.h,
        red_closed: world.map.switch(DOOR_RED_CHANNEL),
        blue_closed: world.map.switch(DOOR_BLUE_CHANNEL),
    }
}

/// Plays the slice script and calls `each` with the world before the first tick and after every tick.
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
fn the_slant_table_is_the_rounded_slant() {
    assert_eq!(O.len(), 26);
    for r in 0..26u32 {
        // round((25 - r) * 10 / 25), halves up, in integers: floor(((25 - r) * 10 * 2 + 25) / 50).
        assert_eq!(
            u32::from(O[r as usize]),
            ((25 - r) * 10 * 2 + 25) / 50,
            "row {r}"
        );
    }
    assert_eq!((O[0], O[16], O[25]), (10, 4, 0));
}

#[test]
fn every_baked_table_and_piece_is_well_formed() {
    for (name, t, rows) in [
        ("plank top", &PLANK_TOP[..], 26),
        ("plank front", &PLANK_FRONT[..], 6),
        ("stone top", &STONE_TOP[..], 26),
        ("stone shallow", &STONE_SHALLOW[..], 64),
        ("stone mid", &STONE_MID[..], 64),
        ("stone deep", &STONE_DEEP[..], 64),
        ("brick front", &BRICK_FRONT[..], 32),
        ("back wall", &BACK_WALL[..], 96),
    ] {
        assert_eq!(t.len(), PERIOD * rows, "{name}");
        assert!(t.iter().all(|&p| p <= 15), "{name} has only EGA colours");
    }
    for (name, p) in [
        ("door", &DOOR_RED_TOP),
        ("exit", &EXIT_DOORWAY),
        ("spikes", &SPIKES),
        ("rail", &RAIL),
        ("bracket", &BRACKET),
        ("porthole", &PORTHOLE),
    ] {
        assert_eq!(
            p.pixels.len(),
            usize::from(p.w) * usize::from(p.h),
            "{name}"
        );
        assert!(p.pixels.iter().all(|&c| c <= 15 || c == 255), "{name}");
        // The runs are exactly the maximal stretches of opaque pixels, row by row.
        assert_eq!(p.rows.len(), usize::from(p.h) + 1, "{name}");
        for y in 0..usize::from(p.h) {
            let row = &p.pixels[y * usize::from(p.w)..(y + 1) * usize::from(p.w)];
            let mut seen = Vec::new();
            let mut x = 0;
            while x < row.len() {
                if row[x] == 255 {
                    x += 1;
                    continue;
                }
                let e = row[x..]
                    .iter()
                    .position(|&c| c == 255)
                    .map_or(row.len(), |n| x + n);
                seen.push((x as u8, (e - x) as u8));
                x = e;
            }
            let runs = &p.runs[usize::from(p.rows[y])..usize::from(p.rows[y + 1])];
            assert_eq!(runs, &seen[..], "{name} row {y}");
        }
    }
    for (name, s, h) in [
        ("stone side", &SIDE_STONE, 42),
        ("brick side", &SIDE_BRICK, 42),
        ("plank side", &SIDE_PLANK, 31),
        ("red door side", &SIDE_DOOR_RED, 42),
        ("blue door side", &SIDE_DOOR_BLUE, 42),
    ] {
        assert_eq!(usize::from(s.h), h, "{name}");
        assert_eq!(s.spans.len(), h, "{name} has a span for each row");
        let used: usize = s.spans.iter().map(|&(_, len, _)| usize::from(len)).sum();
        assert_eq!(used, s.pixels.len(), "{name} packs only the spans' pixels");
        let extent = s.spans.iter().map(|&(a, len, _)| a + len).max();
        assert_eq!(Some(s.w), extent, "{name} says how wide it is");
        assert!(s.pixels.iter().all(|&c| c <= 15), "{name}");
    }
}

/// A map of `w` by `h` back wall with the given tiles (`x`, `y` from the bottom, tile) put over it.
fn small_map(w: usize, h: usize, set: &[(usize, usize, u8)]) -> Vec<u8> {
    let mut data = vec![WALLBG; w * h];
    for &(x, y, t) in set {
        data[y * w + x] = t;
    }
    data
}

#[test]
fn a_ledge_ends_slant_crosses_the_feet_line_on_the_tile_edge() {
    // A plank run over columns 2 to 5 in the middle row of an 8 by 3 map: its collision top is at map y = 16.
    let data = small_map(
        8,
        3,
        &[(2, 1, PLAT), (3, 1, PLAT), (4, 1, PLAT), (5, 1, PLAT)],
    );
    let g = Ground {
        data: &data,
        w: 8,
        h: 3,
        red_closed: true,
        blue_closed: true,
    };
    let mut f = vec![0xEEu8; 128 * 48];
    draw_ground(&mut f, 128, 48, &g, 0, 0);
    let feet_line = 16;
    for s in 0..STRIP {
        // Strip row `s` lies on map row feet_line - 16 + s, and its caps on the near edge plus that row's slant.
        let y = (feet_line - BACK + s) as usize;
        let left = (2 * 16 - FEET_DX + i32::from(O[s as usize])) as usize;
        let right = (6 * 16 - FEET_DX + i32::from(O[s as usize]) - 1) as usize;
        assert_eq!(f[y * 128 + left], BLACK, "left cap of row {s}");
        assert_eq!(f[y * 128 + right], BLACK, "right cap of row {s}");
    }
    // On the feet line the slanted ends are exactly on the tile edges, x = 32 and x = 95.
    let y = feet_line as usize;
    assert_eq!(f[y * 128 + 32], BLACK);
    assert_eq!(f[y * 128 + 95], BLACK);
    // Wood lies between them, and nothing of the plank lies beyond.
    assert!(f[y * 128 + 33..y * 128 + 94].contains(&BROWN));
    assert!(
        f[y * 128 + 96..y * 128 + 110]
            .iter()
            .all(|&p| p != BROWN && p != 14)
    );
}

#[test]
fn bens_feet_row_is_16_px_below_the_far_edge_of_his_ledge() {
    // Ben stands on the ledge of row 13, columns 8 to 14: the feet line is the collision top of those tiles.
    let mut w = level::world();
    w.ben.b.x = 9.0;
    w.ben.b.y = 14.0;
    let mut f = vec![0xEEu8; VIEW_W * VIEW_H];
    crate::draw_slice(&mut f, VIEW_W, VIEW_H, &w, 0);
    let (cam_x, cam_y) = camera(&w, VIEW_W, VIEW_H);
    let (bx, by, _, bh) = ben_view_box(&w, VIEW_W, VIEW_H, &crate::DrawOptions::default());
    let feet = by + bh;
    // His box is 16 px tall, so the strip's far edge is the top of the box.
    let far_edge = feet - 16;
    assert_eq!(far_edge, by);
    // Probe the plank three tiles right of Ben, clear of his sprite: the far edge row (strip row 0) and the near edge row
    // (strip row 25) are the baked plank rows at the pixel's world column.
    let px = bx + 48;
    for strip_row in [0usize, 25] {
        let u = (px + cam_x - i32::from(O[strip_row]) + FEET_DX).rem_euclid(256) as usize;
        let want = PLANK_TOP[strip_row * PERIOD + u];
        let row = far_edge + strip_row as i32;
        assert_eq!(
            f[row as usize * VIEW_W + px as usize],
            want,
            "strip row {strip_row}"
        );
    }
    // The feet line is the tile edge: 14 tiles up from the bottom of the map.
    assert_eq!(feet + cam_y, (w.map.h - 14) * 16);
}

#[test]
fn the_window_gives_the_direct_answer() {
    // The window's tiles, looks and backgrounds are what the map alone gives, at cameras in the map and far outside it, with
    // the doors shut and open.
    let w = level::world();
    for (red, blue) in [(true, true), (false, true), (true, false), (false, false)] {
        let g = Ground {
            red_closed: red,
            blue_closed: blue,
            ..ground(&w)
        };
        for (cx, cy) in [
            (0, 0),
            (0, 400),
            (100, 300),
            (200, 0),
            (300, 500),
            (-50, -50),
            (700, 900),
            (-900, 300),
            (4000, 4000),
            (160, 20),
        ] {
            let (c_lo, c_hi, r_lo, r_hi) = (cx / 16 - 2, cx / 16 + 24, cy / 16 - 2, cy / 16 + 16);
            let (mut tile, mut info) = ([0u8; LARGE_CELLS], [0u8; LARGE_CELLS]);
            let win = Window::new(&g, &mut tile, &mut info, c_lo, c_hi, r_lo, r_hi);
            assert!(win.ok);
            for r in r_lo - MARGIN..=r_hi + MARGIN {
                for c in c_lo - MARGIN..=c_hi + MARGIN {
                    assert_eq!(win.tile(c, r), g.cell(c, r), "tile ({c}, {r})");
                }
            }
            for r in r_lo..=r_hi {
                for c in c_lo..=c_hi {
                    assert_eq!(
                        win.info(c, r),
                        win.info_direct(c, r),
                        "cell ({c}, {r}) at ({cx}, {cy}), doors {red} {blue}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_view_too_large_for_the_window_is_drawn_without_one() {
    let w = level::world();
    let g = ground(&w);
    let (mut tile, mut info) = ([0u8; LARGE_CELLS], [0u8; LARGE_CELLS]);
    assert!(!Window::new(&g, &mut tile, &mut info, 0, MAX_COLS as i32, 0, 10).ok);
    assert!(!Window::new(&g, &mut tile, &mut info, 0, 10, 0, MAX_ROWS as i32).ok);
    // The widest and tallest Web view (1280 by 416 pixels) fits, at any camera.
    for cam_x in [-5, 0, 7, 15, 16] {
        for cam_y in [-5, 0, 7, 15, 16] {
            let cv = Canvas {
                px: &mut [],
                w: 1280,
                h: 416,
            };
            let (c_lo, c_hi, r_lo, r_hi) = cell_range(&cv, cam_x, cam_y);
            assert!(
                Window::new(&g, &mut tile, &mut info, c_lo, c_hi, r_lo, r_hi).ok,
                "camera ({cam_x}, {cam_y})"
            );
        }
    }
    // A frame 40 by 3000 pixels is too tall for a window; it gives the picture that bands of it, each of which fits, give.
    let (fw, fh) = (40usize, 3000usize);
    let mut whole = vec![0u8; fw * fh];
    draw_ground(&mut whole, fw, fh, &g, 100, 10);
    let mut bands = vec![0u8; fw * fh];
    for (i, band) in bands.chunks_exact_mut(fw * 250).enumerate() {
        draw_ground(band, fw, 250, &g, 100, 10 + 250 * i as i32);
    }
    assert!(whole == bands, "banded and whole pictures differ");
}

#[test]
fn stone_is_deeper_the_further_it_lies_from_an_opening() {
    let w = level::world();
    let g = ground(&w);
    // Row 36 of the left mass lies under the gallery's open air; row 20 of the right mass is clear of every opening.
    assert_eq!(depth_direct(&g, 5, 36), 1);
    assert_eq!(depth_direct(&g, 5, 35), 2);
    assert_eq!(depth_direct(&g, 5, 34), 3);
    assert_eq!(depth_direct(&g, 5, 33), 4);
    assert_eq!(depth_direct(&g, 24, 12), 4);
}

#[test]
fn nothing_is_written_outside_the_frame_at_any_camera_or_size() {
    let w = level::world();
    let g = ground(&w);
    let (mw, mh) = (w.map.w * 16, w.map.h * 16);
    for (fw, fh) in [
        (96usize, 64usize),
        (320, 200),
        (1280, 416),
        (1, 1),
        (7, 5),
        (97, 417),
        (640, 400),
    ] {
        // Canary bytes after the frame must stay as they are, and every pixel of the frame must be written.
        let mut buf = vec![0xEEu8; fw * fh + 64];
        let (fwi, fhi) = (fw as i32, fh as i32);
        let xs = [
            -1300,
            -400,
            -17,
            -1,
            0,
            1,
            15,
            100,
            mw / 2,
            mw - fwi,
            mw - fwi + 1,
            mw,
            mw + 400,
            mw + 1300,
        ];
        let ys = [
            -500,
            -17,
            -1,
            0,
            1,
            100,
            mh / 2,
            mh - fhi,
            mh - fhi + 1,
            mh,
            mh + 600,
        ];
        for &cx in &xs {
            for &cy in &ys {
                buf[..fw * fh].fill(0xEE);
                draw_ground(&mut buf, fw, fh, &g, cx, cy);
                assert!(
                    buf[..fw * fh].iter().all(|&p| p <= 15),
                    "{fw}x{fh} at ({cx}, {cy}) leaves a pixel unwritten"
                );
                assert!(
                    buf[fw * fh..].iter().all(|&p| p == 0xEE),
                    "{fw}x{fh} at ({cx}, {cy}) writes past the frame"
                );
            }
        }
    }
}

#[test]
fn degenerate_frames_and_maps_never_panic() {
    let w = level::world();
    let g = ground(&w);
    let mut tiny = vec![0u8; 10];
    draw_ground(&mut tiny, 0, 5, &g, 0, 0);
    draw_ground(&mut tiny, 5, 0, &g, 0, 0);
    draw_ground(&mut tiny, 5, 5, &g, 0, 0);
    draw_ground(&mut tiny, 3, 3, &g, i32::MAX, i32::MIN);
    draw_ground(&mut tiny, 3, 3, &g, i32::MIN, i32::MAX);
    let empty = Ground {
        data: &[],
        w: 0,
        h: 0,
        red_closed: false,
        blue_closed: false,
    };
    draw_ground(&mut tiny, 3, 3, &empty, 0, 0);
    let short = Ground {
        data: &w.map.data[..10],
        w: 32,
        h: 48,
        red_closed: true,
        blue_closed: true,
    };
    draw_ground(&mut tiny, 3, 3, &short, 0, 0);
    let one = [FILL];
    let single = Ground {
        data: &one,
        w: 1,
        h: 1,
        red_closed: true,
        blue_closed: true,
    };
    let mut f = vec![0xEEu8; 64 * 64];
    draw_ground(&mut f, 64, 64, &single, -20, -20);
    assert!(f.iter().all(|&p| p <= 15));
}

#[test]
fn views_of_the_extreme_web_sizes_draw_every_pixel_through_the_replay() {
    // The narrowest and widest Web views (96 by 64 and 1280 by 416), at Ben's camera on sampled ticks.
    let mut tick = 0u64;
    play(|w| {
        tick += 1;
        if tick % 97 != 1 {
            return;
        }
        for (fw, fh) in [(96usize, 64usize), (1280, 416), (320, 208)] {
            let (cx, cy) = camera(w, fw, fh);
            let mut f = vec![0xEEu8; fw * fh];
            draw_ground(&mut f, fw, fh, &ground(w), cx, cy);
            assert!(f.iter().all(|&p| p <= 15), "tick {tick} at {fw}x{fh}");
        }
    });
}

#[test]
fn the_ground_overdraws_less_than_two_and_a_half_writes_a_pixel_over_the_replay() {
    // Every pixel is written at least once, so the average is writes per pixel. The bound is the cost budget of the DOS
    // build (docs/PLATFORM.md): the pixels copied for each picture pixel, background included.
    let (mut writes, mut runs, mut frames, mut worst) = (0u64, 0u64, 0u64, 0f64);
    let mut phases = [(0u64, 0u64); PHASES];
    play(|w| {
        let (cx, cy) = camera(w, VIEW_W, VIEW_H);
        let mut f = vec![0xEEu8; VIEW_W * VIEW_H];
        WRITES.with(|c| c.set([(0, 0); PHASES]));
        draw_ground(&mut f, VIEW_W, VIEW_H, &ground(w), cx, cy);
        let by_phase = WRITES.with(|c| c.get());
        let px: u64 = by_phase.iter().map(|p| p.0).sum();
        for (i, p) in by_phase.iter().enumerate() {
            phases[i].0 += p.0;
            phases[i].1 += p.1;
        }
        let r: u64 = by_phase.iter().map(|p| p.1).sum();
        writes += px;
        runs += r;
        frames += 1;
        worst = worst.max(px as f64 / (VIEW_W * VIEW_H) as f64);
    });
    let average = writes as f64 / (frames as f64 * (VIEW_W * VIEW_H) as f64);
    std::eprintln!(
        "ground overdraw: average {average:.2} writes a pixel (worst frame {worst:.2}), {:.0} runs a frame",
        runs as f64 / frames as f64
    );
    for (name, p) in ["back", "dress", "top", "front", "side", "stand"]
        .iter()
        .zip(phases)
    {
        std::eprintln!(
            "  {name}: {:.0} px, {:.0} runs a frame",
            p.0 as f64 / frames as f64,
            p.1 as f64 / frames as f64
        );
    }
    assert!(average < 2.5, "average overdraw {average}");
    assert!(worst < 3.5, "worst frame overdraw {worst}");
}

#[test]
fn the_generated_data_is_what_the_bake_tool_writes() {
    // `bun tools/tiles/emit-oblique.ts --check` compares the committed file with a fresh bake. It runs where Bun is installed
    // (CI always has it); a host without it skips this test and says so.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match std::process::Command::new("bun")
        .args(["tools/tiles/emit-oblique.ts", "--check"])
        .current_dir(root)
        .output()
    {
        Ok(out) => assert!(
            out.status.success(),
            "oblique_data.rs is out of date: {}",
            std::string::String::from_utf8_lossy(&out.stderr)
        ),
        Err(_) => std::eprintln!("skipped: bun is not installed, so the bake check did not run"),
    }
}

#[test]
fn the_headland_heights_are_the_boards_sines_rounded() {
    for (ix, &h) in HEADLAND_HEIGHTS.iter().enumerate() {
        let ix = ix as f64;
        let want =
            (0.5 + 1.5 * (ix * 0.19 + 0.12 * 9.0).sin() + (ix * 0.067 + 0.12 * 20.0).sin()).round();
        assert_eq!(f64::from(h), want, "column {ix}");
    }
}

#[test]
fn the_sprites_of_the_stack_are_as_designed() {
    // The mountain tile: a peak from row 2 widening to the bottom, D only.
    assert!(MTN_TOP.iter().all(|&p| p == 255 || p == DARK_GREY));
    let on = |x: usize, y: usize| MTN_TOP[y * 16 + x] != 255;
    for y in 0..16 {
        for x in 0..16 {
            let want = y as f64 >= (x as f64 - 7.5).abs() * 1.8 + 2.0;
            assert_eq!(on(x, y), want, "({x}, {y})");
        }
    }
    // The sea tile: nothing above row 10; crest, crest, checker, checker, B, B below.
    assert!(SEA_TOP[..10 * 16].iter().all(|&p| p == 255));
    for x in 0..16 {
        assert_eq!(SEA_TOP[10 * 16 + x], if x % 6 < 4 { CYAN } else { 255 });
        assert_eq!(SEA_TOP[11 * 16 + x], CYAN);
        assert_eq!(SEA_TOP[14 * 16 + x], BLUE);
        assert_eq!(SEA_TOP[15 * 16 + x], BLUE);
    }
}

#[test]
fn the_horizon_sits_on_map_row_39_at_the_gallery_camera() {
    // An all-open map as tall as the slice: the sea's crest row (the first row with C) is the top of row 39 less the camera.
    let data = vec![EMPTY; 32 * 48];
    let g = Ground {
        data: &data,
        w: 32,
        h: 48,
        red_closed: true,
        blue_closed: true,
    };
    let mut f = vec![0xEEu8; 320 * 200];
    draw_ground(&mut f, 320, 200, &g, 100, HORIZON_CAM_Y);
    let row_has = |y: usize, c: u8| f[y * 320..y * 320 + 320].contains(&c);
    let map_y = (48 - 1 - HORIZON_ROW) * 16;
    let crest = (map_y - HORIZON_CAM_Y) as usize;
    assert!(row_has(crest, CYAN), "the crest is on row {crest}");
    assert!(!row_has(crest - 1, CYAN));
    // Above the sea the sky is dark with stars, headlands and clouds; below it is the sea's B and k.
    assert!(f[..crest * 320].iter().all(|&p| p <= 15));
    assert!(
        f[(crest + 6) * 320..(crest + 20) * 320]
            .iter()
            .all(|&p| p == BLUE || p == BLACK)
    );
}

#[test]
fn layer_pixels_appear_only_where_a_cell_is_open_air() {
    // The clouds drift with the tick and nothing else changes with it, so the pixels that differ between two ticks show where
    // the layers can reach: every one lies in a cell whose background is open air.
    let mut checked = 0;
    let mut tick = 0u64;
    play(|w| {
        tick += 1;
        if tick % 60 != 1 {
            return;
        }
        let (cx, cy) = camera(w, VIEW_W, VIEW_H);
        let g = ground(w);
        let (mut a, mut b) = (vec![0xEEu8; VIEW_W * VIEW_H], vec![0xEEu8; VIEW_W * VIEW_H]);
        draw_ground_at(&mut a, VIEW_W, VIEW_H, &g, cx, cy, 0);
        draw_ground_at(&mut b, VIEW_W, VIEW_H, &g, cx, cy, 6000);
        for y in 0..VIEW_H {
            for x in 0..VIEW_W {
                if a[y * VIEW_W + x] != b[y * VIEW_W + x] {
                    let (c, r) = (
                        (x as i32 + cx).div_euclid(16),
                        (y as i32 + cy).div_euclid(16),
                    );
                    assert!(
                        !back_is_wall(&g, c, r),
                        "tick {tick}: ({x}, {y}) differs under a wall cell"
                    );
                    checked += 1;
                }
            }
        }
    });
    assert!(
        checked > 100,
        "the replay shows clouds drifting ({checked} pixels)"
    );
}

#[test]
fn indoor_frames_pay_nothing_for_the_layers_and_every_frame_is_repeatable() {
    let mut w = level::world();
    w.ben.b.x = 9.0;
    w.ben.b.y = 14.0;
    let (cx, cy) = camera(&w, VIEW_W, VIEW_H);
    let g = ground(&w);
    let mut a = vec![0xEEu8; VIEW_W * VIEW_H];
    WRITES.with(|c| c.set([(0, 0); PHASES]));
    draw_ground_at(&mut a, VIEW_W, VIEW_H, &g, cx, cy, 77);
    let px: u64 = WRITES.with(|c| c.get()).iter().map(|p| p.0).sum();
    // Indoors no cell is open air, so no layer is asked to draw: the picture is the back wall, terrain and dressing alone.
    assert!(px < 2 * (VIEW_W * VIEW_H) as u64, "{px} pixels written");
    let mut b = vec![0xEEu8; VIEW_W * VIEW_H];
    draw_ground_at(&mut b, VIEW_W, VIEW_H, &g, cx, cy, 77);
    assert_eq!(a, b);
    // And the same inputs give the same picture on a gallery frame, where the layers draw.
    w.ben.b.x = 20.5;
    w.ben.b.y = 38.0;
    let (cx, cy) = camera(&w, VIEW_W, VIEW_H);
    let g = ground(&w);
    draw_ground_at(&mut a, VIEW_W, VIEW_H, &g, cx, cy, 301);
    draw_ground_at(&mut b, VIEW_W, VIEW_H, &g, cx, cy, 301);
    assert_eq!(a, b);
}

#[test]
fn the_light_sky_is_the_full_one_without_its_clouds_and_headlands() {
    // Light draws the same ground: only pixels of open-air cells differ, and there the Full picture's extra colours are the
    // clouds' B and the headlands' D, over the black sky that Light shows (or a star, which a cloud or headland would hide).
    // Light does not drift, so it is the same at every tick.
    let mut differing = 0;
    let mut tick = 0u64;
    play(|w| {
        tick += 1;
        if tick % 30 != 1 {
            return;
        }
        let (cx, cy) = camera(w, VIEW_W, VIEW_H);
        let g = ground(w);
        let mut full = vec![0xEEu8; VIEW_W * VIEW_H];
        let mut light = full.clone();
        let mut later = full.clone();
        draw_ground_sky(&mut full, VIEW_W, VIEW_H, &g, cx, cy, tick as u32, true);
        draw_ground_sky(&mut light, VIEW_W, VIEW_H, &g, cx, cy, tick as u32, false);
        draw_ground_sky(&mut later, VIEW_W, VIEW_H, &g, cx, cy, 6000, false);
        assert_eq!(light, later, "tick {tick}: the light sky does not drift");
        for y in 0..VIEW_H {
            for x in 0..VIEW_W {
                let (f, l) = (full[y * VIEW_W + x], light[y * VIEW_W + x]);
                if f == l {
                    continue;
                }
                let (c, r) = (
                    (x as i32 + cx).div_euclid(16),
                    (y as i32 + cy).div_euclid(16),
                );
                assert!(
                    !back_is_wall(&g, c, r),
                    "tick {tick}: ({x}, {y}) differs under a wall cell"
                );
                assert!(
                    matches!(f, BLUE | DARK_GREY) && matches!(l, BLACK | 15 | 11 | 14),
                    "tick {tick}: ({x}, {y}) is {f} in Full and {l} in Light"
                );
                differing += 1;
            }
        }
    });
    assert!(
        differing > 1000,
        "the gallery shows clouds and headlands ({differing} pixels)"
    );
}

#[test]
fn a_light_gallery_frame_is_pinned() {
    // Ben on the gallery: the stars and the sea, and the ground as the Full sky draws it.
    let mut w = level::world();
    w.ben.b.x = 20.5;
    w.ben.b.y = 38.0;
    let (cx, cy) = camera(&w, VIEW_W, VIEW_H);
    let g = ground(&w);
    let mut f = vec![0u8; VIEW_W * VIEW_H];
    draw_ground_sky(&mut f, VIEW_W, VIEW_H, &g, cx, cy, 301, false);
    let digest = f.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &p| {
        (h ^ u64::from(p)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    assert_eq!(digest, 0x0e2c_811b_ac67_44b7);
}
