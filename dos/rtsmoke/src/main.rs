// Smoke test for the DOS runtime: FPU control word, the 60 Hz timer with its BIOS chain, vector restore and panic clean-up.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository; the shape follows Desk spikes/d0-rust-dos/progs/src/panicky.rs.
//
// Prints, in order: `FPU CW 027F`, then (after hooking IRQ 0 at 60 Hz and spinning to 600 ticks) `TICKS 600 BIOS <n>`, where n is
// how far the BIOS tick counter moved (about 182, because 600 ticks is ten seconds), then `VECTORS RESTORED yes`. With `/PANIC`
// the program hooks IRQ 0 and panics instead: the panic handler runs the clean-up list and exits with status 3.
#![no_std]
#![no_main]

extern crate alloc;

use fizz_dos_rt::{dpmi, entry, env, fpu, irq, println, timer};

entry!(main);

const TARGET_TICKS: u32 = 600;

fn main() -> i32 {
    fpu::set_double_precision();
    println!("FPU CW {:04X}", fpu::control_word());

    let panic_requested = env::args().iter().any(|a| a.eq_ignore_ascii_case("/PANIC"));
    let vector = irq::vector_of(0);
    let before = dpmi::get_pm_vector(vector).ok();

    if let Err(e) = timer::hook_60hz() {
        println!("HOOK FAILED {:?}", e);
        return 1;
    }
    if panic_requested {
        println!("HOOKED, PANICKING");
        panic!("requested by /PANIC");
    }

    let bios_start = timer::bios_ticks();
    while timer::ticks() < TARGET_TICKS {
        // A volatile spin: HLT is privileged under CWSDPMI, and `ticks` reads the counter with an atomic load each time round.
    }
    let ticks = timer::ticks();
    let bios_end = timer::bios_ticks();
    timer::unhook();

    println!("TICKS {} BIOS {}", ticks.min(TARGET_TICKS), bios_end.wrapping_sub(bios_start));
    let after = dpmi::get_pm_vector(vector).ok();
    println!(
        "VECTORS RESTORED {}",
        if before.is_some() && before == after { "yes" } else { "no" }
    );
    0
}
