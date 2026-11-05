// Prints the digests of the slice picture at the ticks FIZZ2.EXE /BENCH samples, from the portable code on the host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `tools/check-dos.sh` compares this output with the `BENCH FRAME` lines of the DOS build, and the `BENCH NIGHT FRAME` and
// `BENCH NIGHT FULL FRAME` lines with those of its night passes (the second with the light pass), so a DOS-only drawing routine
// that draws a different pixel is a failed check. Run it with `cargo run -p fizz-present-dos --example frames`.
use fizz_ep2::{SLICE_SCRIPT, level, step};
use fizz_present_dos::{
    DIGEST_TICKS, DrawOptions, Frame, HEIGHT, ViewFx, WIDTH, dos_default_options, frame_digest,
    render_slice_with,
};
use fizz_sim::{Buttons, InputFrame};

fn main() {
    // The options FIZZ2.EXE /BENCH draws with when FIZZ2.CFG holds the defaults, as tools/check-dos.sh runs it, then its night pass.
    let day = dos_default_options();
    play(&day, "BENCH FRAME");
    let night = DrawOptions { night: true, ..day };
    play(&night, "BENCH NIGHT FRAME");
    play(
        &DrawOptions {
            full_sky: true,
            ..night
        },
        "BENCH NIGHT FULL FRAME",
    );
}

fn play(options: &DrawOptions, label: &str) {
    let options = *options;
    let mut world = level::world();
    let mut frame = Frame::new();
    let mut puffs = ViewFx::new();
    let mut prev = Buttons::NONE;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            step(&mut world, InputFrame::from_transition(prev, now));
            prev = now;
            puffs.observe(&world, WIDTH, HEIGHT, &options);
            if DIGEST_TICKS.contains(&world.tick.0) {
                render_slice_with(&world, &mut frame, &options, &puffs);
                println!("{label} {} {:016x}", world.tick.0, frame_digest(&frame));
            }
        }
    }
}
