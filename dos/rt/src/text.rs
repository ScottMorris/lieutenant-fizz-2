// The 80x25 text screen: set the mode, write and read back the character cells, read a key through the BIOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: replaces the `dosmemput` and `__dpmi_int` calls of Fizz 1's DJGPP preview code (fizz-dos commit `221dd6c`); written
// for this repository.
//
// A colour adapter keeps its text page at B8000h (mode 3), a monochrome one at B0000h (mode 7). Both lie in the first megabyte, which the
// DPMI host maps at linear 0, so a pointer into them is reached the way `vga::framebuffer` reaches A0000h. A cell is 16 bits: the
// character in the low byte and the attribute in the high byte, which is the order the adapter holds them in (low address first).
//
// The keyboard is read through the BIOS (INT 16h), not IRQ 1, so the check screen runs before any hook is installed.
use crate::dpmi::{self, RealRegs};
use crate::{cleanup, vga};
use core::sync::atomic::{AtomicU8, Ordering};

/// Cells on the screen, 80 columns by 25 rows.
pub const CELLS: usize = 2000;

const COLOUR_PAGE: u32 = 0xB8000;
const MONO_PAGE: u32 = 0xB0000;

/// The mode last set by `enter`: 3 or 7, or 0 when the screen is not ours.
static MODE: AtomicU8 = AtomicU8::new(0);

fn page(mono: bool) -> *mut u16 {
    dpmi::linear_to_ptr::<u16>(if mono { MONO_PAGE } else { COLOUR_PAGE })
}

fn int(n: u8, eax: u32, ecx: u32) -> RealRegs {
    let mut regs = RealRegs {
        eax,
        ecx,
        ..RealRegs::default()
    };
    // A host that refuses the call leaves the registers as they were, which reads as "nothing happened".
    let _ = dpmi::simulate_int(n, &mut regs);
    regs
}

/// Sets the 80x25 text mode (7 on a monochrome adapter, else 3), hides the cursor and registers the return with the clean-up list.
pub fn enter(mono: bool) {
    let mode = if mono { 7 } else { 3 };
    vga::set_mode(mode);
    // INT 10h AH=01h with CH bit 5 set hides the cursor.
    int(0x10, 0x0100, 0x2000);
    MODE.store(mode, Ordering::Relaxed);
    cleanup::register(leave);
}

/// Sets the text mode again, which clears the screen and shows the cursor. Safe to call twice.
pub fn leave() {
    let mode = MODE.swap(0, Ordering::Relaxed);
    if mode != 0 {
        vga::set_mode(mode);
        cleanup::forget(leave);
    }
}

/// Copies `cells` to the text page of the mode `enter` set.
pub fn write_cells(cells: &[u16; CELLS], mono: bool) {
    // SAFETY: B8000h and B0000h are in the first megabyte, mapped at linear 0 and reached through the 4 GiB data segment; each page
    // holds at least 4,000 bytes; `cells` is a separate allocation. Volatile because the adapter's memory is not ordinary memory.
    unsafe {
        let video = page(mono);
        for (i, &cell) in cells.iter().enumerate() {
            core::ptr::write_volatile(video.add(i), cell);
        }
    }
}

/// Reads the 2,000 cells back from the text page.
pub fn read_cells(out: &mut [u16; CELLS], mono: bool) {
    // SAFETY: as `write_cells`.
    unsafe {
        let video = page(mono);
        for (i, cell) in out.iter_mut().enumerate() {
            *cell = core::ptr::read_volatile(video.add(i));
        }
    }
}

/// A key press from the BIOS keyboard buffer: the ASCII code (0 for a function or cursor key) and the scancode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub ascii: u8,
    pub scancode: u8,
}

/// The next key in the BIOS buffer, if one is waiting (INT 16h AH=01h, then AH=00h to take it).
pub fn poll_key() -> Option<Key> {
    let peek = int(0x16, 0x0100, 0);
    // The zero flag is set when the buffer is empty.
    if peek.flags & 0x0040 != 0 {
        return None;
    }
    let key = int(0x16, 0x0000, 0);
    Some(Key {
        ascii: key.eax as u8,
        scancode: (key.eax >> 8) as u8,
    })
}

/// Throws away every key waiting in the BIOS buffer.
pub fn flush_keys() {
    while poll_key().is_some() {}
}
