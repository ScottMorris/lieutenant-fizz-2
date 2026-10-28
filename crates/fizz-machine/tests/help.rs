// Compares the help screen with its golden text, and regenerates it with `BLESS=1`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_machine::switches::write_help;

const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/help.txt");

#[test]
fn help_text_is_the_golden_text() {
    let mut text = String::new();
    write_help(&mut text).unwrap();
    if std::env::var_os("BLESS").is_some() {
        std::fs::write(FILE, &text).unwrap();
    }
    let want = std::fs::read_to_string(FILE).expect("tests/help.txt (BLESS=1 writes it)");
    assert_eq!(text, want.replace("\r\n", "\n"));
}
