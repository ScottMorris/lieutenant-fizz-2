// Hardware probes for the start-up check: display adapter, mouse, joystick, FM chip, Sound Blaster DSP, x87, processor and memory.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: ports of the probes in Fizz 1's `m4/c/compat.c` and `m4/c/sound.c` (fizz-dos commit `221dd6c`), with `__dpmi_int`
// replaced by `dpmi::simulate_int`, `delay` by `pit::delay_us`, `uname` by the EFLAGS toggles and
// `_go32_dpmi_remaining_physical_memory` by `dpmi::free_memory_info`. The Sound Blaster DSP, the OPL3 and the two-chip tests are new.
// None of them is verified on real hardware; `docs/DOS-CHECK.md` lists what was seen in DOSBox and what was not.
//
// Every probe runs before the 60 Hz timer is hooked: the ones that wait use `pit::delay_us`, which refuses to run while the timer
// is hooked, and then they report the absence of the device. Every wait on a device is bounded, so a missing or misbehaving card
// costs a few milliseconds and never a hang. The values here are plain; `fizz-machine` gives them their meaning.
use crate::dpmi::{self, RealRegs};
use crate::{pit, port};
use core::arch::asm;

/// The display adapter, the best one present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Video {
    /// A VGA with a colour display
    Vga,
    /// A VGA with a monochrome display
    MonoVga,
    Ega,
    Cga,
    /// A monochrome text adapter: MDA or Hercules
    Mda,
}

/// What answered at the FM chip's ports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fm {
    None,
    Opl2,
    /// Two OPL2 chips: a Sound Blaster Pro 1
    DualOpl2,
    Opl3,
}

/// The processor family, from the EFLAGS bits that toggle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cpu {
    I386,
    I486,
    /// A Pentium or later: the ID flag toggles
    I586Plus,
}

/// A Sound Blaster DSP that answered its reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dsp {
    pub major: u8,
    pub minor: u8,
    pub base: u16,
}

fn int_call(int: u8, regs: &mut RealRegs) {
    // A host that refuses the call leaves the registers as they were, which every caller reads as "not there".
    let _ = dpmi::simulate_int(int, regs);
}

/// The display adapter. INT 10h AX=1A00h answers 1Ah on a VGA, with BL 8 for a colour display and 7 for a monochrome one. Without it,
/// the EGA information call (AH=12h, BL=10h) changes BL on an EGA. Failing both, the BIOS equipment word (INT 11h) bits 4 and 5 give
/// the start-up display: 01 or 10 a CGA, 11 an MDA or Hercules. MCGA (BL 0Bh or 0Ch) is not taken as VGA.
pub fn video() -> Video {
    let mut r = RealRegs {
        eax: 0x1A00,
        ..RealRegs::default()
    };
    int_call(0x10, &mut r);
    if r.eax as u8 == 0x1A {
        match r.ebx as u8 {
            8 => return Video::Vga,
            7 => return Video::MonoVga,
            _ => {}
        }
    }
    let mut r = RealRegs {
        eax: 0x1200,
        ebx: 0x10,
        ..RealRegs::default()
    };
    int_call(0x10, &mut r);
    if r.ebx as u8 != 0x10 {
        return Video::Ega;
    }
    let mut r = RealRegs::default();
    int_call(0x11, &mut r);
    if (r.eax >> 4) & 3 == 3 { Video::Mda } else { Video::Cga }
}

/// Whether a mouse driver answers INT 33h AX=0 with FFFFh. A null real-mode vector hangs DOS when called, so it is checked first.
pub fn mouse() -> bool {
    match dpmi::get_rm_vector(0x33) {
        Ok((segment, offset)) if segment != 0 || offset != 0 => {}
        _ => return false,
    }
    let mut r = RealRegs::default();
    int_call(0x33, &mut r);
    r.eax as u16 == 0xFFFF
}

/// Whether the game port answers: a bus with no card there reads FFh at 0x201.
pub fn joystick() -> bool {
    port::inb(0x201) != 0xFF
}

fn opl_write(base: u16, reg: u8, value: u8) -> bool {
    port::outb(base, reg);
    if !pit::delay_us(4) {
        return false;
    }
    port::outb(base + 1, value);
    pit::delay_us(24)
}

/// Fizz 1's timer test on the FM chip whose address port is `base`: reset the timers, read the status, start timer 1 and read the
/// status again. A chip reports no flags and then both timer flags (status AND E0 is 00, then C0). The timers are reset again
/// afterwards. Returns the status read after that last reset when the chip passed, so the caller can look at its low bits.
fn opl_timer_test(base: u16) -> Option<u8> {
    if !(opl_write(base, 4, 0x60) && opl_write(base, 4, 0x80)) {
        return None;
    }
    let s1 = port::inb(base);
    if !(opl_write(base, 2, 0xFF) && opl_write(base, 4, 0x21) && pit::delay_us(100)) {
        return None;
    }
    let s2 = port::inb(base);
    if !(opl_write(base, 4, 0x60) && opl_write(base, 4, 0x80)) {
        return None;
    }
    if (s1 & 0xE0) == 0 && (s2 & 0xE0) == 0xC0 {
        Some(port::inb(base))
    } else {
        None
    }
}

