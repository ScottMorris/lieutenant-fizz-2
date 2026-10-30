// Generated OPL and PC speaker data for the sound effects (the chicken's cluck is ids 0 to 3, and eleven more follow), and two small steppers a DOS interrupt handler can drive.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The data is downmixed by `tools/sfx/downmix.ts` from the Undertone source under `assets/sfx/` (see `docs/SOUND.md`); `data.rs` is
// generated and checked in CI. This crate owns no hardware access: the OPL player hands register writes to a callback and the
// speaker player returns a divisor, so the DOS presenter decides how to reach ports and the host tests need no hardware.
// Nothing here allocates, and nothing is `unsafe`.
//
// Stereo: the stored patterns carry no pan or level. On an OPL3 with effects on the Sound Blaster the effect plays as a pair: the same
// records on channel 5 (bank 0, output bit 0x10, the left) and channel 14 (bank 1, output bit 0x20, the right), each with its own
// carrier level, which `place` works out from where the sound source is on screen. Nothing is added to the writes but the bank, the
// output bit and the carrier's total level (an attenuation in 0.75 dB steps), so the pair costs the writes of two channels.
// Each side takes its own record stream (today both get the same one), so a later stereo-authored effect can put different content
// on each side without changing this code. The OPL3 idea (enable by writing 0x01 to register 0x105, output bits 0x10 left, 0x20
// right and 0x30 both in a channel's C0 register, a second register bank above 0x100, five-byte records with a bank byte) follows the
// common OPL3 register conventions, adapted here as a packed bank bit and a runtime rewrite rather than a baked stream.
#![no_std]
#![forbid(unsafe_code)]

mod data;
pub mod place;

/// The number of effects in the source (`assets/sfx/effects.ts`), which is also the length of every table in `data.rs`. Effect ids
/// are `0..EFFECT_COUNT`; the cluck's four variants are ids 0 to 3 (`fizz_ep2::CLUCK_VARIANTS` of them).
pub const EFFECT_COUNT: usize = data::VARIANTS;

/// The OPL channel the effects use, in bank 0: the only channel on a plain OPL2 or an OPL3 whose effects are on the AdLib role, and
/// the left channel of the pair when stereo is available. Scores must not use it.
pub const EFFECT_CHANNEL: u8 = 5;
/// The right channel of the pair: channel 14 is slot 5 (`EFFECT_CHANNEL`) of the second register bank (9 + 5), so its registers are
/// 0x1a5, 0x1b5, 0x1c5 and its operators 0x12a and 0x12d and so on. It is only ever the extension that makes channel 5 stereo; it
/// plays the same effect (or, later, the right half of a stereo-authored one). Scores must not use it either.
pub const EFFECT_CHANNEL_RIGHT: u8 = 14;
/// The OPL channels (numbered 0 to 17, the second bank's from 9) that sound effects own and music and scores must never use. A
/// score-writing tool should refuse these.
pub const RESERVED_OPL_CHANNELS: [u8; 2] = [EFFECT_CHANNEL, EFFECT_CHANNEL_RIGHT];
/// How often [`OplPlayer::tick`] must be called, in hertz (the IMF clock).
pub const OPL_TICK_HZ: u32 = 560;
/// How often [`SpeakerPlayer::tick`] must be called, in hertz (the Keen effect clock, every fourth OPL tick).
pub const SPEAKER_TICK_HZ: u32 = 140;
/// The largest wait an [`OplStep`] can hold, in 560 Hz ticks (the top bit of the packed field is the bank).
pub const MAX_WAIT: u16 = 0x7fff;
/// The OPL3-enable write, as `(bank, reg, val)`: register 0x105 set to 0x01. The audio service makes it once at start, not per effect.
pub const OPL3_ENABLE: (u8, u8, u8) = (1, 0x05, 0x01);
/// The carrier operator's total-level register of channel 5, and of slot 5 of the second bank: the level the pair scales.
const CARRIER_TL: u8 = 0x40 + 0x0d;
/// The config register of channel 5 (and slot 5 of bank 1), which holds the output bits.
const CONFIG: u8 = 0xc0 + EFFECT_CHANNEL;
/// The largest total-level attenuation (6 bits, 0.75 dB a step: 47.25 dB, which is quiet, not silent).
pub const MAX_ATTENUATION: u8 = 63;

