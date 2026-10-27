// VGA mode 13h (320x200, 256 colours): the mode switch, the palette, the retrace and the frame buffer at A0000h.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from fizz-2-dos-poc1 crates/fizz2-dos-rt/src/vga.rs (itself from Desk spikes/d0-rust-dos/rt/src/vga.rs) and
// the frame-buffer window of `DosDisplay` in fizz-2-dos-poc1 crates/fizz2-dos-rt/src/hal.rs. Changes here: the test pattern and its
// checksum are gone; the mode switch goes through the DPMI real-mode call; `set_palette`, `wait_vretrace` and `present` are new.
//
// The mode switch is INT 10h, simulated in real mode by the host (DPMI 0300h). The frame buffer is in the first megabyte, which
// CWSDPMI maps at linear 0, so a pointer into it is A0000h minus the image base through the 4 GiB data segment (the same arithmetic
// DJGPP's "near pointer" trick uses). No selector, and no physical mapping (DPMI 0800h), is needed below 1 MiB.
use crate::dpmi::{self, RealRegs};
use crate::{cleanup, port};

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;
pub const PIXELS: usize = WIDTH * HEIGHT;

const DAC_WRITE_INDEX: u16 = 0x3C8;
const DAC_DATA: u16 = 0x3C9;
const INPUT_STATUS_1: u16 = 0x3DA;
/// Bit 3 of Input Status 1: the vertical retrace is in progress.
const VRETRACE: u8 = 0x08;
/// Upper bound on a poll of the status port, so a machine with no retrace signal costs a frame's time and not a hang.
const RETRACE_SPINS: u32 = 1_000_000;

fn int10(ax: u32) -> u32 {
    let mut regs = RealRegs {
        eax: ax,
        ..RealRegs::default()
    };
    // A host that refuses the call leaves the registers as they were, which reads as "no change".
    let _ = dpmi::simulate_int(0x10, &mut regs);
    regs.eax
}

/// The current video mode (INT 10h AH=0Fh).
pub fn get_mode() -> u8 {
    int10(0x0F00) as u8
}

/// Sets a video mode (INT 10h AH=00h).
pub fn set_mode(mode: u8) {
    int10(u32::from(mode));
}

/// Switches to mode 13h and registers the return to text mode with the clean-up list.
pub fn enter_mode13() {
    set_mode(0x13);
    cleanup::register(leave);
}

/// Returns to 80x25 text mode (mode 3). Safe to call twice.
pub fn leave() {
    if get_mode() != 3 {
        set_mode(3);
    }
    cleanup::forget(leave);
}

/// Writes `colours` to the DAC from entry `first` on. Each colour is red, green and blue at 6 bits, 0 to 63.
pub fn set_palette(first: u8, colours: &[[u8; 3]]) {
    port::outb(DAC_WRITE_INDEX, first);
    for c in colours {
        for &component in c {
            port::outb(DAC_DATA, component & 0x3F);
        }
    }
}

/// Waits for the start of the next vertical retrace: first until any retrace in progress ends, then until one begins.
pub fn wait_vretrace() {
    for _ in 0..RETRACE_SPINS {
        if port::inb(INPUT_STATUS_1) & VRETRACE == 0 {
            break;
        }
    }
    for _ in 0..RETRACE_SPINS {
        if port::inb(INPUT_STATUS_1) & VRETRACE != 0 {
            break;
        }
    }
}

/// The frame buffer, 320 x 200 bytes at linear A0000h.
pub fn framebuffer() -> *mut u8 {
    dpmi::linear_to_ptr::<u8>(0xA0000)
}

/// Copies a whole indexed frame to the screen.
pub fn present(pixels: &[u8; PIXELS]) {
    // SAFETY: A0000h to AFFFFh is in the first megabyte, mapped at linear 0 and reached through the 4 GiB data segment; it is video
    // memory for as long as a graphics mode is set, 64000 bytes fit in it, and `pixels` is a separate allocation.
    unsafe { core::ptr::copy_nonoverlapping(pixels.as_ptr(), framebuffer(), PIXELS) };
}