/// The FM chip at 0x388. After the timer test, an OPL2 reads 06h in the status register's low bits and an OPL3 reads 00h. On a card
/// with a Sound Blaster (`sb_base`), a second chip answering the timer test at `sb_base + 2` makes it two OPL2 chips (a Pro 1); that
/// port is not touched without a DSP, nor on an OPL3, where it reads FFh.
pub fn opl(sb_base: Option<u16>) -> Fm {
    let Some(status) = opl_timer_test(0x388) else {
        return Fm::None;
    };
    if status & 0x06 == 0 {
        return Fm::Opl3;
    }
    match sb_base {
        Some(base) if opl_timer_test(base + 2).is_some() => Fm::DualOpl2,
        _ => Fm::Opl2,
    }
}

/// Polls the status port until its bit 7 equals `want`, at most `tries` times, waiting 10 microseconds between reads.
fn poll(port_number: u16, want: bool, tries: u32) -> bool {
    for _ in 0..tries {
        if (port::inb(port_number) & 0x80 != 0) == want {
            return true;
        }
        pit::delay_us(10);
    }
    false
}

fn dsp_reset(base: u16) -> bool {
    port::outb(base + 6, 1);
    if !pit::delay_us(5) {
        return false;
    }
    port::outb(base + 6, 0);
    poll(base + 0xE, true, 200) && port::inb(base + 0xA) == 0xAA
}

fn dsp_read(base: u16) -> Option<u8> {
    poll(base + 0xE, true, 200).then(|| port::inb(base + 0xA))
}

fn dsp_version(base: u16) -> Option<(u8, u8)> {
    if !poll(base + 0xC, false, 200) {
        return None;
    }
    port::outb(base + 0xC, 0xE1);
    let major = dsp_read(base)?;
    let minor = dsp_read(base)?;
    Some((major, minor))
}

/// A base port the probe will touch: the Sound Blaster family sits at 210h to 280h in steps of 10h.
fn plausible_base(base: u16) -> bool {
    (0x210..=0x280).contains(&base) && base % 0x10 == 0
}

/// A Sound Blaster DSP: reset it (write 1 to base+6, wait, write 0), wait for data at base+0xE bit 7 and read 0AAh from base+0xA, then
/// ask for the version (command 0E1h). The bases tried are the `BLASTER` variable's `A` field when it is a plausible Sound Blaster
/// base, then 220h and 240h. The version is a label only; the chip type comes from `opl`.
pub fn dsp(blaster_base: Option<u16>) -> Option<Dsp> {
    let first = blaster_base.filter(|&b| plausible_base(b));
    let candidates = [first, Some(0x220), Some(0x240)];
    for (i, base) in candidates.iter().enumerate() {
        let Some(base) = *base else { continue };
        if candidates[..i].contains(&Some(base)) {
            continue;
        }
        if dsp_reset(base) {
            let (major, minor) = dsp_version(base).unwrap_or((0, 0));
            return Some(Dsp { major, minor, base });
        }
    }
    None
}

/// Whether an x87 is present: `fninit` then `fnstcw` leaves 037Fh in the control word. The control word is put back to 027Fh afterwards
/// when one was found, because `fninit` resets it.
pub fn fpu() -> bool {
    let mut cw: u16 = 0x5A5A;
    // SAFETY: FNINIT resets the x87 and FNSTCW stores two bytes through the pointer; without an x87 neither does anything.
    unsafe { asm!("fninit", "fnstcw word ptr [{p}]", p = in(reg) &mut cw, options(nostack)) };
    let present = (cw & 0x103F) == 0x003F;
    if present {
        crate::fpu::set_double_precision();
    }
    present
}

/// Whether the EFLAGS bits in `mask` can be changed: flip them, read the flags back and put the original flags back.
fn flags_toggle(mask: u32) -> bool {
    let changed: u32;
    // SAFETY: only the flags register and the stack's top are touched, and the original flags are restored before the block ends; the
    // bits in `mask` (AC and ID) are ones a program may change at any privilege level.
    unsafe {
        asm!(
            "pushfd",
            "pop {orig}",
            "mov {tmp}, {orig}",
            "xor {tmp}, {mask}",
            "push {tmp}",
            "popfd",
            "pushfd",
            "pop {tmp}",
            "push {orig}",
            "popfd",
            "xor {tmp}, {orig}",
            "and {tmp}, {mask}",
            orig = out(reg) _,
            tmp = out(reg) changed,
            mask = in(reg) mask,
        );
    }
    changed != 0
}

/// The processor family. The alignment-check flag (bit 18) toggles on a 486 and later, the CPUID flag (bit 21) on a Pentium and later.
/// CPUID itself is never executed.
pub fn cpu() -> Cpu {
    if !flags_toggle(1 << 18) {
        Cpu::I386
    } else if flags_toggle(1 << 21) {
        Cpu::I586Plus
    } else {
        Cpu::I486
    }
}

/// Free physical memory in kilobytes: the host's free pages times 4. `None` when the host does not say. A count larger than the
/// machine has (the program itself does not fit and the host has paged part of it out) means none and gives 0, as in Fizz 1.
pub fn free_memory() -> Option<u32> {
    let info = dpmi::free_memory_info().ok()?;
    let free = info.free_pages?;
    match info.total_pages {
        Some(total) if free > total => Some(0),
        _ => Some(free.saturating_mul(4)),
    }
}
