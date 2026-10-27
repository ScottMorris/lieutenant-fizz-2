// Generated OPL2 and PC speaker data for the chicken's cluck, and two small steppers a DOS interrupt handler can drive.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The data is downmixed by `tools/sfx/downmix.ts` from the Undertone source under `assets/sfx/` (see `docs/SFX.md`); `data.rs` is
// generated and checked in CI. This crate owns no hardware access: the OPL player hands register writes to a callback and the
// speaker player returns a divisor, so the DOS presenter decides how to reach ports and the host tests need no hardware.
// Nothing here allocates, and nothing is `unsafe`.
#![no_std]
#![forbid(unsafe_code)]

mod data;

pub use data::VARIANTS;

/// The OPL2 channel the effects use; music keeps to the other channels.
pub const EFFECT_CHANNEL: u8 = 5;
/// How often [`OplPlayer::tick`] must be called, in hertz (the IMF clock).
pub const OPL_TICK_HZ: u32 = 560;
/// How often [`SpeakerPlayer::tick`] must be called, in hertz (the Keen effect clock, every fourth OPL tick).
pub const SPEAKER_TICK_HZ: u32 = 140;

/// One OPL2 register write and the number of 560 Hz ticks to wait after it before the next write is due.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OplStep {
    pub reg: u8,
    pub val: u8,
    pub wait: u16,
}

/// One 140 Hz step of the PC speaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeakerStep {
    /// Program PIT channel 2 with this divisor (the tone is 1,193,180 / divisor Hz) and open the speaker gate.
    Divisor(u16),
    /// Close the speaker gate.
    Off,
}

/// The OPL2 records of a cluck variant. A `variant` of `VARIANTS` or more wraps, so a bad value is a different cluck and never a
/// panic in an interrupt handler.
pub fn opl_steps(variant: usize) -> &'static [OplStep] {
    data::OPL[variant % VARIANTS]
}

/// The length of a cluck variant's OPL2 chunk in 560 Hz ticks, including the wait after its last record.
pub fn opl_ticks(variant: usize) -> u32 {
    data::OPL_TICKS[variant % VARIANTS]
}

/// The PC speaker divisors of a cluck variant, one per 140 Hz step, with 0 for gate off; the last is always 0.
pub fn speaker_steps(variant: usize) -> &'static [u16] {
    data::SPEAKER[variant % VARIANTS]
}

/// Steps one OPL2 chunk. Call [`tick`](Self::tick) once per 560 Hz tick; it needs no memory beyond itself.
#[derive(Clone, Debug)]
pub struct OplPlayer {
    steps: &'static [OplStep],
    next: usize,
    /// Ticks still to pass before the record at `next` is due.
    delay: u32,
}

impl OplPlayer {
    /// A player at the start of `variant`'s chunk. The first [`tick`](Self::tick) emits its opening writes.
    pub fn new(variant: usize) -> Self {
        Self {
            steps: opl_steps(variant),
            next: 0,
            delay: 0,
        }
    }

    /// Emits the writes due on this tick to `out(reg, val)`, in order, and returns `true` while the chunk has ticks left.
    /// It returns `false`, writing nothing, on the tick after the chunk's last wait has run out and on every tick after; the
    /// chunk's last write is the key-off, so the channel is silent by then.
    pub fn tick(&mut self, out: &mut impl FnMut(u8, u8)) -> bool {
        loop {
            if self.delay > 0 {
                break;
            }
            let Some(step) = self.steps.get(self.next) else {
                return false;
            };
            out(step.reg, step.val);
            self.delay = u32::from(step.wait);
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

    /// Drives a variant to its end, returning the writes with the tick each landed on and the number of ticks that returned true.
    fn run_opl(variant: usize) -> (Vec<(u32, u8, u8)>, u32) {
        let mut p = OplPlayer::new(variant);
        let mut writes = Vec::new();
        let mut tick = 0u32;
        loop {
            let t = tick;
            if !p.tick(&mut |r, v| writes.push((t, r, v))) {
                return (writes, tick);
            }
            tick += 1;
            assert!(tick < 100_000, "variant {variant} never ends");
        }
    }

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
            let key_writes: Vec<_> = writes.iter().filter(|w| w.1 == b).collect();
            assert!(
                key_writes.iter().any(|w| w.2 & 0x20 != 0),
                "variant {v} never keys on"
            );
            assert_eq!(
                key_writes.last().unwrap().2 & 0x20,
                0,
                "variant {v} ends keyed on"
            );
            // Every write is to channel 5's operators and registers, or the chip's waveform-select enable.
            for &(_, reg, _) in &writes {
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
        while p.tick(&mut |_, _| count += 1) {
            last += 1;
        }
        assert_eq!(last, opl_ticks(0));
        for _ in 0..5 {
            assert!(!p.tick(&mut |_, _| panic!("a finished player wrote")));
        }
        assert_eq!(count, opl_steps(0).len());
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
    fn a_player_is_small_and_copyable_state() {
        // The steppers hold only a slice, an index and a counter, so an interrupt handler can keep one in a static.
        assert!(core::mem::size_of::<OplPlayer>() <= 32);
        assert!(core::mem::size_of::<SpeakerPlayer>() <= 24);
    }
}