/// One OPL register write and the number of 560 Hz ticks to wait after it before the next write is due.
///
/// A register is nine bits on an OPL3: `reg` is its low byte and [`bank`](Self::bank) says whether it is in the first bank
/// (registers 0x000 to 0x0ff, the OPL2's) or the second (0x100 to 0x1ff, OPL3 only). The bank shares a `u16` with the wait, so a
/// step stays four bytes. The generated cluck data is entirely in bank 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OplStep {
    pub reg: u8,
    pub val: u8,
    bank_wait: u16,
}

impl OplStep {
    /// A step in `bank` (0 or 1) that waits `wait` ticks (at most [`MAX_WAIT`]) after its write.
    pub const fn new(bank: u8, reg: u8, val: u8, wait: u16) -> Self {
        assert!(bank <= 1 && wait <= MAX_WAIT);
        Self {
            reg,
            val,
            bank_wait: ((bank as u16) << 15) | wait,
        }
    }

    /// The register bank: 0 for registers 0x000 to 0x0ff, 1 for 0x100 to 0x1ff.
    pub const fn bank(&self) -> u8 {
        (self.bank_wait >> 15) as u8
    }

    /// The ticks to wait after this write.
    pub const fn wait(&self) -> u16 {
        self.bank_wait & MAX_WAIT
    }
}

/// Whether `channel` (0 to 17) is reserved for sound effects.
pub const fn is_reserved_opl_channel(channel: u8) -> bool {
    channel == EFFECT_CHANNEL || channel == EFFECT_CHANNEL_RIGHT
}

/// Which output of the OPL3 a pair member feeds, and so which bank and which C0 output bit it uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Channel 5 in bank 0, output bit 0x10.
    Left,
    /// Channel 14 (slot 5 of bank 1), output bit 0x20.
    Right,
}

impl Side {
    /// The OPL3 output bit for a channel's C0 register: left 0x10, right 0x20. An OPL2 ignores them.
    pub const fn output_bit(self) -> u8 {
        match self {
            Side::Left => 0x10,
            Side::Right => 0x20,
        }
    }

    /// The register bank the side's channel is in.
    pub const fn bank(self) -> u8 {
        match self {
            Side::Left => 0,
            Side::Right => 1,
        }
    }
}

/// One 140 Hz step of the PC speaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeakerStep {
    /// Program PIT channel 2 with this divisor (the tone is 1,193,180 / divisor Hz) and open the speaker gate.
    Divisor(u16),
    /// Close the speaker gate.
    Off,
}

/// The OPL records of effect `id`. An `id` of `EFFECT_COUNT` or more wraps, so a bad value is a different effect and never a
/// panic in an interrupt handler.
pub fn opl_steps(id: usize) -> &'static [OplStep] {
    data::OPL[id % EFFECT_COUNT]
}

/// The length of effect `id`'s OPL chunk in 560 Hz ticks, including the wait after its last record.
pub fn opl_ticks(id: usize) -> u32 {
    data::OPL_TICKS[id % EFFECT_COUNT]
}

/// The PC speaker divisors of effect `id`, one per 140 Hz step, with 0 for gate off; the last is always 0.
pub fn speaker_steps(id: usize) -> &'static [u16] {
    data::SPEAKER[id % EFFECT_COUNT]
}

/// How a player rewrites the stored channel-5 records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Output {
    /// Exactly the stored writes.
    Plain,
    /// Channel 5 with its carrier level raised by this attenuation; the output bits are left to the caller (the OR rule).
    Mono(u8),
    /// One member of the OPL3 pair: the side's bank and output bit, and this attenuation on the carrier.
    Side(Side, u8),
}

