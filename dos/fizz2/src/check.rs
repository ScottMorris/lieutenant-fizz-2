// `FIZZ2.EXE /CHECK` and `/CHECKCELLS`: the read-only start-up check screen, and its non-interactive read-back.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: the flow follows Fizz 1's text-mode system check (fizz-dos commit `221dd6c`); written for this repository.
//
// `/CHECK` probes the machine, resolves the sound plan, draws the 80x25 text screen the pure builder (`fizz_machine::cells`) makes from
// the two, and waits for a key: Esc leaves (exit status 0), Enter goes on to play. It runs before any timer or keyboard hook, because
// the probes' waits need the PIT at the BIOS rate and the keys are read through the BIOS. It shows and changes nothing else.
// It waits for a person, so no script or test may run it; `/CHECKCELLS` is the form they use.
//
// `/CHECKCELLS` does the same drawing, reads the 4,000 bytes back from video memory, puts the screen back to text mode and prints
// the probe lines and the bytes as `CELLROW` lines, which the host compares with its own builder for the same probe values.
use alloc::string::String;
use core::fmt::Write;
use fizz_dos_rt::{println, text};
use fizz_machine::{CELLS, Probe, Settings, cells, cells_hash, ready, resolve, write_cells_hex, write_probe};

/// What the person chose on the check screen.
pub enum Choice {
    /// Esc: leave, exit status 0.
    Quit,
    /// Enter: go on to play.
    Play,
}

/// Builds the screen for `machine` and the default sound settings.
fn build(machine: &Probe) -> [u16; CELLS] {
    let resolved = resolve(&Settings::default(), machine);
    let mut screen = [0u16; CELLS];
    cells(machine, &resolved, &mut screen);
    screen
}

/// `/CHECK`: see the module comment. Returns what the person chose, with the screen back in its start-up text mode.
pub fn check_mode() -> Choice {
    let machine = crate::machine::probe_machine();
    let screen = build(&machine);
    let mono = machine.video.is_text_mono();
    text::enter(mono);
    text::write_cells(&screen, mono);
    text::flush_keys();
    let choice = loop {
        match text::poll_key() {
            Some(key) if key.ascii == 0x1B => break Choice::Quit,
            // Enter does nothing while something the game needs is missing; the screen says what.
            Some(key) if key.ascii == 0x0D && ready(&machine) => break Choice::Play,
            _ => {}
        }
    };
    text::leave();
    choice
}

/// `/CHECKCELLS`: see the module comment.
pub fn cells_mode() -> i32 {
    let machine = crate::machine::probe_machine();
    let screen = build(&machine);
    let mono = machine.video.is_text_mono();
    text::enter(mono);
    text::write_cells(&screen, mono);
    let mut back = [0u16; CELLS];
    text::read_cells(&mut back, mono);
    text::leave();

    let mut out = String::new();
    let _ = write_probe(&machine, &mut out);
    let _ = write_cells_hex(&back, &mut out);
    let _ = writeln!(out, "CELLS {:016x}", cells_hash(&back));
    let _ = writeln!(out, "CELLS WRITTEN {:016x}", cells_hash(&screen));
    let _ = writeln!(out, "EXIT yes");
    for line in out.lines() {
        println!("{}", line);
    }
    0
}
