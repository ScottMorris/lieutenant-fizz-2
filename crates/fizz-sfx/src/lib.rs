// Generated OPL and PC speaker data for the chicken's cluck, and two small steppers a DOS interrupt handler can drive.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The data is downmixed by `tools/sfx/downmix.ts` from the Undertone source under `assets/sfx/` (see `docs/SFX.md`); `data.rs` is
// generated and checked in CI. This crate owns no hardware access: the OPL player hands register writes to a callback and the
// speaker player returns a divisor, so the DOS presenter decides how to reach ports and the host tests need no hardware.
// Nothing here allocates, and nothing is `unsafe`.
//
// Stereo: the stored patterns carry no pan. A player built with `OplPlayer::new_panned` adds the OPL3 enable write and the channel's
// output bits as it plays; the pan idea (enable by writing 0x01 to register 0x105, output bits 0x10 left, 0x20 right and 0x30 both
// in a channel's C0 register, a second register bank above 0x100, five-byte records with a bank byte) follows the owner's IMF lab,
// `src/fizz-dos/opl.ts` and `music.ts`, adapted here as a packed bank bit and a runtime OR rather than a baked stream.
#![no_std]
#![forbid(unsafe_code)]

mod data;

pub use data::VARIANTS;

/// The OPL channel the effects use, in bank 0: unpanned effects and every plain OPL2 card. Scores must not use it.
pub const EFFECT_CHANNEL: u8 = 5;
/// The OPL3 channel a panned effect uses: channel 14 is slot 5 (`EFFECT_CHANNEL`) of the second register bank (9 + 5), so its
/// registers are 0x1a5, 0x1b5, 0x1c5 and its operators 0x12a and 0x12d and so on. Scores must not use it either. An OPL3 channel
/// has one output setting at a time, so a second effect panned at the same moment would need a third reserved channel; that is
/// not done.
pub const EFFECT_CHANNEL_PANNED: u8 = 14;
/// The OPL channels (numbered 0 to 17, the second bank's from 9) that sound effects own and music and scores must never use. A
/// score-writing tool should refuse these.
pub const RESERVED_OPL_CHANNELS: [u8; 2] = [EFFECT_CHANNEL, EFFECT_CHANNEL_PANNED];
/// How often [`OplPlayer::tick`] must be called, in hertz (the IMF clock).
pub const OPL_TICK_HZ: u32 = 560;
/// How often [`SpeakerPlayer::tick`] must be called, in hertz (the Keen effect clock, every fourth OPL tick).
pub const SPEAKER_TICK_HZ: u32 = 140;
/// The largest wait an [`OplStep`] can hold, in 560 Hz ticks (the top bit of the packed field is the bank).
pub const MAX_WAIT: u16 = 0x7fff;
/// The OPL3-enable write a panned player emits first, as `(bank, reg, val)`: register 0x105 set to 0x01.
pub const OPL3_ENABLE: (u8, u8, u8) = (1, 0x05, 0x01);

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
    channel == EFFECT_CHANNEL || channel == EFFECT_CHANNEL_PANNED
}

/// Where an effect sits in the OPL3's two outputs. The stored patterns carry no pan; a player adds it when asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pan {
    Left,
    Centre,
    Right,
}

impl Pan {
    /// The OPL3 output bits for a channel's C0 register: left 0x10, right 0x20, both (centre) 0x30. An OPL2 ignores them.
    pub const fn output_bits(self) -> u8 {
        match self {
            Pan::Left => 0x10,
            Pan::Right => 0x20,
            Pan::Centre => 0x30,
        }
    }
}

/// Whether playing with `pan` needs an OPL3: any pan does, `None` (the plain OPL2 path) does not. On a plain OPL2 chip the enable
/// write and the output bits are unused and harmless, but the panned path is meant only for a card detected as an OPL3.
pub const fn needs_opl3(pan: Option<Pan>) -> bool {
    pan.is_some()
}

/// One 140 Hz step of the PC speaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeakerStep {
    /// Program PIT channel 2 with this divisor (the tone is 1,193,180 / divisor Hz) and open the speaker gate.
    Divisor(u16),
    /// Close the speaker gate.
    Off,
}

/// The OPL records of a cluck variant. A `variant` of `VARIANTS` or more wraps, so a bad value is a different cluck and never a
/// panic in an interrupt handler.
pub fn opl_steps(variant: usize) -> &'static [OplStep] {
    data::OPL[variant % VARIANTS]
}

/// The length of a cluck variant's OPL chunk in 560 Hz ticks, including the wait after its last record.
pub fn opl_ticks(variant: usize) -> u32 {
    data::OPL_TICKS[variant % VARIANTS]
}

/// The PC speaker divisors of a cluck variant, one per 140 Hz step, with 0 for gate off; the last is always 0.
pub fn speaker_steps(variant: usize) -> &'static [u16] {
    data::SPEAKER[variant % VARIANTS]
}

