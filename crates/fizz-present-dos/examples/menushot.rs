// Prints the digests of the Options and Paused screens over the slice's first frame, and optionally writes them as pictures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `tools/check-dos.sh` compares this output with the `MENUSHOT` lines of `FIZZ2.EXE /MENUSHOT`, so the DOS build is proved to draw the
// menu pixels the portable code does. Run it with `cargo run -p fizz-present-dos --example menushot`. Given a directory, it also
// writes `options.ppm` and `paused.ppm` there in the DOS palette (ImageMagick turns them into PNG files).
use fizz_present_dos::menu::{MenuView, shot_options, shot_paused};
use fizz_present_dos::{Frame, HEIGHT, WIDTH, dos_palette, frame_digest, render_slice};
use std::io::Write;

fn picture(frame: &mut Frame) {
    render_slice(&fizz_ep2::level::world(), frame);
}

fn save(dir: &str, name: &str, frame: &Frame) -> std::io::Result<()> {
    let palette = dos_palette();
    let mut out = std::io::BufWriter::new(std::fs::File::create(format!("{dir}/{name}.ppm"))?);
    write!(out, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for &p in frame.pixels() {
        let c = palette[usize::from(p)];
        // A DAC value is 6 bits; 255 / 63 spreads it over eight.
        out.write_all(&[c.r, c.g, c.b].map(|v| (u16::from(v) * 255 / 63) as u8))?;
    }
    out.flush()
}

fn main() -> std::io::Result<()> {
    let dir = std::env::args().nth(1);
    let mut view = Box::new(MenuView::new());
    let mut frame = Box::new(Frame::new());
    picture(&mut frame);
    shot_options(&mut view, &mut frame);
    println!("MENUSHOT options {:016x}", frame_digest(&frame));
    if let Some(dir) = &dir {
        save(dir, "options", &frame)?;
    }
    picture(&mut frame);
    shot_paused(&mut view, &mut frame);
    println!("MENUSHOT paused {:016x}", frame_digest(&frame));
    if let Some(dir) = &dir {
        save(dir, "paused", &frame)?;
    }
    Ok(())
}
