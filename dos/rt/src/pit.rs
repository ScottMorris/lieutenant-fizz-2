// A busy-wait delay in microseconds from the PIT's channel 0 count, for the hardware probes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: replaces the `delay` call of DJGPP's `<dos.h>` that Fizz 1's `compat.c` and `sound.c` probes use (fizz-dos commit
// `221dd6c`). Written for this repository.
//
// The BIOS programs channel 0 as a square wave (mode 3) with a divisor of 65536, and in that mode the counter drops by two on each
// input clock, from 65536 to 0, and reloads. One count is therefore half an input clock: 0.419 microseconds. The delay latches the
// counter, adds up how far it has fallen between reads (modulo 65536, so a reload costs nothing) and stops when the sum reaches the
// target. It is valid only while channel 0 is at the BIOS rate: after `timer::hook_60hz` the counter runs in another mode with
// another period, so `delay_us` refuses to run while the timer is hooked. The wait is also bounded in the number of reads, so a
// counter that does not move (a broken emulator, a stopped timer) ends the wait early instead of hanging.
use crate::{port, timer};

const PIT_COMMAND: u16 = 0x43;
const PIT_CHANNEL0: u16 = 0x40;
/// Counts in a thousand microseconds in mode 3 at the BIOS rate: 1,193,182 input clocks a second, two counts per clock, rounded up.
const COUNTS_PER_MS: u32 = 2387;
/// The longest single delay, in microseconds. Longer requests are cut to it, which keeps the sum well under one sweep of 65536 counts.
pub const MAX_DELAY_US: u32 = 20_000;
/// Reads of the counter after which the wait gives up. A read costs at least a microsecond on any machine, so this is far beyond
/// `MAX_DELAY_US`.
const MAX_READS: u32 = 200_000;

/// Latches channel 0 and reads its count.
fn count() -> u16 {
    port::outb(PIT_COMMAND, 0x00);
    let lo = port::inb(PIT_CHANNEL0);
    let hi = port::inb(PIT_CHANNEL0);
    u16::from(hi) << 8 | u16::from(lo)
}

/// Waits at least `us` microseconds (at most `MAX_DELAY_US`) and returns `true`. Returns `false` at once, having waited for nothing,
/// while the 60 Hz timer is hooked, because the counter then runs at another rate.
pub fn delay_us(us: u32) -> bool {
    if timer::is_hooked() {
        return false;
    }
    let target = (us.min(MAX_DELAY_US) * COUNTS_PER_MS).div_ceil(1000);
    let mut last = count();
    let mut elapsed = 0u32;
    for _ in 0..MAX_READS {
        if elapsed >= target {
            break;
        }
        let now = count();
        elapsed += u32::from(last.wrapping_sub(now));
        last = now;
    }
    true
}
