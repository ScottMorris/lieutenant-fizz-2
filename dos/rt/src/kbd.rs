// IRQ 1: raw scancodes from port 60h into a ring, and the BIOS keyboard state put back at the end.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository from the policy of fizz-2-dos-poc1 crates/fizz2-dos-rt/src/bios_kbd.rs (itself from Desk
// spikes/d0-rust-dos/rt/src/bios_kbd.rs), without the Desk `Modifiers` type, and from the IRQ 1 handler in fizz-2-dos-poc1
// apps/demo-dos/src/main.rs.
//
// While the program owns IRQ 1 the BIOS keyboard handler does not run, so nothing keeps the shift flags in the BIOS data area
// (0040:0017 and 0040:0018) up to date, and DOS reads them when the program is over: a Shift whose release the BIOS never saw would
// stay "down" at the prompt. `hook` saves both bytes; `unhook` (and the clean-up list, at exit and in the panic handler) writes them
// back with the momentary bits cleared, keeping the lock bits (Scroll, Num and Caps Lock, Insert) the LEDs agree with. The handler
// pushes the raw Set 1 scancode, make and break, unchanged; decoding is the program's job.
use crate::irq::{self, Action};
use crate::ring::Ring;
use crate::{cleanup, dpmi, port};
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const IRQ1: u8 = 1;
const DATA_PORT: u16 = 0x60;
/// 0040:0017, the shift flags: bit 0 right Shift, 1 left Shift, 2 Ctrl, 3 Alt, 4 Scroll Lock, 5 Num Lock, 6 Caps Lock, 7 Insert.
const FLAGS: u32 = 0x417;
/// 0040:0018, more flags: bit 0 left Ctrl, 1 left Alt, 2 SysRq, 3 Pause, bits 4 to 7 "key is down" for the four toggles.
const FLAGS2: u32 = 0x418;
/// The bits of `FLAGS` that are locks (and Insert), which survive; the rest are keys held down.
const STATE_BITS: u8 = 0xF0;

static RING: Ring<64> = Ring::new();
static HOOKED: AtomicBool = AtomicBool::new(false);
/// The two flag bytes as found: `FLAGS` in the low byte, `FLAGS2` in the next.
static SAVED: AtomicU32 = AtomicU32::new(0);

fn on_irq1() -> Action {
    RING.push(port::inb(DATA_PORT));
    Action::Eoi
}

fn flag_byte(linear: u32) -> *mut u8 {
    dpmi::linear_to_ptr::<u8>(linear)
}

/// Takes IRQ 1: scancodes go to `pop_scancode` from now on, and the BIOS never sees them. Undone by `unhook`, and from the clean-up
/// list at exit and in the panic handler.
pub fn hook() -> Result<(), irq::Error> {
    if HOOKED.load(Ordering::Relaxed) {
        return Err(irq::Error::Busy);
    }
    // SAFETY: the BIOS data area is conventional memory, mapped at linear 0 by the host.
    let saved = unsafe { u32::from(read_volatile(flag_byte(FLAGS))) | u32::from(read_volatile(flag_byte(FLAGS2))) << 8 };
    SAVED.store(saved, Ordering::Relaxed);
    irq::install(IRQ1, on_irq1)?;
    if !cleanup::register(unhook) {
        let _ = irq::uninstall(IRQ1);
        return Err(irq::Error::CleanupFull);
    }
    HOOKED.store(true, Ordering::Relaxed);
    Ok(())
}

/// Gives IRQ 1 back to the BIOS and restores the keyboard flags. Safe to call twice.
pub fn unhook() {
    if !HOOKED.swap(false, Ordering::Relaxed) {
        return;
    }
    let _ = irq::uninstall(IRQ1);
    let saved = SAVED.load(Ordering::Relaxed);
    // SAFETY: as `hook`.
    unsafe {
        write_volatile(flag_byte(FLAGS), saved as u8 & STATE_BITS);
        // Every bit of this byte is momentary (a key down, or the Pause state).
        write_volatile(flag_byte(FLAGS2), 0);
    }
    cleanup::forget(unhook);
}

/// Whether the keyboard hook is installed.
pub fn is_hooked() -> bool {
    HOOKED.load(Ordering::Relaxed)
}

/// The oldest raw scancode the handler has queued, if any. A break code has bit 7 set; an extended key sends 0xE0 first.
pub fn pop_scancode() -> Option<u8> {
    RING.pop()
}

/// Scancodes dropped because the program did not read them fast enough.
pub fn dropped() -> u32 {
    RING.dropped()
}

/// The two BIOS flag bytes as found when the hook was installed (`FLAGS` in the low byte), for a program that wants the lock state.
pub fn saved_flags() -> u16 {
    SAVED.load(Ordering::Relaxed) as u16
}
