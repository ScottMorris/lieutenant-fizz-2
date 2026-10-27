// Prints the slice replay transcript to standard output.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn main() {
    let mut transcript = String::new();
    fizz_ep2::replay(&mut transcript).expect("writing to a String cannot fail");
    print!("{transcript}");
}