/// Raises the attenuation in a total-level byte (bits 0 to 5), keeping the key-scale bits (6 and 7); it never wraps past 63.
const fn attenuate(tl: u8, by: u8) -> u8 {
    let level = (tl & 0x3f) as u16 + by as u16;
    (tl & 0xc0)
        | if level > MAX_ATTENUATION as u16 {
            MAX_ATTENUATION
        } else {
            level as u8
        }
}

/// Steps one OPL chunk. Call [`tick`](Self::tick) once per 560 Hz tick; it needs no memory beyond itself.
#[derive(Clone, Debug)]
pub struct OplPlayer {
    steps: &'static [OplStep],
    next: usize,
    /// Ticks still to pass before the record at `next` is due.
    delay: u32,
    output: Output,
}

impl OplPlayer {
    fn with(steps: &'static [OplStep], output: Output) -> Self {
        Self {
            steps,
            next: 0,
            delay: 0,
            output,
        }
    }

    /// A plain OPL2 player at the start of `variant`'s chunk: exactly the stored writes, nothing added.
    pub fn new(variant: usize) -> Self {
        Self::with(opl_steps(variant), Output::Plain)
    }

    /// Channel 5 for the mono paths (an OPL2, or an OPL3 whose effects are on the AdLib role): the stored writes with the carrier's
    /// total level raised by `attenuation` (0 to 63 steps of 0.75 dB; the distance). No extra writes, and the output bits stay the
    /// caller's (`route` ORs 0x30 in on an OPL3).
    pub fn mono(variant: usize, attenuation: u8) -> Self {
        Self::with(
            opl_steps(variant),
            Output::Mono(attenuation.min(MAX_ATTENUATION)),
        )
    }

    /// One member of the OPL3 pair, playing `steps` (channel-5 records) on `side` with `attenuation` on its carrier: every write
    /// moves to the side's bank, the C0 write gets exactly the side's output bit, and the carrier's total level is raised. The
    /// right side leaves out the global register 0x01 (waveform-select enable), which the left side's stream already writes.
    pub fn side(steps: &'static [OplStep], side: Side, attenuation: u8) -> Self {
        Self::with(steps, Output::Side(side, attenuation.min(MAX_ATTENUATION)))
    }

    /// Emits the writes due on this tick to `out(bank, reg, val)`, in order, and returns `true` while the chunk has ticks left.
    /// It returns `false`, writing nothing, on the tick after the chunk's last wait has run out and on every tick after; the
    /// chunk's last write is the key-off, so the channel is silent by then.
    pub fn tick(&mut self, out: &mut impl FnMut(u8, u8, u8)) -> bool {
        loop {
            if self.delay > 0 {
                break;
            }
            let Some(step) = self.steps.get(self.next) else {
                return false;
            };
            self.delay = u32::from(step.wait());
            self.next += 1;
            let (mut bank, mut val) = (step.bank(), step.val);
            match self.output {
                Output::Plain => {}
                Output::Mono(att) => {
                    if step.reg == CARRIER_TL {
                        val = attenuate(val, att);
                    }
                }
                Output::Side(side, att) => {
                    if step.reg == 0x01 {
                        // The chip-wide register: bank 0 only, and written once, by the left stream.
                        if side == Side::Right {
                            continue;
                        }
                    } else {
                        bank = side.bank();
                    }
                    if step.reg == CONFIG {
                        val = (val & 0x0f) | side.output_bit();
                    } else if step.reg == CARRIER_TL {
                        val = attenuate(val, att);
                    }
                }
            }
            out(bank, step.reg, val);
        }
        self.delay -= 1;
        true
    }
}

/// The OPL3 pair: the left member on channel 5 and the right on channel 14, stepped together so their writes land on the same
/// ticks. Each member has its own record stream and its own attenuation (power per side, from [`place::Placement`]), so the
/// same code serves an effect with different content on each side; today both get the same stream.
#[derive(Clone, Debug)]
pub struct OplPair {
    left: OplPlayer,
    right: OplPlayer,
}