/// Steps one OPL chunk. Call [`tick`](Self::tick) once per 560 Hz tick; it needs no memory beyond itself.
#[derive(Clone, Debug)]
pub struct OplPlayer {
    steps: &'static [OplStep],
    next: usize,
    /// Ticks still to pass before the record at `next` is due.
    delay: u32,
    pan: Option<Pan>,
    /// A panned player has not yet written the OPL3 enable.
    enable_due: bool,
}

impl OplPlayer {
    /// A plain OPL2 player at the start of `variant`'s chunk: exactly the stored writes, nothing added.
    pub fn new(variant: usize) -> Self {
        Self {
            steps: opl_steps(variant),
            next: 0,
            delay: 0,
            pan: None,
            enable_due: false,
        }
    }

    /// An OPL3 player: as [`new`](Self::new), but the first tick begins with the OPL3-enable write ([`OPL3_ENABLE`]), the effect
    /// plays on [`EFFECT_CHANNEL_PANNED`] (every write moves to bank 1, whose slot 5 has the same register offsets as channel 5,
    /// except the global waveform-select enable at 0x01, which stays in bank 0) and every write to that channel's C0 register
    /// (0x1c5) has `pan`'s output bits set. Use it only when an OPL3 was detected.
    pub fn new_panned(variant: usize, pan: Pan) -> Self {
        Self {
            pan: Some(pan),
            enable_due: true,
            ..Self::new(variant)
        }
    }

    /// Whether this player needs an OPL3 (it was built with [`new_panned`](Self::new_panned)).
    pub fn needs_opl3(&self) -> bool {
        needs_opl3(self.pan)
    }

