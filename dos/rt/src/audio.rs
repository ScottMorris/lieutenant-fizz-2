// The audio service: sound effects stepped from the 560 Hz timer interrupt, on an OPL2, an OPL3 or the PC speaker.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: the design (a PIT at divisor 2131, an OPL register write with the 6 and 35 status reads, the speaker driven through
// PIT channel 2 and port 0x61 every fourth tick, requests handed to the handler and taken up there, silence on shutdown) follows Fizz 1's
// `m4/c/sound.c` (fizz-dos, commit `221dd6c`, read only). The code is written for this repository; the records, the pan and the OR rule
// come from `fizz-sfx` and `fizz-machine`. Not verified on real hardware: the sound itself cannot be heard in a headless run.
//
// What it does. `start` hooks IRQ 0 at 560 Hz (`timer::hook_560hz`, so the BIOS clock keeps its 18.2 Hz through the same accumulator),
// puts the FM chip in the state the plan names and installs `service` as the timer's tick hook. The game calls `cluck` for each
// `Event::Cluck` together with the `Placement` of the chicken on screen (`fizz_sfx::place`, worked out by the program, not here); that
// stores the variant and the three attenuations in one atomic, and the next tick takes it up:
//
// - effects on channel 5 (`FxPlan::Ch5`): an `OplPlayer::mono` plays the variant's records, one `tick` per interrupt, with the carrier's
//   total level raised by the distance attenuation (no pan, no extra writes). On an OPL3 in its own mode (the plan has music on the
//   Sound Blaster) each bank-0 write goes through `fizz_machine::route`, which sets the C0-C8 output bits so the mono voice sounds in
//   both speakers.
// - effects on the OPL3 pair (`FxPlan::Ch14`, an OPL3 with the Sound Blaster chosen): an `OplPair` plays the same records on channel 5
//   (bank 0, output bit 0x10, the left) and channel 14 (bank 1, 0x20, the right), each at the carrier level the placement gives that
//   side. The pair's writes do not go through `route`: each channel has exactly its own output bit. The OPL3 mode was switched on
//   by `start`, so the effect adds no enable write. Two channels double the effect's register writes.
// - the PC speaker (`FxPlan::Speaker`): a `SpeakerPlayer` steps every fourth tick (140 Hz) and sets PIT channel 2's divisor or closes the gate.
// - `FxPlan::Off`, or no card and no speaker choice: the service runs and does nothing, so the clock and the simulation are the same.
//
// Music is not played yet: the plan's music field is read only to know whether the chip runs in OPL3 mode, and channels 5 and 14 are the
// effects' whatever it holds. The service touches no DOS service, allocates nothing and uses no x87.
//
// Ownership. The service's state is touched by the interrupt handler alone (it never nests), so it needs no lock; the program reaches it
// through two atomics only: the pending cluck (`xchg`, no `cmpxchg`, which DOSBox's 486 refuses) and the counters. `stop` clears the
// active flag first, so a tick that arrives while the chip is being put back does nothing.
//
// Reset (`stop`, on the clean-up list, so also in the panic handler): every channel keyed off in both banks, register 0x105 back to 0 so an
// OPL3 leaves OPL3 mode, the speaker gate closed.
use crate::{cleanup, irq, port, timer};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use fizz_machine::{FxPlan, OplMode, Resolved, Stream, route};
use fizz_sfx::place::Placement;
use fizz_sfx::{EFFECT_CHANNEL, OplPair, OplPlayer, SpeakerPlayer, SpeakerStep};

/// Register and data ports of the first register bank, and of the second (OPL3 only).
const ADDR_BANK0: u16 = 0x388;
const DATA_BANK0: u16 = 0x389;
const ADDR_BANK1: u16 = 0x38A;
const DATA_BANK1: u16 = 0x38B;
const PIT_COMMAND: u16 = 0x43;
const PIT_CHANNEL2: u16 = 0x42;
const SPEAKER_PORT: u16 = 0x61;

