// Port I/O: the two instructions the timer, keyboard, PIC and VGA code need.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A DPMI client under CWSDPMI runs with the I/O permission bitmap open, so `in` and `out` work from the program and from interrupt
// handlers.
use core::arch::asm;

/// Writes a byte to an I/O port.
pub fn outb(port: u16, value: u8) {
    // SAFETY: an OUT to a port the PC defines (PIC, PIT, keyboard controller, VGA); it reads and writes no memory.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };
}

/// Reads a byte from an I/O port.
pub fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: an IN from a port the PC defines; it reads and writes no memory.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags)) };
    value
}
