// Writes one picture of the slice, or of the bare ground at a chosen camera, as a binary PPM so it can be looked at.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   cargo run -p fizz-slice-view --example frame -- <tick> <out.ppm> [width height]
//       the picture at replay tick <tick> (0 is the start), as the presenters draw it
//   cargo run -p fizz-slice-view --example frame -- ground <cam_x> <cam_y> <out.ppm> [width height]
//       the ground alone (no Ben, items or chickens) with the view's top left at map pixel (cam_x, cam_y)
//   cargo run -p fizz-slice-view --example frame -- all <out_dir>/<stem>
//       the slice at every 100th replay tick, as <stem>-<tick>.ppm
//   cargo run --release -p fizz-slice-view --example frame -- bench [all|indoor|gallery] [rounds] [width height]
//       host timing of the ground over the bench cameras, indoor and gallery separately
//   cargo run --release -p fizz-slice-view --example frame -- script [--night]
//       draws every tick of the replay as the DOS bench does (`frame::draw_tick`, one call a tick), for callgrind to count
//   Add --open to a `ground` run to open both doors, --night to any picture to draw it at night (for `ground`, with the
//   level's own lights and no Ben) and --light to draw it with the Light detail (the Sky setting; the DOS default), which at
//   night is also the lights' shorter reach. The colours are the palette the presenters show, the night's at night.
//
// The size defaults to 320 by 200. Convert with any PPM reader, for example `python3 -c "from PIL import Image; Image.open('f.ppm').save('f.png')"`.
use fizz_ep2::tiles::{DOOR_BLUE_CHANNEL, DOOR_RED_CHANNEL};
use fizz_ep2::{SLICE_SCRIPT, World, level, step};
use fizz_sim::{Buttons, InputFrame};
use fizz_slice_view::oblique::{Ground, draw_ground, draw_ground_at};
use fizz_slice_view::{DrawOptions, draw_slice_with, light, oblique, palette_for};
use std::fs;

fn write_ppm(path: &str, frame: &[u8], w: usize, h: usize, options: &DrawOptions) {
    let rgba = light::rgba(&palette_for(options));
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for &p in &frame[..w * h] {
        out.extend_from_slice(&rgba[usize::from(p)][..3]);
    }
    fs::write(path, out).expect("write the picture");
}

fn world_at(tick: u64) -> World {
    let mut world = level::world();
    let mut prev = Buttons::NONE;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            if world.tick.0 >= tick {
                return world;
            }
            step(&mut world, InputFrame::from_transition(prev, now));
            prev = now;
        }
    }
    world
}

fn size(args: &[String], at: usize) -> (usize, usize) {
    match (args.get(at), args.get(at + 1)) {
        (Some(w), Some(h)) => (w.parse().expect("width"), h.parse().expect("height")),
        _ => (320, 200),
    }
}

/// Whether the view at (`cam_x`, `cam_y`) shows any open air (a cell of empty sky or the exit), which is what the gallery's
/// cameras do and the indoor ones do not.
fn sees_open_air(world: &World, w: usize, h: usize, cam_x: i32, cam_y: i32) -> bool {
    let (mw, mh) = (world.map.w, world.map.h);
    let (c0, c1) = (cam_x.div_euclid(16), (cam_x + w as i32 - 1).div_euclid(16));
    let (r0, r1) = (cam_y.div_euclid(16), (cam_y + h as i32 - 1).div_euclid(16));
    (r0..=r1).any(|r| {
        (c0..=c1).any(|c| {
            let (tx, ty) = (c.clamp(0, mw - 1), (mh - 1 - r).clamp(0, mh - 1));
            let t = world.map.data[(ty * mw + tx) as usize];
            t == fizz_ep2::tiles::EMPTY || t == fizz_ep2::tiles::EXIT
        })
    })
}

/// Host timing of the ground over camera positions down the map, indoor and gallery (open air) separately, against the flat
/// tile pass it replaced. `bench [all|indoor|gallery] [rounds] [width height]`; with one round it is also what
/// `valgrind --tool=callgrind` counts (`--dump-after=` the draw function gives one part for each frame, in camera order,
/// and the `BENCH CAM` lines name them).
fn bench(args: &[String]) {
    let which = args.get(1).map_or("all", String::as_str);
    let rounds: u32 = args.get(2).map_or(40, |r| r.parse().expect("rounds"));
    let (w, h) = size(args, 3);
    let world = level::world();
    let g = Ground {
        data: &world.map.data,
        w: world.map.w,
        h: world.map.h,
        red_closed: true,
        blue_closed: true,
    };
    let cams: Vec<(i32, i32)> = (0..48).map(|i| (((i * 7) % 12) * 16, i * 12)).collect();
    let mut frame = vec![0u8; w * h];
    let mut flat_total = 0.0;
    let mut flat_n = 0;
    let sets: Vec<(&str, Vec<(i32, i32)>)> = ["indoor", "gallery"]
        .into_iter()
        .map(|class| {
            let set = cams
                .iter()
                .copied()
                .filter(|&(x, y)| sees_open_air(&world, w, h, x, y) == (class == "gallery"))
                .collect();
            (class, set)
        })
        .collect();
    for (class, set) in sets {
        if which != "all" && which != class {
            continue;
        }
        let mut total = 0.0;
        let mut worst = (0.0f64, (0, 0));
        for &(x, y) in &set {
            println!("BENCH CAM {class} {x} {y}");
            let t = std::time::Instant::now();
            for round in 0..rounds {
                draw(&mut frame, w, h, &g, x, y, round);
                std::hint::black_box(&frame);
            }
            let us = t.elapsed().as_secs_f64() * 1e6 / f64::from(rounds);
            total += us;
            if us > worst.0 {
                worst = (us, (x, y));
            }
        }
        if !set.is_empty() {
            println!(
                "{class}: {} cameras, ground avg {:.1} us, worst {:.1} us at ({}, {})",
                set.len(),
                total / set.len() as f64,
                worst.0,
                worst.1.0,
                worst.1.1
            );
        }
        // The flat pass is timed only when the run is for timing; a single round is for counting instructions.
        if rounds > 1 {
            let t = std::time::Instant::now();
            for _ in 0..rounds {
                for &(x, y) in &set {
                    fizz_view::draw_tilemap_on_sky(&mut frame, w, h, x, y, 32, 48, 9, |tx, ty| {
                        let t = world.map.data[(ty * 32 + tx) as usize];
                        (t != 0).then_some(t)
                    });
                    std::hint::black_box(&frame);
                }
            }
            flat_total += t.elapsed().as_secs_f64() * 1e6;
            flat_n += set.len() as u32 * rounds;
        }
    }
    if flat_n > 0 {
        println!(
            "flat tile pass avg {:.1} us",
            flat_total / f64::from(flat_n)
        );
    }
}

