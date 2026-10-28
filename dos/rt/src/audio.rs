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
// Music (`fizz-music`): `music_start` asks for the level song, and the next tick builds a `MusicPlayer` for the arrangement the plan's
// `MusicPlan` names (`Opl2`: the OPL2 stream on channels 0-4 and 6-8; `Opl3`: the OPL3 stream with its stereo bits; `Off`: none). Each tick,
// in this order, the handler takes the pending requests, steps the music (the player's due writes go into a 16-write buffer, the most any
// tick of the stream carries), steps the effect (channel 5 or the pair), then the speaker on every fourth tick. Music and effects use
// different channels, so the order only matters for timing: both writers go through `opl_raw`, whose 6 and 35 status reads are the
// chip's settling times, one write after another in the one handler. Music writes go through `fizz_machine::route` with the stream's
// kind (an OPL2 stream is centred on an OPL3 chip, an OPL3 stream keeps its own output bits). The OPL3 enable (register 0x105 = 1) is
// written by `start` before the service goes active, so it always precedes the first music write; `MUSIC ORDER` reports the check. Music
// writes are not counted in `register_writes` (the `AUDIO` line counts effects and the start and stop key-offs); they have their own
// counters and an integer FNV-1a (`fizz_music::Fnv`) of each write's tick, bank, register and value as the stream authored it, for the
// `MUSIC` line. Channels 5 and 14 are never written by music. The PC speaker never plays music. The service touches no DOS service,
// allocates nothing and uses no x87.
//
// Measuring (`/BENCH /AUDIO`): `set_profile(true)` makes the handler latch PIT channel 0 on entry and exit and keep the sum, the largest
// and the count of the clocks between, and the writes of the busiest tick are always kept. DOSBox does not charge ISA port I/O the way a
// bus does, so those times are the emulator's; the exact write and status-read counts are the evidence, not the microseconds.
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
use fizz_machine::{FxPlan, MusicPlan, OplMode, Resolved, Stream, enable_writes, route};
use fizz_music::{Card, Fnv, MusicPlayer, Write};
use fizz_sfx::place::Placement;
use fizz_sfx::{EFFECT_CHANNEL, OplPair, OplPlayer, SpeakerPlayer, SpeakerStep};

/// Register and data ports of the first register bank, and of the second (OPL3 only).
const ADDR_BANK0: u16 = 0x388;
const DATA_BANK0: u16 = 0x389;
const ADDR_BANK1: u16 = 0x38A;
const DATA_BANK1: u16 = 0x38B;
const PIT_COMMAND: u16 = 0x43;
const PIT_CHANNEL0: u16 = 0x40;
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

/// Every OPL register write made since `start`, music included. Written by the handler (and `start`/`stop`) only.
static ALL_WRITES: AtomicU32 = AtomicU32::new(0);
/// Whether the last write to register 0x105 turned OPL3 mode on.
static OPL3_ON: AtomicBool = AtomicBool::new(false);
/// The arrangement the plan names: 0 none, 1 OPL2, 2 OPL3. Set by `start`.
static MUSIC_ARR: AtomicU8 = AtomicU8::new(0);
/// Set by `music_start`, taken (swapped to false) by the handler.
static MUSIC_REQ: AtomicBool = AtomicBool::new(false);
/// The music's own counters, published by the handler after each music tick: ticks stepped, writes made, loops jumped, the FNV-1a of the
/// writes, the most writes in one tick, and whether the OPL3 enable preceded the first write (0 none yet, 1 yes, 2 no).
static MUSIC_TICKS: AtomicU32 = AtomicU32::new(0);
static MUSIC_WRITES: AtomicU32 = AtomicU32::new(0);
static MUSIC_LOOPS: AtomicU32 = AtomicU32::new(0);
static MUSIC_HASH: AtomicU32 = AtomicU32::new(0x811c_9dc5);
static MUSIC_MAX_TICK: AtomicU32 = AtomicU32::new(0);
static MUSIC_ORDER: AtomicU8 = AtomicU8::new(0);
/// The most OPL writes any one tick made (music and effects together).
static MAX_TICK_WRITES: AtomicU32 = AtomicU32::new(0);
/// Whether the handler times itself, and its sum, largest and count in PIT clocks.
static PROFILE: AtomicBool = AtomicBool::new(false);
static PROF_SUM: AtomicU32 = AtomicU32::new(0);
static PROF_MAX: AtomicU32 = AtomicU32::new(0);
static PROF_COUNT: AtomicU32 = AtomicU32::new(0);
/// Samples thrown away because a reading was impossible (above the divisor): the program's own latch of the same counter can fall between
/// the handler's, and the handler then reads the other's half-read count.
static PROF_BAD: AtomicU32 = AtomicU32::new(0);

