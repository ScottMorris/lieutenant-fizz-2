// Prints the check screen's cells as `CELLROW` lines for the probe values on standard input, for `tools/check-dos.sh`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: cargo run -q -p fizz-machine --example cells-hex < transcript
//
// The input is the `PROBE <key> <value>` lines `FIZZ2.EXE /CHECKCELLS` prints (any other line is ignored). The screen is built with
// the default settings, as the program builds it. The output is the 25 `CELLROW` lines the program prints for the bytes it reads back
// from video memory, so a difference means the screen the adapter holds is not the one the pure builder made.

use fizz_machine::{CELLS, Probe, Settings, cells, parse_probe, resolve, write_cells_hex};
use std::io::Read;

fn main() {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .expect("read standard input");
    let probe: Probe = match parse_probe(&text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cells-hex: {e}");
            std::process::exit(1);
        }
    };
    let resolved = resolve(&Settings::default(), &probe);
    let mut screen = [0u16; CELLS];
    cells(&probe, &resolved, &mut screen);
    let mut out = String::new();
    write_cells_hex(&screen, &mut out).expect("format");
    print!("{out}");
}