/// Whether the service runs. Cleared first by `stop`.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// The chip's mode for `stop`, which must not read the interrupt's state: 0 unused, 1 OPL2, 2 OPL3.
static CHIP: AtomicU8 = AtomicU8::new(0);
/// The cluck the program wants played; 0 for none. Taken (swapped to 0) by the handler. Bits 0 to 7 are `variant + 1`, then three
/// 6-bit attenuations (0.75 dB steps): bits 8 to 13 the left channel, 14 to 19 the right and 20 to 25 the single channel.
static PENDING: AtomicU32 = AtomicU32::new(0);
/// OPL register writes made since `start`, the enable and key-off writes included. Written by the handler (and `start`/`stop`) only.
static WRITES: AtomicU32 = AtomicU32::new(0);
/// Speaker steps made since `start`: a divisor set or a gate closed. Written by the handler only.
static SPEAKER_STEPS: AtomicU32 = AtomicU32::new(0);

/// The interrupt handler's state, behind a cell the handler alone touches.
struct Service {
    opl: OplMode,
    fx: FxPlan,
    opl_player: Option<OplPlayer>,
    pair: Option<OplPair>,
    speaker: Option<SpeakerPlayer>,
    ticks: u32,
}

struct IrqCell(UnsafeCell<Service>);

// SAFETY: the program is single-threaded and the interrupt handler never nests; the cell is written by `start` before the tick hook
// that reads it is set, and read and written by `service` alone after that.
unsafe impl Sync for IrqCell {}

static SERVICE: IrqCell = IrqCell(UnsafeCell::new(Service {
    opl: OplMode::None,
    fx: FxPlan::Off,
    opl_player: None,
    pair: None,
    speaker: None,
    ticks: 0,
}));