/// Writes in the buffer the music steps into: the most any tick of a stream carries (`fizz_music::MAX_WRITES_PER_TICK`).
const MUSIC_BUF: usize = 16;
/// Status reads in one OPL write: six after the register, 35 after the data.
pub const READS_PER_WRITE: u32 = 41;

/// The interrupt handler's state, behind a cell the handler alone touches.
struct Service {
    opl: OplMode,
    fx: FxPlan,
    /// The card whose arrangement the plan plays, or `None` for no music.
    card: Option<Card>,
    stream: Stream,
    music: Option<MusicPlayer>,
    fnv: Fnv,
    music_ticks: u32,
    music_writes: u32,
    music_max: u32,
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
    card: None,
    stream: Stream::Opl2,
    music: None,
    fnv: Fnv::new(),
    music_ticks: 0,
    music_writes: 0,
    music_max: 0,
    opl_player: None,
    pair: None,
    speaker: None,
    ticks: 0,
}));

/// One OPL register write: the register port, six status reads, the data port, then 35 status reads (the chip's settling times).
/// Every writer, music and effects, goes through here; it counts only in `ALL_WRITES`.
fn opl_raw(bank: u8, reg: u8, val: u8) {
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
    if bank == 1 && reg == 0x05 {
        OPL3_ON.store(val & 1 != 0, Ordering::Relaxed);
    }
    ALL_WRITES.store(ALL_WRITES.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
}

/// An effect, start or stop write: `opl_raw`, counted for the `AUDIO` line.
fn opl_write(bank: u8, reg: u8, val: u8) {
    opl_raw(bank, reg, val);
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

/// The card the plan's music plays on, if any.
fn card_of(plan: &Resolved) -> Option<Card> {
    if plan.opl == OplMode::None {
        return None;
    }
    match plan.music {
        MusicPlan::Off => None,
        MusicPlan::Opl2 => Some(Card::Opl2),
        MusicPlan::Opl3 => Some(Card::Opl3),
    }
}

impl Service {
    fn new(plan: &Resolved) -> Service {
        Service {
            opl: plan.opl,
            fx: plan.fx,
            card: card_of(plan),
            stream: plan.music.stream().unwrap_or(Stream::Opl2),
            music: None,
            fnv: Fnv::new(),
            music_ticks: 0,
            music_writes: 0,
            music_max: 0,
            opl_player: None,
            pair: None,
            speaker: None,
            ticks: 0,
        }
    }

    /// Builds the music player for the plan's arrangement (nothing when the plan has none); a second request changes nothing.
    fn begin_music(&mut self) {
        if self.music.is_none()
            && let Some(card) = self.card
        {
            self.music = Some(MusicPlayer::new(fizz_music::level_song().arrangement(card)));
        }
    }

    /// Steps the music one 560 Hz tick: its due writes (at most 16) go to the chip, hashed as the stream authored them.
    fn step_music(&mut self) {
        let (opl, stream) = (self.opl, self.stream);
        let Some(player) = self.music.as_mut() else {
            return;
        };
        let mut buf = [Write::default(); MUSIC_BUF];
        let n = player.tick(&mut buf);
        for w in &buf[..n] {
            if MUSIC_ORDER.load(Ordering::Relaxed) == 0 {
                let ok = stream == Stream::Opl2 || OPL3_ON.load(Ordering::Relaxed);
                MUSIC_ORDER.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            }
            opl_raw(w.bank, w.reg, route(opl, stream, w.bank, w.reg, w.val));
            self.fnv.write(self.music_ticks, *w);
        }
        self.music_writes = self.music_writes.wrapping_add(n as u32);
        self.music_max = self.music_max.max(n as u32);
        self.music_ticks = self.music_ticks.wrapping_add(1);
        MUSIC_TICKS.store(self.music_ticks, Ordering::Relaxed);
        MUSIC_WRITES.store(self.music_writes, Ordering::Relaxed);
        MUSIC_LOOPS.store(player.loops(), Ordering::Relaxed);
        MUSIC_HASH.store(self.fnv.value(), Ordering::Relaxed);
        MUSIC_MAX_TICK.store(self.music_max, Ordering::Relaxed);
    }

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
        if MUSIC_REQ.swap(false, Ordering::Relaxed) {
            self.begin_music();
        }
        self.step_music();
        let opl = self.opl;
        if let Some(player) = self.opl_player.as_mut() {
            let more = player.tick(&mut |bank, reg, val| opl_write(bank, reg, route(opl, Stream::Opl2, bank, reg, val)));
            if !more {
                self.opl_player = None;
            }
        }
        if let Some(pair) = self.pair.as_mut()
            && !pair.tick(&mut |bank, reg, val| opl_write(bank, reg, route(opl, Stream::Opl3, bank, reg, val)))
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
    let before = ALL_WRITES.load(Ordering::Relaxed);
    if PROFILE.load(Ordering::Relaxed) {
        let t0 = pit_count();
        state.tick();
        let t1 = pit_count();
        let period = u32::from(timer::divisor());
        if t0 > period || t1 > period {
            PROF_BAD.store(PROF_BAD.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
        } else {
            // The counter counts down and reloads once a period: a later reading above the earlier one means one reload between.
            let elapsed = if t1 <= t0 { t0 - t1 } else { t0 + period - t1 };
            PROF_SUM.store(PROF_SUM.load(Ordering::Relaxed).wrapping_add(elapsed), Ordering::Relaxed);
            if elapsed > PROF_MAX.load(Ordering::Relaxed) {
                PROF_MAX.store(elapsed, Ordering::Relaxed);
            }
            PROF_COUNT.store(PROF_COUNT.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
        }
    } else {
        state.tick();
    }
    let made = ALL_WRITES.load(Ordering::Relaxed).wrapping_sub(before);
    if made > MAX_TICK_WRITES.load(Ordering::Relaxed) {
        MAX_TICK_WRITES.store(made, Ordering::Relaxed);
    }
}

/// PIT channel 0's current count (latched), counting down from the divisor.
fn pit_count() -> u32 {
    port::outb(PIT_COMMAND, 0x00);
    let lo = port::inb(PIT_CHANNEL0);
    let hi = port::inb(PIT_CHANNEL0);
    u32::from(hi) << 8 | u32::from(lo)
}

/// Hooks the timer at 560 Hz and starts the audio service for `plan`: the chip is keyed off (and put in OPL3 mode when the plan
/// uses it), and `cluck` then plays on the device the plan names. With no device the service still runs, silently, so the clock is the same.
/// `stop` undoes it, and so do the exit and the panic handler. Call it after the probes: their waits need the PIT at the BIOS rate.
pub fn start(plan: &Resolved) -> Result<(), irq::Error> {
    reset(plan);
    timer::hook_560hz()?;
    if !cleanup::register(stop) {
        timer::unhook();
        return Err(irq::Error::CleanupFull);
    }
    timer::set_tick_hook(Some(service));
    // The handler does nothing until `ACTIVE`, so the long writes in `chip_up` cannot race it.
    chip_up(plan);
    ACTIVE.store(true, Ordering::Release);
    Ok(())
}

/// Puts the service's state and counters at their start for `plan`. The handler must not be reading the cell: no tick hook is set.
fn reset(plan: &Resolved) {
    // SAFETY: no tick hook is set yet, so the handler is not reading the cell.
    unsafe { *SERVICE.0.get() = Service::new(plan) };
    PENDING.store(0, Ordering::Relaxed);
    MUSIC_REQ.store(false, Ordering::Relaxed);
    WRITES.store(0, Ordering::Relaxed);
    ALL_WRITES.store(0, Ordering::Relaxed);
    OPL3_ON.store(false, Ordering::Relaxed);
    SPEAKER_STEPS.store(0, Ordering::Relaxed);
    MUSIC_ARR.store(
        match card_of(plan) {
            None => 0,
            Some(Card::Opl2) => 1,
            Some(Card::Opl3) => 2,
        },
        Ordering::Relaxed,
    );
    MUSIC_TICKS.store(0, Ordering::Relaxed);
    MUSIC_WRITES.store(0, Ordering::Relaxed);
    MUSIC_LOOPS.store(0, Ordering::Relaxed);
    MUSIC_HASH.store(Fnv::new().value(), Ordering::Relaxed);
    MUSIC_MAX_TICK.store(0, Ordering::Relaxed);
    MUSIC_ORDER.store(0, Ordering::Relaxed);
    MAX_TICK_WRITES.store(0, Ordering::Relaxed);
    PROF_SUM.store(0, Ordering::Relaxed);
    PROF_MAX.store(0, Ordering::Relaxed);
    PROF_COUNT.store(0, Ordering::Relaxed);
    PROF_BAD.store(0, Ordering::Relaxed);
}

/// Records the chip's mode for `stop` and puts the chip in the state the plan names: OPL3 mode on first (`enable_writes`: register 0x105,
/// then 0x104), then every channel keyed off. Nothing else is written before it, so the first music write always follows the enable.
fn chip_up(plan: &Resolved) {
    CHIP.store(
        match plan.opl {
            OplMode::None => 0,
            OplMode::Opl2 => 1,
            OplMode::Opl3 => 2,
        },
        Ordering::Relaxed,
    );
    if plan.opl != OplMode::None {
        for &(bank, reg, val) in enable_writes(plan.opl) {
            opl_write(bank, reg, val);
        }
        if plan.opl == OplMode::Opl3 {
            key_off_bank(1);
        }
        key_off_bank(0);
    }
}

/// Asks for the level song to start on the plan's arrangement at the next tick: a no-op when the service is not running, when the plan has
/// no music or the song is already playing. It plays until `stop` and loops itself.
pub fn music_start() {
    if ACTIVE.load(Ordering::Relaxed) {
        MUSIC_REQ.store(true, Ordering::Relaxed);
    }
}

/// Turns the handler's self-timing on or off (see the header). Call it before the run and read `profile` after `stop`.
pub fn set_profile(on: bool) {
    PROF_SUM.store(0, Ordering::Relaxed);
    PROF_MAX.store(0, Ordering::Relaxed);
    PROF_COUNT.store(0, Ordering::Relaxed);
    PROF_BAD.store(0, Ordering::Relaxed);
    MAX_TICK_WRITES.store(0, Ordering::Relaxed);
    PROFILE.store(on, Ordering::Relaxed);
}

/// What the music did since `start`, for the `MUSIC` line.
#[derive(Clone, Copy, Debug)]
pub struct MusicReport {
    /// 0 for no music, 1 for the OPL2 arrangement, 2 for the OPL3 one.
    pub arr: u8,
    pub ticks: u32,
    pub writes: u32,
    pub loops: u32,
    pub hash: u32,
    /// The most writes the music made on one tick.
    pub max_per_tick: u32,
    /// Whether the OPL3 enable preceded the first music write: 0 no write yet, 1 yes, 2 no.
    pub order: u8,
}

pub fn music_report() -> MusicReport {
    MusicReport {
        arr: MUSIC_ARR.load(Ordering::Relaxed),
        ticks: MUSIC_TICKS.load(Ordering::Relaxed),
        writes: MUSIC_WRITES.load(Ordering::Relaxed),
        loops: MUSIC_LOOPS.load(Ordering::Relaxed),
        hash: MUSIC_HASH.load(Ordering::Relaxed),
        max_per_tick: MUSIC_MAX_TICK.load(Ordering::Relaxed),
        order: MUSIC_ORDER.load(Ordering::Relaxed),
    }
}

/// The handler's cost, from `set_profile(true)`, and the writes it made.
#[derive(Clone, Copy, Debug)]
pub struct Profile {
    /// Interrupts timed, and their total and largest time in PIT clocks (0.838 us each).
    pub count: u32,
    pub sum_clocks: u32,
    pub max_clocks: u32,
    /// Samples discarded as impossible (see `PROF_BAD`).
    pub bad: u32,
    /// The most OPL writes any one tick made since `set_profile`, music and effects together.
    pub max_tick_writes: u32,
    /// OPL writes of every kind since `start`, and the status reads they cost (`READS_PER_WRITE` each).
    pub total_writes: u32,
}

pub fn profile() -> Profile {
    Profile {
        count: PROF_COUNT.load(Ordering::Relaxed),
        sum_clocks: PROF_SUM.load(Ordering::Relaxed),
        max_clocks: PROF_MAX.load(Ordering::Relaxed),
        bad: PROF_BAD.load(Ordering::Relaxed),
        max_tick_writes: MAX_TICK_WRITES.load(Ordering::Relaxed),
        total_writes: ALL_WRITES.load(Ordering::Relaxed),
    }
}

/// `/MUSICTEST`: steps the plan's arrangement for its prelude, two full loops and the tick the third pass starts on (so the player has
/// jumped back twice; no ticks at all when the plan has no music) as fast as the machine allows, through the real port-write path, with no
/// timer, no hook and no pacing. The chip is put up as `start` does and put back as `stop` does (the clean-up list covers a panic). Read
/// the result with `music_report`.
pub fn music_test(plan: &Resolved) -> Result<(), irq::Error> {
    reset(plan);
    if !cleanup::register(stop) {
        return Err(irq::Error::CleanupFull);
    }
    chip_up(plan);
    ACTIVE.store(true, Ordering::Release);
    // SAFETY: no tick hook is set, so nothing else reads the cell.
    let state = unsafe { &mut *SERVICE.0.get() };
    state.begin_music();
    let ticks = state.card.map_or(0, |card| {
        let a = fizz_music::level_song().arrangement(card);
        a.prelude_ticks + 2 * a.loop_ticks + 1
    });
    for _ in 0..ticks {
        state.step_music();
    }
    stop();
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
