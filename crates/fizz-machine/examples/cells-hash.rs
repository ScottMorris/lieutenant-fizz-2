// Prints the hash of the check screen for the probe values on standard input, for `tools/check-dos.sh` to compare with `/PROBE`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: cargo run -q -p fizz-machine --example cells-hash < transcript
//
// The input is the `PROBE <key> <value>` lines `FIZZ2.EXE /PROBE` prints (any other line is ignored). The screen is built with the
// default settings, as `/PROBE` builds it, and the output is the `CELLS <hash>` line the program prints for the same values.

use fizz_machine::{CELLS, Probe, Settings, cells, cells_hash, parse_probe, resolve};
use std::io::Read;

fn main() {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .expect("read standard input");
    let probe: Probe = match parse_probe(&text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cells-hash: {e}");
            std::process::exit(1);
        }
    };
    let resolved = resolve(&Settings::default(), &probe);
    let mut screen = [0u16; CELLS];
    cells(&probe, &resolved, &mut screen);
    println!("CELLS {:016x}", cells_hash(&screen));
}