#[inline(never)]
fn draw_tick(frame: &mut [u8], world: &World, options: &DrawOptions) {
    draw_slice_with(frame, 320, 200, world, world.tick.0 as u32, options);
    std::hint::black_box(&frame);
}

#[inline(never)]
fn draw(frame: &mut [u8], w: usize, h: usize, g: &Ground, x: i32, y: i32, round: u32) {
    // The clouds drift with the tick, so a timed run draws at a different tick each round.
    draw_ground_at(frame, w, h, g, x, y, 600 + round);
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--open` leaves both doors open, to look at what they hide.
    let open = args.iter().any(|a| a == "--open");
    args.retain(|a| a != "--open");
    let night = args.iter().any(|a| a == "--night");
    let light_sky = args.iter().any(|a| a == "--light");
    args.retain(|a| a != "--night" && a != "--light");
    let options = DrawOptions {
        night,
        full_sky: !light_sky,
        ..DrawOptions::default()
    };
    match args.first().map(String::as_str) {
        Some("ground") => {
            let cam_x: i32 = args[1].parse().expect("cam_x");
            let cam_y: i32 = args[2].parse().expect("cam_y");
            let (w, h) = size(&args, 4);
            let mut world = level::world();
            if open {
                world.map.set_switch(DOOR_RED_CHANNEL, false);
                world.map.set_switch(DOOR_BLUE_CHANNEL, false);
            }
            let g = Ground {
                data: &world.map.data,
                w: world.map.w,
                h: world.map.h,
                red_closed: world.map.switch(DOOR_RED_CHANNEL),
                blue_closed: world.map.switch(DOOR_BLUE_CHANNEL),
            };
            let mut frame = vec![0xEEu8; w * h];
            draw_ground(&mut frame, w, h, &g, cam_x, cam_y);
            if night {
                oblique::draw_night(&mut frame, w, h, &g, cam_x, cam_y);
                let mut lights = light::Lights::new();
                oblique::night_lights(&mut lights, &g, (cam_x, cam_y), (w, h), 0);
                let (cols, rows) = light::grid_size(w, h);
                let mut add = vec![0u16; cols * rows];
                light::add_lights(
                    &mut add,
                    cols,
                    rows,
                    lights.select(),
                    (cam_x * light::SUB, cam_y * light::SUB),
                );
                light::shade(&mut frame, w, h, &add, oblique::NIGHT_AMBIENT);
            }
            write_ppm(&args[3], &frame, w, h, &options);
        }
        Some("bench") => bench(&args),
        Some("script") => {
            let mut world = level::world();
            let mut frame = vec![0u8; 320 * 200];
            let mut prev = Buttons::NONE;
            for &(frames, now) in SLICE_SCRIPT {
                for _ in 0..frames {
                    step(&mut world, InputFrame::from_transition(prev, now));
                    prev = now;
                    draw_tick(&mut frame, &world, &options);
                }
            }
            println!("{}", frame.iter().map(|&p| u64::from(p)).sum::<u64>());
        }
        Some("all") => {
            for tick in (0..=1700).step_by(100) {
                let world = world_at(tick);
                let mut frame = vec![0xEEu8; 320 * 200];
                draw_slice_with(&mut frame, 320, 200, &world, tick as u32, &options);
                write_ppm(
                    &format!("{}-{tick}.ppm", args[1]),
                    &frame,
                    320,
                    200,
                    &options,
                );
            }
        }
        Some(t) => {
            let tick: u64 = t.parse().expect("tick");
            let (w, h) = size(&args, 2);
            let world = world_at(tick);
            let mut frame = vec![0xEEu8; w * h];
            draw_slice_with(&mut frame, w, h, &world, tick as u32, &options);
            write_ppm(&args[1], &frame, w, h, &options);
        }
        None => eprintln!(
            "usage: frame <tick> <out.ppm> [w h] | ground <cam_x> <cam_y> <out.ppm> [w h] | all <stem>"
        ),
    }
}
