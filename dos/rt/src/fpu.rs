// The x87 control word: read it, and set it to 53-bit precision.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from fizz-2-dos-poc1 crates/fizz2-dos-rt/src/fpu.rs (itself from Desk spikes/d0-rust-dos/rt/src/fpu.rs).
// Changes here: only `control_word` and `set_double_precision` are kept. The x87 `fsin`, `fcos`, `fpatan` and `fsqrt` helpers are
// deliberately gone: Fizz 2 takes transcendental functions from a software `libm` so that its results do not depend on the host
// FPU's microcode.
//
// `_start` runs FNINIT, which leaves the control word at 037Fh: 64-bit mantissa (extended precision), round to nearest, all
// exceptions masked. The simulation's numeric model is `f64` with IEEE rounding, so a program sets 027Fh (53-bit mantissa) before
// its first tick. Interrupt handlers never touch the x87, so the control word is the program's alone.
use core::arch::asm;

/// The control word as it is now.
pub fn control_word() -> u16 {
    let mut cw: u16 = 0;
    // SAFETY: FNSTCW stores two bytes through the pointer.
    unsafe { asm!("fnstcw word ptr [{p}]", p = in(reg) &mut cw, options(nostack)) };
    cw
}

/// Sets the precision control to 53 bits (control word 027Fh), which makes `f64` arithmetic round as IEEE doubles do.
pub fn set_double_precision() {
    let cw: u16 = 0x027F;
    // SAFETY: FLDCW loads two bytes from the pointer.
    unsafe { asm!("fldcw word ptr [{p}]", p = in(reg) &cw, options(nostack)) };
}
