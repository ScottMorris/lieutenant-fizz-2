// IRQ 0 at 60 Hz, with the BIOS clock still advancing at 18.2 Hz.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from fizz-2-dos-poc1 crates/fizz2-dos-rt/src/timer.rs (itself from Desk spikes/d0-rust-dos/rt/src/timer.rs).
// Changes here: the divisor is 19886 (60 Hz) instead of 2131 (560 Hz); the hand-written IRQ 0 stub and the music callback are gone,
// the tick and the accumulator are plain Rust in a handler of the shared `irq` registry; `hook_60hz`, `ticks` and `bios_ticks` are
// the public surface.
//
// The PIT's input clock is 1,193,182 Hz, so divisor 19886 gives 60.0011 interrupts a second. The BIOS clock (the tick counter at
// 0040:006C, which DOS reads for the time of day and the disk driver for timeouts) expects 65536 input clocks between ticks, 18.2065
// Hz. A 16-bit accumulator reconciles the two: each interrupt adds 19886 to it, and when it reaches 65536 it wraps and that
// interrupt is passed to the BIOS handler, which counts it and sends the end-of-interrupt. Every other interrupt is acknowledged
// here. Over time the BIOS handler runs once for every 65536 input clocks, so the DOS clock keeps correct time.
//
// There is no `HLT` anywhere: it is privileged under CWSDPMI. Waiting is a volatile spin on `ticks`.
use crate::{cleanup, dpmi, irq, port};
use core::ptr::read_volatile;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const PIT_COMMAND: u16 = 0x43;
const PIT_CHANNEL0: u16 = 0x40;
/// 1,193,182 / 19886 = 60.0011 Hz.
pub const PIT_DIVISOR_60HZ: u16 = 19886;
const IRQ0: u8 = 0;

/// Interrupts since `hook_60hz`. Written by the handler only.
static TICKS: AtomicU32 = AtomicU32::new(0);
/// Input clocks owed to the BIOS clock, below 65536. Written by the handler only (and reset while unhooked).
static ACC: AtomicU32 = AtomicU32::new(0);
static HOOKED: AtomicBool = AtomicBool::new(false);

fn on_irq0() -> irq::Action {
    TICKS.store(TICKS.load(Ordering::Relaxed).wrapping_add(1), Ordering::Release);
    let acc = ACC.load(Ordering::Relaxed) + u32::from(PIT_DIVISOR_60HZ);
    if acc >= 65536 {
        ACC.store(acc - 65536, Ordering::Relaxed);
        irq::Action::Chain
    } else {
        ACC.store(acc, Ordering::Relaxed);
        irq::Action::Eoi
    }
}

/// Programs PIT channel 0 as a rate generator (mode 2) with `divisor`.
fn program_pit(command: u8, divisor: u16) {
    port::outb(PIT_COMMAND, command);
    port::outb(PIT_CHANNEL0, divisor as u8);
    port::outb(PIT_CHANNEL0, (divisor >> 8) as u8);
}

/// Hooks IRQ 0 and sets the PIT to 60 Hz. The vector and the divisor are put back by `unhook`, and from the clean-up list at exit
/// and in the panic handler.
pub fn hook_60hz() -> Result<(), irq::Error> {
    if HOOKED.load(Ordering::Relaxed) {
        return Err(irq::Error::Busy);
    }
    TICKS.store(0, Ordering::Relaxed);
    ACC.store(0, Ordering::Relaxed);
    irq::install(IRQ0, on_irq0)?;
    if !cleanup::register(unhook) {
        let _ = irq::uninstall(IRQ0);
        return Err(irq::Error::CleanupFull);
    }
    HOOKED.store(true, Ordering::Relaxed);
    // Mode 2 (rate generator), lobyte/hibyte access, binary count.
    program_pit(0x34, PIT_DIVISOR_60HZ);
    Ok(())
}

/// Puts the previous IRQ 0 handler and the BIOS divisor back. Safe to call twice.
pub fn unhook() {
    if !HOOKED.swap(false, Ordering::Relaxed) {
        return;
    }
    // The vector goes first: the BIOS handler counts every interrupt it is given, so it must not see 60 Hz for longer than needed.
    // Then mode 3 (square wave) with divisor 0, which is 65536, the BIOS default.
    let _ = irq::uninstall(IRQ0);
    program_pit(0x36, 0);
    cleanup::forget(unhook);
}

/// Whether the timer hook is installed.
pub fn is_hooked() -> bool {
    HOOKED.load(Ordering::Relaxed)
}

/// Interrupts counted since `hook_60hz`, at 60.0011 Hz. It wraps after about 2.3 years.
pub fn ticks() -> u32 {
    TICKS.load(Ordering::Acquire)
}

/// Input clocks since `hook_60hz`, at 1,193,182 Hz (0.838 microseconds each): the interrupt count times the divisor, plus how far
/// channel 0 has counted down in the current period. This is the 486's stopwatch, which has no `rdtsc`.
///
/// The count is read with the latch command (a write of 0 to port 0x43 freezes channel 0's counter for two byte reads at 0x40).
/// An interrupt can land between the reads and the interrupt count; the count is then read again, so the pair always belongs
/// to one period. One more case remains: the counter has reloaded but IRQ 0 has not yet run, so `ticks` is a period behind.
/// That window is a few microseconds and rare, and the value then reads one period (16,667 us) too low, so a caller that
/// subtracts two readings must treat a result that goes backwards as that case and add `PIT_DIVISOR_60HZ`. A reading costs
/// two port reads and a port write, about 3 to 5 us on a 486.
pub fn clocks() -> u64 {
    loop {
        let before = ticks();
        port::outb(PIT_COMMAND, 0x00);
        let lo = port::inb(PIT_CHANNEL0);
        let hi = port::inb(PIT_CHANNEL0);
        if before == ticks() {
            let remaining = (u32::from(hi) << 8 | u32::from(lo)).min(u32::from(PIT_DIVISOR_60HZ));
            return u64::from(before) * u64::from(PIT_DIVISOR_60HZ) + u64::from(PIT_DIVISOR_60HZ) - u64::from(remaining);
        }
    }
}

/// The BIOS tick counter at 0040:006C, 18.2065 Hz.
pub fn bios_ticks() -> u32 {
    // SAFETY: conventional memory is mapped at linear 0 and the counter is dword-aligned; the BIOS updates it behind our back.
    unsafe { read_volatile(dpmi::linear_to_ptr::<u32>(0x46C)) }
}
