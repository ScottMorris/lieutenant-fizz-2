// Writes the slice picture at the given ticks as PPM files, to look at the HUD: `hudshot <dir> [reduced] <tick>...`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// It plays the slice script with the view's effects fed after each step, as the DOS loop does, and saves `<dir>/t<tick>.ppm` in the
// DOS palette. With `--list-snacks` it prints the ticks at which the score changes, and where Ben takes a key.
use fizz_ep2::{SLICE_SCRIPT, WorldFlags, level, step};
use fizz_present_dos::{DrawOptions, Frame, HEIGHT, ViewFx, WIDTH, dos_palette, render_slice_with};
use fizz_sim::{Buttons, InputFrame};
use std::io::Write;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let list = args.first().is_some_and(|a| a == "--list");
    let dir = args.first().cloned().unwrap_or_default();
    let reduced = args.iter().any(|a| a == "reduced");
    let ticks: Vec<u64> = args.iter().filter_map(|a| a.parse().ok()).collect();
    let options = DrawOptions {
        shake: !reduced,
        ..DrawOptions::default()
    };
    let palette = dos_palette();
    let mut world = level::world();
    let mut frame = Frame::new();
    let mut fx = ViewFx::new();
    let mut prev = Buttons::NONE;
    let (mut score, mut flags) = (0, 0);
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            step(&mut world, InputFrame::from_transition(prev, now));
            prev = now;
            fx.observe(&world, WIDTH, HEIGHT, &options);
            if list {
                if world.score() != score || world.flags.0 != flags {
                    println!(
                        "tick {} score {} key red {} blue {} deaths {}",
                        world.tick.0,
                        world.score(),
                        world.flags.contains(WorldFlags::KEY_RED),
                        world.flags.contains(WorldFlags::KEY_BLUE),
                        world.deaths
                    );
                }
                score = world.score();
                flags = world.flags.0;
            }
            if ticks.contains(&world.tick.0) {
                render_slice_with(&world, &mut frame, &options, &fx);
                let mut out = std::io::BufWriter::new(std::fs::File::create(format!(
                    "{dir}/t{}.ppm",
                    world.tick.0
                ))?);
                write!(out, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
                for &p in frame.pixels() {
                    let c = palette[usize::from(p)];
                    out.write_all(&[c.r, c.g, c.b].map(|v| (u16::from(v) * 255 / 63) as u8))?;
                }
            }
        }
    }
    Ok(())
}
