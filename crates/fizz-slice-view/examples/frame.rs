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
//   Add --open to a `ground` run to open both doors.
//
// The size defaults to 320 by 200. Convert with any PPM reader, for example `python3 -c "from PIL import Image; Image.open('f.ppm').save('f.png')"`.
use fizz_ep2::tiles::{DOOR_BLUE_CHANNEL, DOOR_RED_CHANNEL};
use fizz_ep2::{SLICE_SCRIPT, World, level, step};
use fizz_sim::{Buttons, InputFrame};
use fizz_slice_view::oblique::{Ground, draw_ground};
use fizz_slice_view::{DrawOptions, draw_slice_with};
use std::fs;

/// The sixteen EGA colours in palette order, as red, green and blue bytes.
const EGA: [[u8; 3]; 16] = [
    [0, 0, 0],
    [0, 0, 170],
    [0, 170, 0],
    [0, 170, 170],
    [170, 0, 0],
    [170, 0, 170],
    [170, 85, 0],
    [170, 170, 170],
    [85, 85, 85],
    [85, 85, 255],
    [85, 255, 85],
    [85, 255, 255],
    [255, 85, 85],
    [255, 85, 255],
    [255, 255, 85],
    [255, 255, 255],
];

fn write_ppm(path: &str, frame: &[u8], w: usize, h: usize) {
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for &p in &frame[..w * h] {
        out.extend_from_slice(&EGA[usize::from(p & 15)]);
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

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--open` leaves both doors open, to look at what they hide.
    let open = args.iter().any(|a| a == "--open");
    args.retain(|a| a != "--open");
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
            write_ppm(&args[3], &frame, w, h);
        }
        Some("bench") => {
            // Host timing of the ground against the flat tile pass it replaced, over camera positions down the map.
            let world = level::world();
            let g = Ground {
                data: &world.map.data,
                w: world.map.w,
                h: world.map.h,
                red_closed: true,
                blue_closed: true,
            };
            let cams: Vec<(i32, i32)> = (0..48).map(|i| (((i * 7) % 12) * 16, i * 12)).collect();
            let mut frame = vec![0u8; 320 * 200];
            let rounds = 40;
            let t = std::time::Instant::now();
            for _ in 0..rounds {
                for &(x, y) in &cams {
                    draw_ground(&mut frame, 320, 200, &g, x, y);
                    std::hint::black_box(&frame);
                }
            }
            let ground = t.elapsed().as_secs_f64() * 1e6 / f64::from(rounds * cams.len() as i32);
            let t = std::time::Instant::now();
            for _ in 0..rounds {
                for &(x, y) in &cams {
                    fizz_view::draw_tilemap_on_sky(
                        &mut frame,
                        320,
                        200,
                        x,
                        y,
                        32,
                        48,
                        9,
                        |tx, ty| {
                            let t = world.map.data[(ty * 32 + tx) as usize];
                            (t != 0).then_some(t)
                        },
                    );
                    std::hint::black_box(&frame);
                }
            }
            let flat = t.elapsed().as_secs_f64() * 1e6 / f64::from(rounds * cams.len() as i32);
            println!(
                "ground {ground:.1} us, flat tile pass {flat:.1} us, ratio {:.2}",
                ground / flat
            );
        }
        Some("all") => {
            for tick in (0..=1700).step_by(100) {
                let world = world_at(tick);
                let mut frame = vec![0xEEu8; 320 * 200];
                draw_slice_with(
                    &mut frame,
                    320,
                    200,
                    &world,
                    tick as u32,
                    &DrawOptions::default(),
                );
                write_ppm(&format!("{}-{tick}.ppm", args[1]), &frame, 320, 200);
            }
        }
        Some(t) => {
            let tick: u64 = t.parse().expect("tick");
            let (w, h) = size(&args, 2);
            let world = world_at(tick);
            let mut frame = vec![0xEEu8; w * h];
            draw_slice_with(
                &mut frame,
                w,
                h,
                &world,
                tick as u32,
                &DrawOptions::default(),
            );
            write_ppm(&args[1], &frame, w, h);
        }
        None => eprintln!(
            "usage: frame <tick> <out.ppm> [w h] | ground <cam_x> <cam_y> <out.ppm> [w h] | all <stem>"
        ),
    }
}