    /// Emits the writes due on this tick to `out(bank, reg, val)`, in order, and returns `true` while the chunk has ticks left.
    /// It returns `false`, writing nothing, on the tick after the chunk's last wait has run out and on every tick after; the
    /// chunk's last write is the key-off, so the channel is silent by then. A panned player's enable write comes first, on the
    /// first tick, and takes no time.
    pub fn tick(&mut self, out: &mut impl FnMut(u8, u8, u8)) -> bool {
        if self.enable_due {
            self.enable_due = false;
            out(OPL3_ENABLE.0, OPL3_ENABLE.1, OPL3_ENABLE.2);
        }
        loop {
            if self.delay > 0 {
                break;
            }
            let Some(step) = self.steps.get(self.next) else {
                return false;
            };
            let mut val = step.val;
            let mut bank = step.bank();
            if let Some(pan) = self.pan {
                // The stored pattern is for channel 5 in bank 0. Slot 5 of bank 1 has the same offsets, so the move is the bank.
                if step.reg != 0x01 {
                    bank = 1;
                }
                if step.reg == 0xc0 + EFFECT_CHANNEL {
                    val = (val & 0x0f) | pan.output_bits();
                }
            }
            out(bank, step.reg, val);
            self.delay = u32::from(step.wait());
            self.next += 1;
        }
        self.delay -= 1;
        true
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

    const PANS: [Pan; 3] = [Pan::Left, Pan::Centre, Pan::Right];

    #[test]
    fn variant_count_matches_the_rules() {
        assert_eq!(VARIANTS, fizz_ep2::CLUCK_VARIANTS as usize);
    }

    #[test]
    fn every_opl_variant_terminates_at_its_declared_length() {
        for v in 0..VARIANTS {
            let (writes, ticks) = run_opl(v);
            assert_eq!(ticks, opl_ticks(v), "variant {v}");
            assert_eq!(writes.len(), opl_steps(v).len(), "variant {v}");
            // About 100 to 400 ms at 560 Hz.
            assert!((56..=224).contains(&ticks), "variant {v}: {ticks} ticks");
        }
    }

    #[test]
    fn opl_variants_end_keyed_off_on_the_effect_channel() {
        for v in 0..VARIANTS {
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
    fn an_unpanned_player_never_adds_the_enable_or_output_bits() {
        for v in 0..VARIANTS {
            let (writes, _) = run_opl(v);
            assert!(!writes.iter().any(|w| w.1 == 1), "variant {v}");
            assert!(!OplPlayer::new(v).needs_opl3());
            for w in writes.iter().filter(|w| w.2 == 0xc0 + EFFECT_CHANNEL) {
                assert_eq!(w.3 & 0x30, 0, "variant {v}");
            }
        }
    }

    #[test]
    fn a_panned_player_enables_the_opl3_first_and_only_then() {
        for v in 0..VARIANTS {
            for pan in PANS {
                let (writes, ticks) = run_with(OplPlayer::new_panned(v, pan));
                assert_eq!(writes[0], (0, 1, 0x05, 0x01), "variant {v} {pan:?}");
                assert_eq!(writes.iter().filter(|w| w.2 == 0x05).count(), 1);
                assert!(OplPlayer::new_panned(v, pan).needs_opl3());
                assert!(needs_opl3(Some(pan)) && !needs_opl3(None));
                // The enable takes no time: the length and every other write's tick are the plain path's.
                assert_eq!(ticks, opl_ticks(v));
                assert_eq!(writes[0].0, 0);
            }
        }
    }

    #[test]
    fn pan_bits_are_exact_on_every_channel_config_write() {
        const C0_PANNED: u8 = 0xc0 + EFFECT_CHANNEL; // register 0x1c5 in bank 1
        for v in 0..VARIANTS {
            let (plain, _) = run_opl(v);
            for pan in PANS {
                let (panned, _) = run_with(OplPlayer::new_panned(v, pan));
                let rest = &panned[1..];
                assert_eq!(rest.len(), plain.len());
                let mut configs = 0;
                for (p, q) in plain.iter().zip(rest) {
                    // Same tick and register; the panned write is in bank 1, except the global 0x01.
                    assert_eq!((p.0, p.2), (q.0, q.2), "variant {v} {pan:?}");
                    assert_eq!(q.1, u8::from(p.2 != 0x01), "variant {v} {pan:?}");
                    if p.2 == C0_PANNED {
                        assert_eq!(q.1, 1);
                        configs += 1;
                        // The feedback and connection bits are kept; the output bits are exactly the pan's.
                        assert_eq!(q.3 & 0x0f, p.3 & 0x0f);
                        assert_eq!(q.3 & 0xf0, pan.output_bits());
                    } else {
                        assert_eq!(p.3, q.3, "variant {v} {pan:?}");
                    }
                }
                assert!(configs > 0, "variant {v} never writes the channel config");
            }
        }
        assert_eq!(Pan::Left.output_bits(), 0x10);
        assert_eq!(Pan::Right.output_bits(), 0x20);
        assert_eq!(Pan::Centre.output_bits(), 0x30);
    }

    #[test]
    fn panned_writes_all_target_slot_five_of_bank_one() {
        // Slot 5 of a bank: operators 0x0a (modulator) and 0x0d (carrier), and the frequency, key and config registers 0xa5-0xc5.
        const SLOT5: [u8; 13] = [
            0x2a, 0x2d, 0x4a, 0x4d, 0x6a, 0x6d, 0x8a, 0x8d, 0xea, 0xed, 0xa5, 0xb5, 0xc5,
        ];
        assert_eq!(EFFECT_CHANNEL_PANNED, 9 + EFFECT_CHANNEL);
        for v in 0..VARIANTS {
            for pan in PANS {
                let (writes, _) = run_with(OplPlayer::new_panned(v, pan));
                for &(_, bank, reg, _) in &writes[1..] {
                    if reg == 0x01 {
                        assert_eq!(bank, 0);
                    } else {
                        assert_eq!(bank, 1, "variant {v} {pan:?} register {reg:#04x}");
                        assert!(SLOT5.contains(&reg), "variant {v} register {reg:#04x}");
                    }
                }
                // The pan bits land on 0x1c5, and nothing else in bank 1 carries them.
                let config: Vec<_> = writes.iter().filter(|w| w.1 == 1 && w.2 == 0xc5).collect();
                assert!(!config.is_empty());
                assert!(config.iter().all(|w| w.3 & 0xf0 == pan.output_bits()));
            }
        }
    }

    #[test]
    fn the_reserved_channels_are_five_and_fourteen() {
        assert_eq!(RESERVED_OPL_CHANNELS, [5, 14]);
        for c in 0..18 {
            assert_eq!(is_reserved_opl_channel(c), c == 5 || c == 14);
        }
    }

    #[test]
    fn panned_variants_still_terminate_keyed_off() {
        for v in 0..VARIANTS {
            for pan in PANS {
                let (writes, _) = run_with(OplPlayer::new_panned(v, pan));
                let last = writes.iter().rfind(|w| w.1 == 1 && w.2 == 0xb5).unwrap();
                assert_eq!(last.3 & 0xf0 & 0x20, 0, "variant {v} {pan:?}");
            }
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
        for v in 0..VARIANTS {
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
            // About 100 to 400 ms at 140 Hz.
            assert!(
                (14..=56).contains(&steps.len()),
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
        for v in 0..VARIANTS {
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
        for v in 0..VARIANTS {
            assert_eq!(run_opl(v), run_opl(v));
        }
        assert_ne!(speaker_steps(0), speaker_steps(1));
        assert_ne!(speaker_steps(1), speaker_steps(2));
        assert_ne!(opl_steps(0), opl_steps(2));
    }

    #[test]
    fn an_out_of_range_variant_wraps_and_never_panics() {
        assert_eq!(opl_steps(VARIANTS), opl_steps(0));
        assert_eq!(speaker_steps(VARIANTS + 1), speaker_steps(1));
    }

    #[test]
    fn a_player_is_small_state() {
        // The steppers hold a slice, an index and a counter, so an interrupt handler can keep one in a static.
        assert!(core::mem::size_of::<OplPlayer>() <= 40);
        assert!(core::mem::size_of::<SpeakerPlayer>() <= 24);
    }
}