/// One OPL register write: the register port, six status reads, the data port, then 35 status reads (the chip's settling times).
fn opl_write(bank: u8, reg: u8, val: u8) {
    let (addr, data) = if bank == 0 {
        (ADDR_BANK0, DATA_BANK0)
    } else {
        (ADDR_BANK1, DATA_BANK1)
    };
    port::outb(addr, reg);
    for _ in 0..6 {
        let _ = port::inb(ADDR_BANK0);
    }
    port::outb(data, val);
    for _ in 0..35 {
        let _ = port::inb(ADDR_BANK0);
    }
    WRITES.store(WRITES.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
}

/// Keys off the nine channels of `bank` and, for bank 0, switches the rhythm section off.
fn key_off_bank(bank: u8) {
    for ch in 0..9 {
        opl_write(bank, 0xB0 + ch, 0);
    }
    if bank == 0 {
        opl_write(0, 0xBD, 0);
    }
}

fn speaker_off() {
    port::outb(SPEAKER_PORT, port::inb(SPEAKER_PORT) & 0xFC);
}

fn speaker_set(divisor: u16) {
    // Channel 2, lobyte/hibyte, mode 3 (square wave), binary.
    port::outb(PIT_COMMAND, 0xB6);
    port::outb(PIT_CHANNEL2, divisor as u8);
    port::outb(PIT_CHANNEL2, (divisor >> 8) as u8);
    port::outb(SPEAKER_PORT, port::inb(SPEAKER_PORT) | 3);
}

impl Service {
    /// Starts the cluck `variant` on the plan's device, replacing one still playing.
    fn begin(&mut self, variant: usize, left: u8, right: u8, mono: u8) {
        match self.fx {
            FxPlan::Off => {}
            FxPlan::Ch5 => {
                if self.opl_player.is_some() {
                    opl_write(0, 0xB0 + EFFECT_CHANNEL, 0);
                }
                self.opl_player = Some(OplPlayer::mono(variant, mono));
            }
            FxPlan::Ch14 => {
                if self.pair.is_some() {
                    opl_write(0, 0xB0 + EFFECT_CHANNEL, 0);
                    opl_write(1, 0xB0 + EFFECT_CHANNEL, 0);
                }
                self.pair = Some(OplPair::same(variant, left, right));
            }
            FxPlan::Speaker => self.speaker = Some(SpeakerPlayer::new(variant)),
        }
    }

    fn tick(&mut self) {
        let pending = PENDING.swap(0, Ordering::Relaxed);
        if pending != 0 {
            self.begin(
                ((pending & 0xff) - 1) as usize,
                ((pending >> 8) & 0x3f) as u8,
                ((pending >> 14) & 0x3f) as u8,
                ((pending >> 20) & 0x3f) as u8,
            );
        }
        let opl = self.opl;
        if let Some(player) = self.opl_player.as_mut() {
            let more = player.tick(&mut |bank, reg, val| opl_write(bank, reg, route(opl, Stream::Opl2, bank, reg, val)));
            if !more {
                self.opl_player = None;
            }
        }
        if let Some(pair) = self.pair.as_mut()
            && !pair.tick(&mut |bank, reg, val| opl_write(bank, reg, val))
        {
            self.pair = None;
        }
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks & 3 == 0
            && let Some(player) = self.speaker.as_mut()
        {
            match player.tick() {
                Some(SpeakerStep::Divisor(d)) => speaker_set(d),
                Some(SpeakerStep::Off) => speaker_off(),
                None => {
                    self.speaker = None;
                    return;
                }
            }
            SPEAKER_STEPS.store(SPEAKER_STEPS.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
        }
    }
}

/// The timer's tick hook: runs in the interrupt, once per 560 Hz tick.
fn service() {
    if !ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    // SAFETY: see `IrqCell`; this is the only reader after `start`.
    let state = unsafe { &mut *SERVICE.0.get() };
    state.tick();
}

/// Hooks the timer at 560 Hz and starts the audio service for `plan`: the chip is keyed off (and put in OPL3 mode when the plan
/// uses it), and `cluck` then plays on the device the plan names. With no device the service still runs, silently, so the clock is the same.
/// `stop` undoes it, and so do the exit and the panic handler. Call it after the probes: their waits need the PIT at the BIOS rate.
pub fn start(plan: &Resolved) -> Result<(), irq::Error> {
    // SAFETY: no tick hook is set yet, so the handler is not reading the cell.
    unsafe {
        *SERVICE.0.get() = Service {
            opl: plan.opl,
            fx: plan.fx,
            opl_player: None,
            pair: None,
            speaker: None,
            ticks: 0,
        }
    };
    PENDING.store(0, Ordering::Relaxed);
    WRITES.store(0, Ordering::Relaxed);
    SPEAKER_STEPS.store(0, Ordering::Relaxed);
    timer::hook_560hz()?;
    if !cleanup::register(stop) {
        timer::unhook();
        return Err(irq::Error::CleanupFull);
    }
    timer::set_tick_hook(Some(service));
    CHIP.store(
        match plan.opl {
            OplMode::None => 0,
            OplMode::Opl2 => 1,
            OplMode::Opl3 => 2,
        },
        Ordering::Relaxed,
    );
    // The handler does nothing until `ACTIVE`, so the long writes below cannot race it.
    if plan.opl != OplMode::None {
        if plan.opl == OplMode::Opl3 {
            // OPL3 mode on (register 0x105 bit 0), every channel a two-operator one (0x104).
            opl_write(1, 0x05, 0x01);
            opl_write(1, 0x04, 0x00);
            key_off_bank(1);
        }
        key_off_bank(0);
    }
    ACTIVE.store(true, Ordering::Release);
    Ok(())
}

/// Plays cluck variant `variant` (an `Event::Cluck`'s) from where `placement` puts its chicken, on the plan's device at the next tick,
/// replacing one still playing. Several calls before a tick play the last. A no-op when the service is not running or the plan has no
/// device. The OPL3 pair uses the placement's two sides, a single OPL channel its distance, and the speaker neither (it has no volume).
pub fn cluck(variant: u8, placement: &Placement) {
    if ACTIVE.load(Ordering::Relaxed) {
        let packed = (u32::from(variant) + 1)
            | (u32::from(placement.left_attenuation()) << 8)
            | (u32::from(placement.right_attenuation()) << 14)
            | (u32::from(placement.mono_attenuation()) << 20);
        PENDING.store(packed, Ordering::Relaxed);
    }
}

/// Silences the chip and the speaker and stops the service. Safe to call twice; the timer is left hooked for `timer::unhook`.
pub fn stop() {
    if !ACTIVE.swap(false, Ordering::AcqRel) {
        return;
    }
    timer::set_tick_hook(None);
    match CHIP.load(Ordering::Relaxed) {
        0 => {}
        mode => {
            key_off_bank(0);
            if mode == 2 {
                key_off_bank(1);
                // Back to OPL2 mode, so whatever runs next sees a chip in its power-up state.
                opl_write(1, 0x05, 0x00);
            }
        }
    }
    speaker_off();
    cleanup::forget(stop);
}

/// OPL register writes made since `start`.
pub fn register_writes() -> u32 {
    WRITES.load(Ordering::Relaxed)
}

/// Speaker steps made since `start`.
pub fn speaker_steps() -> u32 {
    SPEAKER_STEPS.load(Ordering::Relaxed)
}
