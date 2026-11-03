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
fn the_depth_cache_gives_the_direct_answer() {
    let w = level::world();
    let g = ground(&w);
    for (cx, cy) in [
        (0, 0),
        (0, 400),
        (100, 300),
        (200, 0),
        (300, 500),
        (-50, -50),
        (700, 900),
    ] {
        let (c_lo, c_hi, r_lo, r_hi) = (cx / 16 - 2, cx / 16 + 24, cy / 16 - 2, cy / 16 + 16);
        let d = Depth::new(&g, c_lo, c_hi, g.h - 1 - r_hi, g.h - 1 - r_lo);
        for r in r_lo..=r_hi {
            for c in c_lo..=c_hi {
                let ty = g.h - 1 - r;
                assert_eq!(d.at(&g, c, ty), depth_direct(&g, c, ty), "cell ({c}, {ty})");
            }
        }
    }
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