impl OplPair {
    /// A pair with one record stream per side and its own attenuations.
    pub fn new(
        left: &'static [OplStep],
        right: &'static [OplStep],
        left_attenuation: u8,
        right_attenuation: u8,
    ) -> Self {
        Self {
            left: OplPlayer::side(left, Side::Left, left_attenuation),
            right: OplPlayer::side(right, Side::Right, right_attenuation),
        }
    }

    /// The same effect on both sides, as every effect is today.
    pub fn same(variant: usize, left_attenuation: u8, right_attenuation: u8) -> Self {
        let steps = opl_steps(variant);
        Self::new(steps, steps, left_attenuation, right_attenuation)
    }

    /// Steps both members (left first); returns `true` while either has ticks left.
    pub fn tick(&mut self, out: &mut impl FnMut(u8, u8, u8)) -> bool {
        let l = self.left.tick(out);
        let r = self.right.tick(out);
        l || r
    }
}

/// Steps one PC speaker list. Call [`tick`](Self::tick) once per 140 Hz step.
#[derive(Clone, Debug)]
pub struct SpeakerPlayer {
    steps: &'static [u16],
    next: usize,
}

impl SpeakerPlayer {
    /// A player at the start of `variant`'s list.
    pub fn new(variant: usize) -> Self {
        Self {
            steps: speaker_steps(variant),
            next: 0,
        }
    }

    /// The step for this tick, or `None` once the list is spent (the last step before that is always [`SpeakerStep::Off`]).
    pub fn tick(&mut self) -> Option<SpeakerStep> {
        let divisor = *self.steps.get(self.next)?;
        self.next += 1;
        Some(if divisor == 0 {
            SpeakerStep::Off
        } else {
            SpeakerStep::Divisor(divisor)
        })
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    /// One write as a driver sees it: the tick it landed on, the bank, the register and the value.
    type Write = (u32, u8, u8, u8);

    fn run_with(mut p: OplPlayer) -> (Vec<Write>, u32) {
        let mut writes = Vec::new();
        let mut tick = 0u32;
        loop {
            let t = tick;
            if !p.tick(&mut |b, r, v| writes.push((t, b, r, v))) {
                return (writes, tick);
            }
            tick += 1;
            assert!(tick < 100_000, "a variant never ends");
        }
    }

    /// Drives a plain OPL2 variant to its end.
    fn run_opl(variant: usize) -> (Vec<Write>, u32) {
        run_with(OplPlayer::new(variant))
    }

    /// Effect ids, in order: the cluck's four variants, then fzzt, stun, bonk, hurt, win, key, clunk, plink, boing, jump and crunch.
    /// The OPL records, the OPL chunk's ticks and the speaker frames of each, as `tools/sfx/downmix.ts --stats` prints them. A
    /// change to the source or the converter moves these on purpose, and this table is what says so.
    const EFFECTS: [(&str, usize, u32, usize); 15] = [
        ("cluck 0", 178, 212, 48),
        ("cluck 1", 74, 88, 16),
        ("cluck 2", 147, 188, 42),
        ("cluck 3", 150, 200, 46),
        ("fzzt", 69, 100, 16),
        ("stun", 75, 112, 16),
        ("bonk", 74, 88, 14),
        ("hurt", 223, 336, 62),
        ("win", 317, 432, 86),
        ("key", 166, 220, 42),
        ("clunk", 95, 116, 20),
        ("plink", 39, 56, 8),
        ("boing", 186, 252, 42),
        ("jump", 57, 84, 12),
        ("crunch", 118, 136, 24),
    ];

    #[test]
    fn effect_ids_are_the_cluck_variants_then_the_eleven_others() {
        assert_eq!(EFFECT_COUNT, EFFECTS.len());
        assert_eq!(EFFECT_COUNT, fizz_ep2::CLUCK_VARIANTS as usize + 11);
    }

    #[test]
    fn every_effect_has_the_recorded_size_and_length() {
        for (id, &(name, records, ticks, frames)) in EFFECTS.iter().enumerate() {
            assert_eq!(opl_steps(id).len(), records, "{name}: records");
            assert_eq!(opl_ticks(id), ticks, "{name}: ticks");
            assert_eq!(speaker_steps(id).len(), frames, "{name}: speaker frames");
        }
    }

    /// An effect on its own channel stays inside the music's 16 writes in any tick (the patch load is 11), so a pair of channels
    /// is at most 22 and the chip never has more than 38 writes to make in a tick with music on it.
    #[test]
    fn no_effect_writes_more_than_eleven_registers_in_a_tick() {
        for (id, &(name, ..)) in EFFECTS.iter().enumerate() {
            let mut run = 0;
            let mut worst = 0;
            for s in opl_steps(id) {
                run += 1;
                worst = worst.max(run);
                if s.wait() > 0 {
                    run = 0;
                }
            }
            assert!(worst <= 11, "{name}: {worst} writes in one tick");
        }
    }

    #[test]
    fn every_opl_variant_terminates_at_its_declared_length() {
        for v in 0..EFFECT_COUNT {
            let (writes, ticks) = run_opl(v);
            assert_eq!(ticks, opl_ticks(v), "variant {v}");
            assert_eq!(writes.len(), opl_steps(v).len(), "variant {v}");
            // 100 ms to 800 ms at 560 Hz.
            assert!((56..=450).contains(&ticks), "variant {v}: {ticks} ticks");
        }
    }

    #[test]
    fn opl_variants_end_keyed_off_on_the_effect_channel() {
        for v in 0..EFFECT_COUNT {
            let (writes, _) = run_opl(v);
            let b = 0xb0 + EFFECT_CHANNEL;
            let key_writes: Vec<_> = writes.iter().filter(|w| w.2 == b).collect();
            assert!(
                key_writes.iter().any(|w| w.3 & 0x20 != 0),
                "variant {v} never keys on"
            );
            assert_eq!(
                key_writes.last().unwrap().3 & 0x20,
                0,
                "variant {v} ends keyed on"
            );
            // Every write is to bank 0, channel 5's operators and registers, or the chip's waveform-select enable.
            for &(_, bank, reg, _) in &writes {
                let channel_reg = matches!(
                    reg,
                    0x2a | 0x2d
                        | 0x4a
                        | 0x4d
                        | 0x6a
                        | 0x6d
                        | 0x8a
                        | 0x8d
                        | 0xea
                        | 0xed
                        | 0xa5
                        | 0xb5
                        | 0xc5
                );
                assert_eq!(bank, 0, "variant {v}");
                assert!(
                    reg == 0x01 || channel_reg,
                    "variant {v} writes register {reg:#04x}"
                );
            }
        }
    }

    #[test]
    fn opl_writes_land_in_time_order_and_the_player_stays_finished() {
        let mut p = OplPlayer::new(0);
        let mut last = 0;
        let mut count = 0;
        while p.tick(&mut |_, _, _| count += 1) {
            last += 1;
        }
        assert_eq!(last, opl_ticks(0));
        for _ in 0..5 {
            assert!(!p.tick(&mut |_, _, _| panic!("a finished player wrote")));
        }
        assert_eq!(count, opl_steps(0).len());
    }

    /// The plain OPL2 path's writes, pinned: a hash of (tick, register, value) for every write and the chunk length, taken from
    /// the output before the bank bit and the pan existed. If the data or the player changes the plain path, this fails.
    #[test]
    fn the_plain_opl2_path_is_unchanged() {
        const PINNED: [(u64, usize); 4] = [
            (0xbd88348588b69cd4, 178),
            (0x90045cf8eef35856, 74),
            (0xea01205b3ac03f53, 147),
            (0x6e624a38ea72199c, 150),
        ];
        for (v, &(want, count)) in PINNED.iter().enumerate() {
            let (writes, ticks) = run_opl(v);
            let mut h: u64 = 0xcbf29ce484222325;
            for (t, _, r, val) in writes
                .iter()
                .copied()
                .chain(core::iter::once((ticks, 0, 0, 0)))
            {
                for b in t.to_le_bytes().into_iter().chain([r, val]) {
                    h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
                }
            }
            assert_eq!((h, writes.len()), (want, count), "variant {v}");
        }
    }

    /// Variant 3 is variant 1 twice: the same count of key-ons, doubled, and the same patch writes, in under 450 ms.
    #[test]
    fn variant_3_is_variant_1_twice() {
        let key_ons = |v: usize| {
            let (writes, _) = run_opl(v);
            let mut on = false;
            let mut n = 0;
            for w in writes.iter().filter(|w| w.2 == 0xb0 + EFFECT_CHANNEL) {
                let now = w.3 & 0x20 != 0;
                n += usize::from(now && !on);
                on = now;
            }
            n
        };
        assert_eq!(key_ons(3), 2 * key_ons(1));
        assert!(opl_ticks(3) > opl_ticks(1) * 2 && opl_ticks(3) * 1000 / 560 < 450);
        assert!(speaker_steps(3).len() > speaker_steps(1).len() * 2);
    }

    #[test]
    fn a_plain_or_unattenuated_player_adds_nothing() {
        for v in 0..EFFECT_COUNT {
            let (plain, _) = run_opl(v);
            assert!(!plain.iter().any(|w| w.1 == 1), "variant {v}");
            for w in plain.iter().filter(|w| w.2 == CONFIG) {
                assert_eq!(w.3 & 0x30, 0, "variant {v}");
                // No effect is additive, so the modulator's level never needs scaling with the carrier's.
                assert_eq!(w.3 & 1, 0, "variant {v} is additive");
            }
            assert_eq!(run_with(OplPlayer::mono(v, 0)), run_opl(v));
        }
    }

    #[test]
    fn mono_attenuation_changes_only_the_carrier_level() {
        for v in 0..EFFECT_COUNT {
            let (plain, ticks) = run_opl(v);
            for att in [1u8, 4, 20, 63] {
                let (mono, mono_ticks) = run_with(OplPlayer::mono(v, att));
                assert_eq!((mono.len(), mono_ticks), (plain.len(), ticks));
                let mut levels = 0;
                for (p, q) in plain.iter().zip(&mono) {
                    assert_eq!((p.0, p.1, p.2), (q.0, q.1, q.2));
                    if p.2 == CARRIER_TL {
                        levels += 1;
                        assert_eq!(q.3 & 0xc0, p.3 & 0xc0, "key scaling is kept");
                        assert_eq!(q.3 & 0x3f, ((p.3 & 0x3f) + att).min(63));
                    } else {
                        assert_eq!(p.3, q.3);
                    }
                }
                assert!(levels > 0, "variant {v} never sets the carrier level");
            }
        }
        assert_eq!(attenuate(0x5f, 40), 0x7f, "clamped at 63, key scale kept");
        assert_eq!(attenuate(0xc3, 2), 0xc5);
    }

    /// The pair's streams for variant `v` with the given attenuations: (left writes, right writes, ticks).
    fn pair_run(v: usize, l: u8, r: u8) -> (Vec<Write>, Vec<Write>, u32) {
        let mut pair = OplPair::same(v, l, r);
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut tick = 0u32;
        loop {
            let t = tick;
            let more = pair.tick(&mut |b, reg, val| {
                if b == 0 {
                    left.push((t, b, reg, val));
                } else {
                    right.push((t, b, reg, val));
                }
            });
            if !more {
                return (left, right, tick);
            }
            tick += 1;
        }
    }

    #[test]
    fn the_pair_plays_one_effect_on_both_channels_with_one_output_bit_each() {
        for v in 0..EFFECT_COUNT {
            let (plain, plain_ticks) = run_opl(v);
            let waveform_enables = plain.iter().filter(|w| w.2 == 0x01).count();
            for (l, r) in [(4u8, 4u8), (1, 14), (14, 1), (0, 63)] {
                let (left, right, ticks) = pair_run(v, l, r);
                assert_eq!(ticks, plain_ticks, "the pair ends with the single channel");
                // Left is bank 0, right is bank 1; the global enable is written once, by the left.
                assert_eq!(left.len(), plain.len());
                assert_eq!(right.len(), plain.len() - waveform_enables);
                assert!(left.iter().all(|w| w.1 == 0));
                assert!(right.iter().all(|w| w.1 == 1));
                assert!(right.iter().all(|w| w.2 != 0x01 && w.2 != 0x05));
                // Both sides keep the plain stream's order and ticks, with only the C0 bit and the carrier level changed.
                for (side, writes, bit, att) in
                    [("left", &left, 0x10, l), ("right", &right, 0x20, r)]
                {
                    let mut it = writes.iter();
                    for p in plain.iter().filter(|p| side == "left" || p.2 != 0x01) {
                        let q = it.next().unwrap();
                        assert_eq!((p.0, p.2), (q.0, q.2), "variant {v} {side}");
                        match p.2 {
                            CONFIG => {
                                assert_eq!(q.3 & 0x0f, p.3 & 0x0f);
                                assert_eq!(q.3 & 0xf0, bit, "exactly one output bit");
                            }
                            CARRIER_TL => {
                                assert_eq!(q.3, attenuate(p.3, att), "variant {v} {side}")
                            }
                            _ => assert_eq!(q.3, p.3, "variant {v} {side}"),
                        }
                    }
                }
            }
        }
        assert_eq!(
            (Side::Left.output_bit(), Side::Right.output_bit()),
            (0x10, 0x20)
        );
        assert_eq!((Side::Left.bank(), Side::Right.bank()), (0, 1));
    }

    #[test]
    fn the_pair_writes_only_slot_five_of_each_bank_and_ends_keyed_off() {
        // Slot 5 of a bank: operators 0x0a (modulator) and 0x0d (carrier), and the frequency, key and config registers 0xa5-0xc5.
        const SLOT5: [u8; 13] = [
            0x2a, 0x2d, 0x4a, 0x4d, 0x6a, 0x6d, 0x8a, 0x8d, 0xea, 0xed, 0xa5, 0xb5, 0xc5,
        ];
        assert_eq!(EFFECT_CHANNEL_RIGHT, 9 + EFFECT_CHANNEL);
        for v in 0..EFFECT_COUNT {
            let (left, right, _) = pair_run(v, 4, 4);
            for w in left.iter().filter(|w| w.2 != 0x01).chain(&right) {
                assert!(SLOT5.contains(&w.2), "variant {v} register {:#04x}", w.2);
            }
            for writes in [&left, &right] {
                let key = writes.iter().rfind(|w| w.2 == 0xb5).unwrap();
                assert_eq!(key.3 & 0x20, 0, "variant {v} ends keyed on");
            }
        }
    }

    #[test]
    fn the_pair_can_carry_different_content_per_side_with_its_own_level() {
        // The hook for stereo-authored effects, not used by the game: each side steps its own stream at its own level.
        let mut pair = OplPair::new(opl_steps(0), opl_steps(2), 3, 9);
        let mut counts = [0usize; 2];
        let mut tl = [0u8; 2];
        while pair.tick(&mut |b, reg, val| {
            counts[usize::from(b)] += 1;
            if reg == CARRIER_TL {
                tl[usize::from(b)] = val & 0x3f;
            }
        }) {}
        assert_eq!(counts[0], opl_steps(0).len());
        let global = opl_steps(2).iter().filter(|s| s.reg == 0x01).count();
        assert_eq!(counts[1], opl_steps(2).len() - global);
        assert!(tl[0] >= 3 && tl[1] >= 9);
    }

    #[test]
    fn the_reserved_channels_are_five_and_fourteen() {
        assert_eq!(RESERVED_OPL_CHANNELS, [5, 14]);
        for c in 0..18 {
            assert_eq!(is_reserved_opl_channel(c), c == 5 || c == 14);
        }
    }

    #[test]
    fn the_bank_bit_round_trips_and_a_step_stays_four_bytes() {
        assert_eq!(core::mem::size_of::<OplStep>(), 4);
        for bank in 0..=1 {
            for wait in [0, 1, 4, 0x1234, MAX_WAIT] {
                let s = OplStep::new(bank, 0xc5, 0x3e, wait);
                assert_eq!((s.bank(), s.reg, s.val, s.wait()), (bank, 0xc5, 0x3e, wait));
            }
        }
    }

    #[test]
    fn a_second_bank_step_is_reported_to_the_callback_with_its_bank() {
        static STEPS: [OplStep; 2] = [OplStep::new(1, 0x05, 0x01, 0), OplStep::new(0, 0xb5, 0, 2)];
        let mut p = OplPlayer::new(0);
        p.steps = &STEPS;
        let (writes, ticks) = run_with(p);
        assert_eq!(writes, [(0, 1, 0x05, 0x01), (0, 0, 0xb5, 0)]);
        assert_eq!(ticks, 2);
    }

    #[test]
    fn every_speaker_variant_terminates_with_the_gate_off() {
        for v in 0..EFFECT_COUNT {
            let mut p = SpeakerPlayer::new(v);
            let mut steps = Vec::new();
            while let Some(s) = p.tick() {
                steps.push(s);
                assert!(steps.len() < 10_000);
            }
            assert_eq!(p.tick(), None);
            assert_eq!(steps.len(), speaker_steps(v).len());
            assert_eq!(steps.last(), Some(&SpeakerStep::Off), "variant {v}");
            assert!(
                steps.iter().any(|s| matches!(s, SpeakerStep::Divisor(_))),
                "variant {v} is silent"
            );
            // 50 ms to 650 ms at 140 Hz.
            assert!(
                (8..=110).contains(&steps.len()),
                "variant {v}: {} steps",
                steps.len()
            );
            // Divisors stay in the audible band the converter clamps to (60 to 9000 Hz).
            for s in &steps {
                if let SpeakerStep::Divisor(d) = s {
                    assert!((132..=19_890).contains(d), "variant {v}: divisor {d}");
                }
            }
        }
    }

    #[test]
    fn the_speaker_list_is_no_longer_than_the_opl_chunk() {
        // The speaker list drops the quiet tail the OPL2 still plays, so it may be shorter, never longer.
        for v in 0..EFFECT_COUNT {
            let opl_ms = opl_ticks(v) * 1000 / OPL_TICK_HZ;
            let speaker_ms = speaker_steps(v).len() as u32 * 1000 / SPEAKER_TICK_HZ;
            assert!(
                speaker_ms <= opl_ms + 8,
                "variant {v}: {speaker_ms} ms against {opl_ms} ms"
            );
            assert!(
                speaker_ms * 2 >= opl_ms,
                "variant {v}: {speaker_ms} ms against {opl_ms} ms"
            );
        }
    }

    #[test]
    fn playing_twice_is_identical_and_variants_differ() {
        for v in 0..EFFECT_COUNT {
            assert_eq!(run_opl(v), run_opl(v));
        }
        assert_ne!(speaker_steps(0), speaker_steps(1));
        assert_ne!(speaker_steps(1), speaker_steps(2));
        assert_ne!(opl_steps(0), opl_steps(2));
    }

    #[test]
    fn an_out_of_range_variant_wraps_and_never_panics() {
        assert_eq!(opl_steps(EFFECT_COUNT), opl_steps(0));
        assert_eq!(speaker_steps(EFFECT_COUNT + 1), speaker_steps(1));
    }

    #[test]
    fn a_player_is_small_state() {
        // The steppers hold a slice, an index and a counter, so an interrupt handler can keep one in a static.
        assert!(core::mem::size_of::<OplPlayer>() <= 40);
        assert!(core::mem::size_of::<SpeakerPlayer>() <= 24);
    }
}
